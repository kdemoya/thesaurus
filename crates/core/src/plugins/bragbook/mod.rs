//! First-party local plugin: durable collection, user review and accepted highlights.
use super::*;
use crate::continuity::permit;
use registry::{CapabilityProvider, ProviderFuture};
mod collection;
mod export;
#[cfg(test)]
mod tests;

pub const ID: &str = "thesaurus.bragbook";
pub struct Bragbook;
impl CapabilityProvider for Bragbook {
    fn manifest(&self) -> PluginManifest {
        serde_json::from_value(json!({
            "id":ID,"name":"Bragbook","version":"1.0.0","api_min":2,"api_max":2,
            "host":"service","description":"Collect cited accomplishment candidates locally. Review your contribution and impact before accepting entries or exporting highlights.",
            "hooks":["document.revision.created","task.completed","task.changed","task.accepted","delivery.changed"],
            "requests":["sources.read"],
            "configuration":[
                {"key":"completed-tasks","label":"Completed tasks","kind":"boolean","default":true,"description":"Collect recorded task completions from selected sources."},
                {"key":"confirmed-deliveries","label":"Confirmed deliveries","kind":"boolean","default":true,"description":"Collect explicitly confirmed delivery commitments."},
                {"key":"source-markers","label":"Marked accomplishments","kind":"boolean","default":true,"description":"Collect source lines starting Accomplishment:, Achievement:, Impact: or Shipped:."}
            ]
        })).expect("static bragbook manifest")
    }
    fn health(&self) -> Result<()> {
        Ok(())
    }
    fn invoke<'a>(&'a self, _command: &'a str, _input: Value) -> ProviderFuture<'a> {
        Box::pin(async { bail!("invalid: use the bragbook review API") })
    }
}

