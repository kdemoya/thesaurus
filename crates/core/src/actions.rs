//! Explicit action previews and atomic receipts for registered local providers.
use crate::{
    Store,
    continuity::{event, operation, remember},
    store::{hash, now, validate_text},
};
use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use sqlx::{Sqlite, Transaction};
use thesaurus_contracts::*;
async fn task(tx: &mut Transaction<'_, Sqlite>, id: &str) -> Result<Task> {
    Ok(crate::store::task_row(
        &sqlx::query("SELECT * FROM tasks WHERE id=?")
            .bind(id)
            .fetch_optional(&mut **tx)
            .await?
            .context("record not found")?,
    ))
}
async fn raw_preview(tx: &mut Transaction<'_, Sqlite>, id: &str) -> Result<ActionPreview> {
    let r: String = sqlx::query_scalar("SELECT record FROM action_previews WHERE id=?")
        .bind(id)
        .fetch_optional(&mut **tx)
        .await?
        .context("record not found")?;
    Ok(serde_json::from_str(&r)?)
}
async fn raw_receipt(tx: &mut Transaction<'_, Sqlite>, id: &str) -> Result<ActionReceipt> {
    let r: String = sqlx::query_scalar("SELECT record FROM action_receipts WHERE id=?")
        .bind(id)
        .fetch_optional(&mut **tx)
        .await?
        .context("record not found")?;
    Ok(serde_json::from_str(&r)?)
}
async fn frame(tx: &mut Transaction<'_, Sqlite>, id: &str) -> Result<Value> {
    let r: String = sqlx::query_scalar("SELECT record FROM cap_frames WHERE id=?")
        .bind(id)
        .fetch_optional(&mut **tx)
        .await?
        .context("record not found")?;
    let f: ActivityFrame = serde_json::from_str(&r)?;
    Ok(
        json!({"id":f.id,"session_id":f.session_id,"created_at":f.created_at,"png_bytes":f.png_bytes,"text_hash":hash(&f.text)}),
    )
}
async fn apply_status(
    tx: &mut Transaction<'_, Sqlite>,
    old: &Task,
    status: &str,
    origin: &str,
) -> Result<Task> {
    if old.status == "superseded" {
        bail!("conflict: a superseded task cannot be changed");
    }
    if status == "accepted" && old.status == "candidate" {
        let revision: i64 = sqlx::query_scalar("SELECT revision FROM documents WHERE id=?")
            .bind(&old.evidence.document_id)
            .fetch_one(&mut **tx)
            .await?;
        if revision != old.evidence.revision {
            bail!("conflict: task evidence changed; review current source");
        }
    }
    let changed = sqlx::query(
        "UPDATE tasks SET status=?,version=version+1,updated_at=? WHERE id=? AND version=?",
    )
    .bind(status)
    .bind(now())
    .bind(&old.id)
    .bind(old.version)
    .execute(&mut **tx)
    .await?;
    if changed.rows_affected() != 1 {
        bail!("conflict: task changed before the approved effect");
    }
    let after = task(tx, &old.id).await?;
    crate::insights::record_task_transition_tx(tx, Some(old), &after, origin).await?;
    event(tx, "task", &old.id).await?;
    Ok(after)
}
impl Store {
    pub async fn migrate_actions(&self) -> Result<()> {
        sqlx::raw_sql("CREATE TABLE IF NOT EXISTS action_previews(id TEXT PRIMARY KEY,record TEXT NOT NULL); CREATE TABLE IF NOT EXISTS action_receipts(id TEXT PRIMARY KEY,preview_id TEXT NOT NULL UNIQUE,record TEXT NOT NULL); CREATE TABLE IF NOT EXISTS action_receipt_history(receipt_id TEXT NOT NULL,version INTEGER NOT NULL,record TEXT NOT NULL,PRIMARY KEY(receipt_id,version));").execute(&self.pool).await?;
        Ok(())
    }
    pub async fn action_previews(&self) -> Result<Vec<ActionPreview>> {
        let rows: Vec<String> =
            sqlx::query_scalar("SELECT record FROM action_previews ORDER BY rowid DESC LIMIT 100")
                .fetch_all(&self.pool)
                .await?;
        rows.into_iter()
            .map(|s| Ok(serde_json::from_str(&s)?))
            .collect()
    }
    pub async fn action_receipts(&self) -> Result<Vec<ActionReceipt>> {
        let rows: Vec<String> =
            sqlx::query_scalar("SELECT record FROM action_receipts ORDER BY rowid DESC LIMIT 100")
                .fetch_all(&self.pool)
                .await?;
        rows.into_iter()
            .map(|s| Ok(serde_json::from_str(&s)?))
            .collect()
    }
    pub async fn action_receipt(&self, id: &str) -> Result<ActionReceipt> {
        let mut tx = self.pool.begin().await?;
        raw_receipt(&mut tx, id).await
    }
    pub async fn action_preview(&self, id: &str) -> Result<ActionPreview> {
        let mut tx = self.pool.begin().await?;
        raw_preview(&mut tx, id).await
    }
    pub async fn action_preview_receipt(&self, id: &str) -> Result<Option<ActionReceipt>> {
        let record: Option<String> =
            sqlx::query_scalar("SELECT record FROM action_receipts WHERE preview_id=?")
                .bind(id)
                .fetch_optional(&self.pool)
                .await?;
        record.map(|s| Ok(serde_json::from_str(&s)?)).transpose()
    }
    pub async fn preview_action(&self, req: ActionPreviewRequest) -> Result<ActionPreview> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let preview = Self::preview_action_tx(&mut tx, req, None).await?;
        tx.commit().await?;
        Ok(preview)
    }
    pub(crate) async fn preview_action_tx(
        tx: &mut Transaction<'_, Sqlite>,
        req: ActionPreviewRequest,
        origin: Option<PluginActionOrigin>,
    ) -> Result<ActionPreview> {
        validate_text(&req.target_id, 100, "action target")?;
        let (digest, replay) = if let Some(origin) = &origin {
            operation(
                tx,
                "plugin-action-preview",
                "new",
                &req.idempotency_key,
                &(&req, origin),
            )
            .await?
        } else {
            operation(tx, "action-preview", "new", &req.idempotency_key, &req).await?
        };
        if let Some(id) = replay {
            return raw_preview(tx, &id).await;
        }
        let (before, proposed, undo, explanation) = match req.provider.as_str() {
            "task_status" => {
                let old = task(tx, &req.target_id).await?;
                let status = req
                    .status
                    .as_deref()
                    .context("invalid: choose a proposed task status")?;
                if !["accepted", "dismissed", "completed", "candidate"].contains(&status)
                    || old.status == status
                {
                    bail!("invalid: choose a different supported task status");
                }
                if req.expected_version != Some(old.version) || old.status == "superseded" {
                    bail!("conflict: task changed or superseded");
                }
                (serde_json::to_value(&old)?,json!({"status":status,"version":old.version+1,"updated_at":"execution time"}),true,"Change only this local task's status and increment its version. Record transition, event and receipt atomically. No message or external task is changed.".to_string())
            }
            "delete_activity_frame" => {
                if req.status.is_some() || req.expected_version.is_some() {
                    bail!("invalid: frame deletion has no task status/version");
                }
                (frame(tx,&req.target_id).await?,json!({"delete":"PNG, OCR text and search entry for exactly this frame"}),false,"Permanently delete one retained local capture frame and its searchable text. The receipt retains identifiers, size, time and a text digest, never the image or OCR text. This provider does not support undo.".to_string())
            }
            _ => bail!(
                "invalid: supported action providers are task_status and delete_activity_frame"
            ),
        };
        let fingerprint = if let Some(origin) = &origin {
            hash(&serde_json::to_string(&(
                &req.provider,
                &req.target_id,
                &before,
                &proposed,
                origin,
            ))?)
        } else {
            hash(&serde_json::to_string(&(
                &req.provider,
                &req.target_id,
                &before,
                &proposed,
            ))?)
        };
        let p = ActionPreview {
            id: uuid::Uuid::new_v4().to_string(),
            provider: req.provider,
            target_id: req.target_id,
            before,
            proposed,
            fingerprint,
            supports_undo: undo,
            explanation,
            created_at: now(),
            status: "proposed".into(),
            origin,
        };
        sqlx::query("INSERT INTO action_previews(id,record) VALUES(?,?)")
            .bind(&p.id)
            .bind(serde_json::to_string(&p)?)
            .execute(&mut **tx)
            .await?;
        remember(tx, &req.idempotency_key, &digest, &p.id).await?;
        event(tx, "action_preview", &p.id).await?;
        Ok(p)
    }
    pub async fn execute_action(
        &self,
        id: &str,
        req: ActionExecuteRequest,
    ) -> Result<ActionReceipt> {
        if !req.approve_effects {
            bail!("forbidden: explicitly approve the previewed effects");
        }
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let (digest, replay) =
            operation(&mut tx, "action-execute", id, &req.idempotency_key, &req).await?;
        if let Some(id) = replay {
            return raw_receipt(&mut tx, &id).await;
        }
        let mut p = raw_preview(&mut tx, id).await?;
        if p.status != "proposed" || p.fingerprint != req.fingerprint {
            bail!("conflict: preview changed or already executed");
        }
        if let Some(origin) = &p.origin {
            crate::plugins::contributions::validate_action_origin_tx(&mut tx, origin, &p).await?;
        }
        let after = match p.provider.as_str() {
            "task_status" => {
                let old = task(&mut tx, &p.target_id).await?;
                if serde_json::to_value(&old)? != p.before {
                    bail!("conflict: task changed after preview; prepare a new action");
                }
                let status = p.proposed["status"]
                    .as_str()
                    .context("invalid proposed status")?;
                serde_json::to_value(apply_status(&mut tx, &old, status, "approved_action").await?)?
            }
            "delete_activity_frame" => {
                if frame(&mut tx, &p.target_id).await? != p.before {
                    bail!("conflict: frame changed after preview");
                }
                sqlx::query("DELETE FROM cap_frames_fts WHERE frame_id=?")
                    .bind(&p.target_id)
                    .execute(&mut *tx)
                    .await?;
                sqlx::query("DELETE FROM cap_frames WHERE id=?")
                    .bind(&p.target_id)
                    .execute(&mut *tx)
                    .await?;
                event(&mut tx, "capture", &p.target_id).await?;
                json!({"deleted":true,"frame_id":p.target_id})
            }
            _ => bail!("invalid action provider"),
        };
        if p.origin.is_some() {
            sqlx::query(
                "DELETE FROM plugin_effect_context WHERE contributor='__host_transaction__'",
            )
            .execute(&mut *tx)
            .await?;
        }
        p.status = "executed".into();
        let r = ActionReceipt {
            id: uuid::Uuid::new_v4().to_string(),
            preview_id: p.id.clone(),
            provider: p.provider.clone(),
            target_id: p.target_id.clone(),
            version: 1,
            before: p.before.clone(),
            after,
            supports_undo: p.supports_undo,
            status: "executed".into(),
            executed_at: now(),
            undone_at: None,
            undo_effect: None,
            contributor: "You".into(),
            origin: p.origin.clone(),
        };
        sqlx::query("INSERT INTO action_receipts(id,preview_id,record) VALUES(?,?,?)")
            .bind(&r.id)
            .bind(id)
            .bind(serde_json::to_string(&r)?)
            .execute(&mut *tx)
            .await?;
        sqlx::query("INSERT INTO action_receipt_history(receipt_id,version,record) VALUES(?,1,?)")
            .bind(&r.id)
            .bind(serde_json::to_string(&r)?)
            .execute(&mut *tx)
            .await?;
        sqlx::query("UPDATE action_previews SET record=? WHERE id=?")
            .bind(serde_json::to_string(&p)?)
            .bind(id)
            .execute(&mut *tx)
            .await?;
        remember(&mut tx, &req.idempotency_key, &digest, &r.id).await?;
        event(&mut tx, "action_receipt", &r.id).await?;
        tx.commit().await?;
        Ok(r)
    }
    pub async fn undo_action(&self, id: &str, req: ActionUndoRequest) -> Result<ActionReceipt> {
        if !req.approve_effects {
            bail!("forbidden: approve restoring the previous task status");
        }
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let (digest, replay) =
            operation(&mut tx, "action-undo", id, &req.idempotency_key, &req).await?;
        if let Some(id) = replay {
            return raw_receipt(&mut tx, &id).await;
        }
        let mut r = raw_receipt(&mut tx, id).await?;
        if r.version != req.expected_receipt_version || r.status != "executed" {
            bail!("conflict: action receipt changed or already undone");
        }
        if !r.supports_undo || r.provider != "task_status" {
            bail!("invalid: this provider does not support undo");
        }
        let old = task(&mut tx, &r.target_id).await?;
        if serde_json::to_value(&old)? != r.after {
            bail!("conflict: task changed since execution; undo would overwrite newer work");
        }
        let status = r.before["status"]
            .as_str()
            .context("invalid recorded prior status")?;
        let after = apply_status(&mut tx, &old, status, "approved_action_undo").await?;
        r.undo_effect = Some(serde_json::to_value(after)?);
        r.status = "undone".into();
        r.version += 1;
        r.undone_at = Some(now());
        let record = serde_json::to_string(&r)?;
        sqlx::query("UPDATE action_receipts SET record=? WHERE id=?")
            .bind(&record)
            .bind(id)
            .execute(&mut *tx)
            .await?;
        sqlx::query("INSERT INTO action_receipt_history(receipt_id,version,record) VALUES(?,?,?)")
            .bind(id)
            .bind(r.version)
            .bind(record)
            .execute(&mut *tx)
            .await?;
        remember(&mut tx, &req.idempotency_key, &digest, id).await?;
        event(&mut tx, "action_receipt", id).await?;
        tx.commit().await?;
        Ok(r)
    }
}
