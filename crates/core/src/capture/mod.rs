use crate::{
    Store,
    continuity::{event, operation, permit, project_sources, remember},
    private_dir, private_write,
    store::{now, validate_text},
};
use anyhow::{Context, Result, bail};
use sqlx::Row;
use std::{path::Path, process::Stdio};
use thesaurus_contracts::*;
use tokio::{io::AsyncReadExt, process::Command};
pub(crate) async fn read_bounded(path: &Path, max: usize) -> Result<Vec<u8>> {
    let file = tokio::fs::File::open(path).await?;
    let mut out = vec![];
    file.take(max as u64 + 1).read_to_end(&mut out).await?;
    if out.len() > max {
        bail!("invalid: capture file exceeds size bound");
    }
    Ok(out)
}
pub(crate) async fn command(c: &mut Command, timeout: u64, max: usize) -> Result<Vec<u8>> {
    c.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    let mut child = c
        .spawn()
        .context("invalid: configured local capture executable unavailable")?;
    let out = child.stdout.take().context("capture stdout unavailable")?;
    let result = tokio::time::timeout(std::time::Duration::from_secs(timeout), async {
        let mut bytes = vec![];
        out.take(max as u64 + 1).read_to_end(&mut bytes).await?;
        if bytes.len() > max {
            bail!("invalid: local processor output exceeds bound");
        }
        if !child.wait().await?.success() {
            bail!(
                "invalid: local processor failed; inspect configuration and OS capture permissions"
            );
        }
        Ok::<_, anyhow::Error>(bytes)
    })
    .await
    .context("invalid: local processor timed out")?;
    let _ = child.kill().await;
    result
}
impl Store {
    pub async fn migrate_capture(&self) -> Result<()> {
        sqlx::raw_sql(include_str!("schema.sql"))
            .execute(&self.pool)
            .await?;
        Ok(())
    }
    pub async fn recover_capture(&self, dir: &Path) -> Result<()> {
        sqlx::query("UPDATE cap_sessions SET state='paused',version=version+1,record=json_set(record,'$.state','paused','$.version',version+1,'$.error','Service restarted; capture requires explicit resume','$.updated_at',?) WHERE state='active'").bind(now()).execute(&self.pool).await?;
        sqlx::query("UPDATE cap_audio SET record=json_set(record,'$.status','interrupted','$.error','Service restarted; explicitly transcribe again with a new request') WHERE json_extract(record,'$.status') IN ('queued','processing')").execute(&self.pool).await?;
        let _ = tokio::fs::remove_dir_all(dir.join("local-audio")).await;
        let _ = tokio::fs::remove_dir_all(dir.join("activity-tmp")).await;
        sqlx::query("UPDATE live_meetings SET state=CASE WHEN state='stopping' THEN 'stopped' ELSE 'paused' END,version=version+1,record=json_set(record,'$.state',CASE WHEN state='stopping' THEN 'stopped' ELSE 'paused' END,'$.version',version+1,'$.recorded_ms',COALESCE(json_extract(record,'$.recorded_ms'),0)+CAST(COALESCE(MAX(0,(julianday(?) - julianday(json_extract(record,'$.active_since')))*86400000),0) AS INTEGER),'$.active_since',NULL,'$.processing',json('false'),'$.pending_chunks',0,'$.error','Service restarted; recording requires explicit resume','$.updated_at',?) WHERE state IN ('starting','recording','pausing','stopping')").bind(now()).bind(now()).execute(&self.pool).await?;
        sqlx::query("UPDATE live_meetings SET record=json_set(record,'$.processing',json('false'),'$.pending_chunks',0,'$.error','Transcription interrupted; unprocessed temporary audio was removed') WHERE json_extract(record,'$.processing')=1").execute(&self.pool).await?;
        let _ = tokio::fs::remove_dir_all(dir.join("live-audio")).await;
        sqlx::query("UPDATE transcription_models SET state='interrupted',detail='Service restarted; explicitly resume the download' WHERE state IN ('downloading','cancelling')").execute(&self.pool).await?;
        sqlx::query("UPDATE transcription_receipts SET record=json_set(record,'$.status','uncertain','$.error','Service restarted during delivery; audio was not resent. Inspect provider request/file IDs and clean up retained resources') WHERE json_extract(record,'$.status')='sending'").execute(&self.pool).await?;
        self.purge_activity().await
    }
}
mod activity;
mod audio;
pub mod live;
pub(crate) mod processing;
mod recording_defaults;
pub(crate) mod summaries;
#[cfg(test)]
mod tests;
pub mod transcription;
