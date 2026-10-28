//! Explicit local question cues and source-backed review of missing knowledge.
mod cues;
mod gaps;
mod refresh;
use crate::{
    Store,
    continuity::{documents, event, operation, permit, project_sources, remember, watermarks},
    store::{hash, now, validate_text},
};
use anyhow::{Context, Result, bail};
use serde::Serialize;
use sqlx::{Row, Sqlite, Transaction};
use thesaurus_contracts::*;
const DETECTION: &str = "Local explicit question markers; conflicts are user-linked. Candidate relevance is lexical, not a verified answer.";
fn page(q: &DiscoveryQuery) -> Result<(usize, usize)> {
    let o = q.offset.unwrap_or(0) as usize;
    let l = q.limit.unwrap_or(25) as usize;
    if o > 100000 || !(1..=50).contains(&l) {
        bail!("invalid discovery page");
    }
    Ok((o, l))
}
fn optional_note(s: &str) -> Result<()> {
    if s.len() > 2000 || s.contains('\0') {
        bail!("invalid discovery note");
    }
    Ok(())
}
async fn citations(
    tx: &mut Transaction<'_, Sqlite>,
    project: &str,
    ev: &[Evidence],
    min: usize,
) -> Result<Vec<String>> {
    if ev.len() < min || ev.len() > 6 {
        bail!("invalid: choose {min}–6 source citations");
    }
    let scope = project_sources(tx, project).await?;
    let mut ids = vec![];
    for e in ev {
        if e.line < 1 || e.quote.trim().is_empty() || e.quote.len() > 2000 {
            bail!("invalid: evidence requires exact nonempty lines up to 2000 bytes");
        }
        let row=sqlx::query("SELECT d.source_id,r.body FROM documents d JOIN revisions r ON r.document_id=d.id WHERE d.id=? AND r.revision=?").bind(&e.document_id).bind(e.revision).fetch_optional(&mut **tx).await?.context("invalid evidence revision")?;
        let source: String = row.get("source_id");
        if !scope.contains(&source)
            || row
                .get::<String, _>("body")
                .lines()
                .nth(e.line as usize - 1)
                != Some(e.quote.as_str())
        {
            bail!("invalid evidence scope or locator");
        }
        ids.push(source);
    }
    ids.sort();
    ids.dedup();
    Ok(ids)
}
async fn allowed(
    tx: &mut Transaction<'_, Sqlite>,
    project: &str,
    access: &Access,
) -> Result<Vec<String>> {
    let scope = project_sources(tx, project).await?;
    let ids = sqlx::query("SELECT id,allow_external FROM sources")
        .fetch_all(&mut **tx)
        .await?
        .into_iter()
        .filter(|r| access.permits(r.get("id"), r.get("allow_external")))
        .map(|r| r.get::<String, _>("id"))
        .collect::<Vec<_>>();
    if !scope.iter().any(|s| ids.contains(s)) {
        bail!("record not found");
    }
    Ok(ids)
}
async fn history(
    tx: &mut Transaction<'_, Sqlite>,
    kind: &str,
    id: &str,
    action: &str,
    contributor: &str,
    note: &str,
    item: &impl Serialize,
) -> Result<()> {
    let e = DiscoveryHistoryItem {
        sequence: 0,
        kind: kind.into(),
        id: id.into(),
        action: action.into(),
        contributor: contributor.into(),
        created_at: now(),
        note: note.into(),
        snapshot: serde_json::to_value(item)?,
    };
    sqlx::query("INSERT INTO discovery_history(kind,record_id,record) VALUES(?,?,?)")
        .bind(kind)
        .bind(id)
        .bind(serde_json::to_string(&e)?)
        .execute(&mut **tx)
        .await?;
    event(tx, "discovery", id).await
}
impl Store {
    pub async fn migrate_discovery(&self) -> Result<()> {
        sqlx::raw_sql(include_str!("schema.sql"))
            .execute(&self.pool)
            .await?;
        Ok(())
    }
    pub async fn discovery_history(
        &self,
        kind: &str,
        id: &str,
        q: DiscoveryQuery,
        access: &Access,
    ) -> Result<DiscoveryHistoryPage> {
        let (offset, limit) = page(&q)?;
        let mut tx = self.pool.begin().await?;
        let sources = match kind {
            "cue" => cues::raw(&mut tx, id).await?.source_ids,
            "gap" => gaps::raw(&mut tx, id).await?.source_ids,
            _ => bail!("invalid discovery history kind"),
        };
        permit(&mut tx, &sources, access).await?;
        let total: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM discovery_history WHERE kind=? AND record_id=?",
        )
        .bind(kind)
        .bind(id)
        .fetch_one(&mut *tx)
        .await?;
        let rows=sqlx::query("SELECT sequence,record FROM discovery_history WHERE kind=? AND record_id=? ORDER BY sequence DESC LIMIT ? OFFSET ?").bind(kind).bind(id).bind(limit as i64).bind(offset as i64).fetch_all(&mut *tx).await?;
        let mut items = vec![];
        for row in rows {
            let mut e: DiscoveryHistoryItem = serde_json::from_str(row.get("record"))?;
            e.sequence = row.get("sequence");
            items.push(e);
            if serde_json::to_vec(&items)?.len() > 300000 {
                if items.len() > 1 {
                    items.pop();
                }
                break;
            }
        }
        let next_offset = (offset + items.len() < total as usize).then_some(offset + items.len());
        tx.commit().await?;
        Ok(DiscoveryHistoryPage {
            items,
            total: total as usize,
            offset,
            next_offset,
        })
    }
}
