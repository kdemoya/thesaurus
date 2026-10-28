mod calendar;
mod capacity;
mod opportunities;
mod weekly;
use crate::{
    Store,
    continuity::{documents, event, operation, permit, project_sources, remember, tasks},
    store::{now, validate_text},
};
use anyhow::{Context, Result, bail};
use chrono::{DateTime, NaiveDate, Utc};
use sqlx::{Row, Sqlite, Transaction};
use thesaurus_contracts::*;
fn admin(access: &Access) -> Result<()> {
    if access.source_ids.is_some()
        || access.external
        || !access.allow_write
        || access.contributor != "You"
    {
        bail!("forbidden: insight changes require the user");
    }
    Ok(())
}
async fn scope(
    tx: &mut Transaction<'_, Sqlite>,
    project: &str,
    access: &Access,
) -> Result<Vec<String>> {
    let ids = project_sources(tx, project).await?;
    permit(tx, &ids, access).await?;
    Ok(ids)
}
fn date(value: &str) -> Result<NaiveDate> {
    let d = NaiveDate::parse_from_str(value, "%Y-%m-%d").context("invalid date; use YYYY-MM-DD")?;
    if d.to_string() != value {
        bail!("invalid date");
    }
    Ok(d)
}
fn period(start: &str, end: &str) -> Result<(NaiveDate, NaiveDate)> {
    let start = date(start)?;
    let end = date(end)?;
    if end < start || (end - start).num_days() > 30 {
        bail!("invalid: choose a period of 1–31 days");
    }
    Ok((start, end))
}
pub(crate) async fn record_task_transition_tx(
    tx: &mut Transaction<'_, Sqlite>,
    before: Option<&Task>,
    after: &Task,
    origin: &str,
) -> Result<()> {
    let transition = TaskTransition {
        task: after.clone(),
        previous_status: before.map(|t| t.status.clone()),
        origin: origin.into(),
        recorded_at: now(),
    };
    sqlx::query("INSERT OR IGNORE INTO insight_task_history(task_id,version,record,recorded_at) VALUES(?,?,?,?)").bind(&after.id).bind(after.version).bind(serde_json::to_string(&transition)?).bind(&transition.recorded_at).execute(&mut **tx).await?;
    Ok(())
}
async fn history(
    tx: &mut Transaction<'_, Sqlite>,
    kind: &str,
    id: &str,
    version: i64,
    record: &impl serde::Serialize,
) -> Result<()> {
    sqlx::query(
        "INSERT INTO insight_review_history(kind,record_id,version,record) VALUES(?,?,?,?)",
    )
    .bind(kind)
    .bind(id)
    .bind(version)
    .bind(serde_json::to_string(record)?)
    .execute(&mut **tx)
    .await?;
    event(tx, kind, id).await
}
impl Store {
    pub async fn migrate_insights(&self) -> Result<()> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        sqlx::raw_sql("CREATE TABLE IF NOT EXISTS insight_task_history(task_id TEXT NOT NULL REFERENCES tasks(id),version INTEGER NOT NULL,record TEXT NOT NULL,recorded_at TEXT NOT NULL,PRIMARY KEY(task_id,version)); CREATE INDEX IF NOT EXISTS insight_task_history_time ON insight_task_history(recorded_at); CREATE TABLE IF NOT EXISTS insight_opportunities(id TEXT PRIMARY KEY,project_id TEXT NOT NULL REFERENCES project_briefs(id),record TEXT NOT NULL); CREATE TABLE IF NOT EXISTS insight_weekly(id TEXT PRIMARY KEY,project_id TEXT NOT NULL REFERENCES project_briefs(id),record TEXT NOT NULL); CREATE TABLE IF NOT EXISTS insight_review_history(kind TEXT NOT NULL,record_id TEXT NOT NULL,version INTEGER NOT NULL,record TEXT NOT NULL,PRIMARY KEY(kind,record_id,version)); CREATE TABLE IF NOT EXISTS insight_efforts(project_id TEXT NOT NULL REFERENCES project_briefs(id),task_id TEXT NOT NULL REFERENCES tasks(id),record TEXT NOT NULL,PRIMARY KEY(project_id,task_id)); CREATE TABLE IF NOT EXISTS insight_capacity(id TEXT PRIMARY KEY,project_id TEXT NOT NULL REFERENCES project_briefs(id),record TEXT NOT NULL);").execute(&mut *tx).await?;
        for row in sqlx::query("SELECT * FROM tasks t WHERE NOT EXISTS(SELECT 1 FROM insight_task_history h WHERE h.task_id=t.id)").fetch_all(&mut *tx).await?{record_task_transition_tx(&mut tx,None,&crate::store::task_row(&row),"migration_baseline_prior_history_unknown").await?;}
        tx.commit().await?;
        Ok(())
    }
}
