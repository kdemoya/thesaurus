use super::*;
use crate::store::now;
use sqlx::Row;
fn day() -> String {
    chrono::Utc::now().format("%Y-%m-%d").to_string()
}
fn key(id: &str, attempt: i64) -> String {
    format!("job:{id}:{attempt}")
}
pub(crate) async fn migrate_ledger(store: &Store) -> Result<()> {
    let mut tx = store.pool.begin_with("BEGIN IMMEDIATE").await?;
    sqlx::raw_sql("CREATE TABLE IF NOT EXISTS usage_job_attempts(id TEXT PRIMARY KEY,job_id TEXT NOT NULL,connection TEXT NOT NULL,attempt INTEGER NOT NULL,units INTEGER NOT NULL DEFAULT 1,reserved_day TEXT NOT NULL,started_day TEXT,state TEXT NOT NULL,created_at TEXT NOT NULL); CREATE INDEX IF NOT EXISTS usage_job_attempt_day ON usage_job_attempts(connection,reserved_day,started_day); CREATE TABLE IF NOT EXISTS usage_ledger_metadata(id TEXT PRIMARY KEY,record TEXT NOT NULL);").execute(&mut *tx).await?;
    let initialized: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM usage_ledger_metadata WHERE id='migration')",
    )
    .fetch_one(&mut *tx)
    .await?;
    if !initialized {
        for row in sqlx::query("SELECT * FROM jobs WHERE connection!='local'")
            .fetch_all(&mut *tx)
            .await?
        {
            let job = crate::store::job_row(&row);
            let d = job.updated_at.chars().take(10).collect::<String>();
            if job.attempt > 0 {
                sqlx::query("INSERT OR IGNORE INTO usage_job_attempts(id,job_id,connection,attempt,units,reserved_day,started_day,state,created_at) VALUES(?,?,?,?,?,?,?,'started',?)").bind(format!("legacy-job:{}",job.id)).bind(&job.id).bind(&job.connection).bind(job.attempt).bind(job.attempt).bind(&d).bind(&d).bind(now()).execute(&mut *tx).await?;
            }
            if job.status == "queued" {
                reserve_job_tx(&mut tx, &job).await?;
            }
        }
        sqlx::query("INSERT INTO usage_ledger_metadata(id,record) VALUES('migration',?)").bind(format!("{}: earlier job attempt dates were not recorded; migrated attempts conservatively assigned to their last known update day. New attempts retain distinct reservation/start days.",now())).execute(&mut *tx).await?;
    }
    tx.commit().await?;
    Ok(())
}
pub(crate) async fn reserve_job_tx(tx: &mut Transaction<'_, Sqlite>, job: &Job) -> Result<()> {
    if job.connection == "local" {
        return Ok(());
    }
    sqlx::query("INSERT INTO usage_job_attempts(id,job_id,connection,attempt,units,reserved_day,state,created_at) VALUES(?,?,?,?,1,?,'reserved',?)").bind(key(&job.id,job.attempt+1)).bind(&job.id).bind(&job.connection).bind(job.attempt+1).bind(day()).bind(now()).execute(&mut **tx).await?;
    Ok(())
}
pub(crate) async fn used_tx(tx: &mut Transaction<'_, Sqlite>, connection: &str) -> Result<i64> {
    let d = day();
    Ok(sqlx::query_scalar("SELECT (SELECT COUNT(*) FROM runtime_runs WHERE connection=? AND substr(created_at,1,10)=?)+(SELECT COALESCE(SUM(units),0) FROM usage_job_attempts WHERE connection=? AND ((state='started' AND started_day=?) OR (state='reserved' AND reserved_day=?)))").bind(connection).bind(&d).bind(connection).bind(&d).bind(&d).fetch_one(&mut **tx).await?)
}
impl Store {
    pub(crate) async fn start_model_job(&self, job: &Job, configured: i64) -> Result<()> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let d = day();
        let id = key(&job.id, job.attempt);
        let existing = sqlx::query("SELECT state,reserved_day FROM usage_job_attempts WHERE id=?")
            .bind(&id)
            .fetch_optional(&mut *tx)
            .await?;
        let already_counted = existing.as_ref().is_some_and(|r| {
            r.get::<String, _>("state") == "reserved" && r.get::<String, _>("reserved_day") == d
        });
        let limit = effective_limit_tx(&mut tx, &job.connection, configured, true).await?;
        if used_tx(&mut tx, &job.connection).await? + i64::from(!already_counted) > limit {
            sqlx::query(
                "UPDATE usage_job_attempts SET state='blocked' WHERE id=? AND state='reserved'",
            )
            .bind(&id)
            .execute(&mut *tx)
            .await?;
            tx.commit().await?;
            bail!("forbidden: background work exceeds current budget; no provider request made");
        }
        if existing.is_some() {
            sqlx::query("UPDATE usage_job_attempts SET state='started',started_day=? WHERE id=? AND state='reserved'").bind(&d).bind(&id).execute(&mut *tx).await?;
        } else {
            sqlx::query("INSERT INTO usage_job_attempts(id,job_id,connection,attempt,units,reserved_day,started_day,state,created_at) VALUES(?,?,?,?,1,?,?,'started',?)").bind(&id).bind(&job.id).bind(&job.connection).bind(job.attempt).bind(&d).bind(&d).bind(now()).execute(&mut *tx).await?;
        }
        tx.commit().await?;
        Ok(())
    }
}
