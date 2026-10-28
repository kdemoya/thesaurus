//! Durable meeting automation. A job pins its destination when recording starts.
use super::*;
use sqlx::{Sqlite, Transaction};

async fn settings_tx(tx: &mut Transaction<'_, Sqlite>) -> Result<MeetingProcessingSettings> {
    let record: Option<String> =
        sqlx::query_scalar("SELECT record FROM meeting_processing_settings WHERE id=1")
            .fetch_optional(&mut **tx)
            .await?;
    Ok(record
        .map(|s| serde_json::from_str(&s))
        .transpose()?
        .unwrap_or_default())
}

#[cfg(test)]
mod tests;
async fn destination_tx(
    tx: &mut Transaction<'_, Sqlite>,
    settings: &MeetingProcessingSettings,
) -> Result<String> {
    if let Some(connection) = &settings.connection {
        return Ok(connection.clone());
    }
    let record: Option<String> =
        sqlx::query_scalar("SELECT record FROM onboarding_progress WHERE id=1")
            .fetch_optional(&mut **tx)
            .await?;
    Ok(record
        .map(|s| serde_json::from_str::<OnboardingProgress>(&s))
        .transpose()?
        .unwrap_or_default()
        .primary_connection)
}
pub(crate) async fn queue_new_meeting(tx: &mut Transaction<'_, Sqlite>, id: &str) -> Result<()> {
    let settings = settings_tx(tx).await?;
    let connection = destination_tx(tx, &settings).await?;
    sqlx::query("INSERT INTO meeting_processing_jobs(session_id,connection,status) VALUES(?,?,?)")
        .bind(id)
        .bind(connection)
        .bind(if settings.automatic_summaries {
            "waiting"
        } else {
            "disabled"
        })
        .execute(&mut **tx)
        .await?;
    Ok(())
}
fn job(row: sqlx::sqlite::SqliteRow) -> MeetingProcessingState {
    MeetingProcessingState {
        session_id: row.get("session_id"),
        connection: row.get("connection"),
        generation: row.get("generation"),
        status: row.get("status"),
        run_id: row.get("run_id"),
        error: row.get("error"),
    }
}

/// Only this meeting's tool-free summary may use its saved destination. General
/// source sharing and external agent grants remain independent of automation.
pub(crate) async fn check_destination_tx(
    tx: &mut Transaction<'_, Sqlite>,
    summary: &MeetingSummary,
    external: bool,
) -> Result<()> {
    crate::source_controls::check_connection_tx(tx, &summary.source_id, &summary.connection, false)
        .await?;
    let managed: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM meeting_processing_jobs WHERE session_id=? AND connection=? AND run_id=? AND status='processing')")
        .bind(&summary.session_id).bind(&summary.connection).bind(&summary.id).fetch_one(&mut **tx).await?;
    if !managed {
        crate::source_controls::check_connection_tx(
            tx,
            &summary.source_id,
            &summary.connection,
            external,
        )
        .await?;
    }
    Ok(())
}
pub(crate) async fn claim_tx(
    tx: &mut Transaction<'_, Sqlite>,
    session: &str,
    connection: &str,
    generation: i64,
    run: &str,
) -> Result<()> {
    let changed = sqlx::query("UPDATE meeting_processing_jobs SET status='processing',run_id=?,error=NULL WHERE session_id=? AND connection=? AND generation=? AND status='waiting'")
        .bind(run).bind(session).bind(connection).bind(generation).execute(&mut **tx).await?.rows_affected();
    if changed != 1 {
        bail!("conflict: meeting processing job changed");
    }
    Ok(())
}