fn admin(access: &Access) -> Result<()> {
    if access.source_ids.is_some()
        || access.external
        || !access.allow_write
        || access.contributor != "You"
    {
        bail!("forbidden: bragbook requires the user");
    }
    Ok(())
}
async fn raw(tx: &mut Transaction<'_, Sqlite>, id: &str) -> Result<BragbookEntry> {
    let record: String = sqlx::query_scalar("SELECT record FROM bragbook_entries WHERE id=?")
        .bind(id)
        .fetch_optional(&mut **tx)
        .await?
        .context("record not found")?;
    Ok(serde_json::from_str(&record)?)
}
async fn allowed(
    tx: &mut Transaction<'_, Sqlite>,
    p: &PluginRecord,
    item: &BragbookEntry,
    access: &Access,
) -> Result<()> {
    permit(tx, &item.source_ids, access).await?;
    contributions::scope_tx(tx, p, &item.source_ids, &[]).await
}
async fn refresh(
    tx: &mut Transaction<'_, Sqlite>,
    mut item: BragbookEntry,
) -> Result<BragbookEntry> {
    item.stale_evidence = false;
    for e in &item.evidence {
        let rev: Option<i64> = sqlx::query_scalar("SELECT revision FROM documents WHERE id=?")
            .bind(&e.document_id)
            .fetch_optional(&mut **tx)
            .await?;
        item.stale_evidence |= rev != Some(e.revision);
    }
    item.support_changed = match item.origin_kind.as_str() {
        "task" => {
            sqlx::query_scalar::<_, String>("SELECT status FROM tasks WHERE id=?")
                .bind(&item.origin_id)
                .fetch_optional(&mut **tx)
                .await?
                .as_deref()
                != Some("completed")
        }
        "delivery" => {
            sqlx::query_scalar::<_, String>(
                "SELECT json_extract(record,'$.status') FROM delivery_commitments WHERE id=?",
            )
            .bind(&item.origin_id)
            .fetch_optional(&mut **tx)
            .await?
            .as_deref()
                != Some("completed")
        }
        _ => false,
    };
    Ok(item)
}
async fn save(
    tx: &mut Transaction<'_, Sqlite>,
    item: &BragbookEntry,
    contributor: &str,
) -> Result<()> {
    let record = serde_json::to_string(item)?;
    if record.len() > 32_000 {
        bail!("invalid: bragbook entry exceeds 32 KB");
    }
    sqlx::query("UPDATE bragbook_entries SET status=?,version=?,record=? WHERE id=?")
        .bind(&item.status)
        .bind(item.version)
        .bind(&record)
        .bind(&item.id)
        .execute(&mut **tx)
        .await?;
    sqlx::query(
        "INSERT INTO bragbook_history(entry_id,version,contributor,record) VALUES(?,?,?,?)",
    )
    .bind(&item.id)
    .bind(item.version)
    .bind(contributor)
    .bind(&record)
    .execute(&mut **tx)
    .await?;
    event(tx, "bragbook", &item.id).await
}
impl Store {
    pub async fn migrate_bragbook(&self) -> Result<()> {
        sqlx::raw_sql(include_str!("schema.sql"))
            .execute(&self.pool)
            .await?;
        Ok(())
    }
    pub async fn bragbook_entries(
        &self,
        query: BragbookQuery,
        access: &Access,
    ) -> Result<BragbookList> {
        admin(access)?;
        let status = query.status.as_deref().unwrap_or("candidate");
        if !["candidate", "accepted", "dismissed", "all"].contains(&status) {
            bail!("invalid: bragbook status");
        }
        let offset = query.offset.unwrap_or(0) as usize;
        let limit = query.limit.unwrap_or(25) as usize;
        if offset > 10000 || !(1..=50).contains(&limit) {
            bail!("invalid: offset 0–10000, limit 1–50");
        }
        let mut tx = self.pool.begin().await?;
        let p = load(&mut tx, ID).await?;
        let rows = sqlx::query_scalar::<_, String>("SELECT record FROM bragbook_entries WHERE status=? OR ?='all' ORDER BY collected_at DESC,id")
            .bind(status).bind(status).fetch_all(&mut *tx).await?;
        let mut items = vec![];
        let mut total = 0;
        for record in rows {
            let item: BragbookEntry = serde_json::from_str(&record)?;
            if allowed(&mut tx, &p, &item, access).await.is_err() {
                continue;
            }
            if total >= offset && items.len() < limit {
                items.push(refresh(&mut tx, item).await?);
            }
            total += 1;
        }
        tx.commit().await?;
        Ok(BragbookList {
            next_offset: (offset + items.len() < total).then_some(offset + items.len()),
            items,
            total,
        })
    }
    pub async fn review_bragbook_entry(
        &self,
        id: &str,
        req: BragbookReview,
        access: &Access,
    ) -> Result<BragbookEntry> {
        admin(access)?;
        if !["candidate", "accepted", "dismissed"].contains(&req.status.as_str()) {
            bail!("invalid: bragbook status");
        }
        validate_text(&req.title, 300, "accomplishment title")?;
        validate_text(&req.contribution, 4000, "your contribution")?;
        if req.impact.len() > 4000 || req.impact.contains('\0') {
            bail!("invalid: impact must be at most 4000 bytes");
        }
        if let Some(day) = &req.occurred_on {
            let parsed = chrono::NaiveDate::parse_from_str(day, "%Y-%m-%d")
                .context("invalid: use YYYY-MM-DD")?;
            if parsed.to_string() != *day || parsed > chrono::Utc::now().date_naive() {
                bail!("invalid: accomplishment date must be today or earlier");
            }
        }
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let p = load(&mut tx, ID).await?;
        let mut item = raw(&mut tx, id).await?;
        allowed(&mut tx, &p, &item, access).await?;
        let (digest, old) =
            operation(&mut tx, "bragbook-review", id, &req.idempotency_key, &req).await?;
        if old.is_some() {
            return refresh(&mut tx, item).await;
        }
        if item.version != req.expected_version {
            bail!("conflict: this entry changed; reload before saving");
        }
        item = refresh(&mut tx, item).await?;
        if req.status == "accepted" && item.support_changed {
            bail!("conflict: the underlying work is no longer completed; review its current state");
        }
        item.title = req.title.trim().into();
        item.contribution = req.contribution.trim().into();
        item.impact = req.impact.trim().into();
        item.occurred_on = req.occurred_on;
        item.status = req.status;
        item.version += 1;
        item.updated_at = now();
        item.reviewed_at = (item.status != "candidate").then(now);
        save(&mut tx, &item, "You").await?;
        remember(&mut tx, &req.idempotency_key, &digest, id).await?;
        tx.commit().await?;
        Ok(item)
    }
}