impl Store {
    pub async fn meeting_processing_settings(&self) -> Result<MeetingProcessingSettings> {
        let mut tx = self.pool.begin().await?;
        settings_tx(&mut tx).await
    }
    pub async fn save_meeting_processing_settings(
        &self,
        req: MeetingProcessingSettingsRequest,
        config: &crate::Config,
    ) -> Result<MeetingProcessingSettings> {
        if req
            .settings
            .connection
            .as_ref()
            .is_some_and(|id| id != "local" && !config.connections.iter().any(|c| &c.id == id))
        {
            bail!("invalid: choose a configured meeting model");
        }
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let (digest, old) = operation(
            &mut tx,
            "meeting-processing-settings",
            "workspace",
            &req.idempotency_key,
            &req,
        )
        .await?;
        let previous = settings_tx(&mut tx).await?;
        if old.is_some() {
            return Ok(previous);
        }
        if previous.version != req.settings.version {
            bail!("conflict: meeting settings changed; reload before saving");
        }
        let mut settings = req.settings;
        settings.version += 1;
        sqlx::query("INSERT INTO meeting_processing_settings(id,record) VALUES(1,?) ON CONFLICT(id) DO UPDATE SET record=excluded.record")
            .bind(serde_json::to_string(&settings)?).execute(&mut *tx).await?;
        if !settings.automatic_summaries {
            sqlx::query(
                "UPDATE meeting_processing_jobs SET status='disabled' WHERE status='waiting'",
            )
            .execute(&mut *tx)
            .await?;
        }
        remember(&mut tx, &req.idempotency_key, &digest, "workspace").await?;
        event(&mut tx, "meeting-processing", "settings").await?;
        tx.commit().await?;
        Ok(settings)
    }
    pub async fn meeting_processing_state(
        &self,
        id: &str,
    ) -> Result<Option<MeetingProcessingState>> {
        Ok(
            sqlx::query("SELECT * FROM meeting_processing_jobs WHERE session_id=?")
                .bind(id)
                .fetch_optional(&self.pool)
                .await?
                .map(job),
        )
    }
    /// One explicit retry/regeneration uses the saved destination without another
    /// per-call approval. Replayed requests cannot reserve another model run.
    pub async fn request_meeting_processing(
        &self,
        id: &str,
        req: MeetingProcessingRequest,
    ) -> Result<MeetingProcessingState> {
        let session = self.live_meeting(id).await?;
        if session.state != "stopped" || session.processing || session.pending_chunks != 0 {
            bail!("conflict: wait for the recording and transcription to finish");
        }
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let (digest, old) = operation(
            &mut tx,
            "meeting-processing",
            id,
            &req.idempotency_key,
            &req,
        )
        .await?;
        if old.is_none() {
            let settings = settings_tx(&mut tx).await?;
            let connection = destination_tx(&mut tx, &settings).await?;
            sqlx::query("INSERT INTO meeting_processing_jobs(session_id,connection,status) VALUES(?,?,'waiting') ON CONFLICT(session_id) DO UPDATE SET connection=excluded.connection,generation=generation+1,status='waiting',run_id=NULL,error=NULL WHERE status NOT IN ('waiting','processing')")
                .bind(id).bind(connection).execute(&mut *tx).await?;
            remember(&mut tx, &req.idempotency_key, &digest, id).await?;
            event(&mut tx, "meeting-processing", id).await?;
        }
        let state = job(
            sqlx::query("SELECT * FROM meeting_processing_jobs WHERE session_id=?")
                .bind(id)
                .fetch_one(&mut *tx)
                .await?,
        );
        tx.commit().await?;
        Ok(state)
    }
    pub async fn pending_meeting_processing(&self) -> Result<Vec<MeetingProcessingState>> {
        let rows = sqlx::query("SELECT j.* FROM meeting_processing_jobs j JOIN live_meetings m ON m.id=j.session_id WHERE j.status='waiting' AND m.state='stopped' AND COALESCE(json_extract(m.record,'$.pending_chunks'),0)=0 AND COALESCE(json_extract(m.record,'$.processing'),0)=0 ORDER BY m.created_at,j.session_id LIMIT 20")
            .fetch_all(&self.pool).await?;
        Ok(rows.into_iter().map(job).collect())
    }
    pub async fn block_meeting_processing(
        &self,
        id: &str,
        status: &str,
        error: Option<&str>,
    ) -> Result<()> {
        if !["failed", "empty"].contains(&status) {
            bail!("invalid: processing state");
        }
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let changed = sqlx::query("UPDATE meeting_processing_jobs SET status=?,error=? WHERE session_id=? AND status='waiting'")
            .bind(status).bind(error.map(|e|e.chars().take(1500).collect::<String>())).bind(id).execute(&mut *tx).await?.rows_affected();
        if changed > 0 {
            event(&mut tx, "meeting-processing", id).await?;
        }
        tx.commit().await?;
        Ok(())
    }
}
