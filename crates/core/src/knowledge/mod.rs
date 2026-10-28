//! Explicit dated claims, evidence panels and user curated identity records.
mod answers;
mod claims;
mod people;
use crate::{
    Store,
    continuity::{event, operation, permit, project_sources, remember},
    store::{now, validate_text},
};
use anyhow::{Context, Result, bail};
use chrono::{DateTime, NaiveDate, Utc};
use sqlx::{Row, Sqlite, Transaction};
use thesaurus_contracts::*;
fn date(s: &str) -> Result<NaiveDate> {
    let d = NaiveDate::parse_from_str(s, "%Y-%m-%d").context("invalid date; use YYYY-MM-DD")?;
    if d.to_string() != s {
        bail!("invalid date; use YYYY-MM-DD");
    }
    Ok(d)
}
fn interval(from: &str, until: Option<&str>) -> Result<()> {
    let from = date(from)?;
    if let Some(to) = until {
        if date(to)? <= from {
            bail!("invalid: valid_until is exclusive and must follow valid_from");
        }
    }
    Ok(())
}
fn optional(s: &str, max: usize) -> Result<()> {
    if s.len() > max || s.contains('\0') {
        bail!("invalid knowledge note");
    }
    Ok(())
}
fn page(q: &KnowledgeQuery) -> Result<(usize, usize)> {
    let o = q.offset.unwrap_or(0) as usize;
    let l = q.limit.unwrap_or(25) as usize;
    if o > 100000 || !(1..=50).contains(&l) {
        bail!("invalid knowledge page");
    }
    Ok((o, l))
}
async fn citations(
    tx: &mut Transaction<'_, Sqlite>,
    project: &str,
    ev: &[Evidence],
) -> Result<Vec<String>> {
    if ev.is_empty() || ev.len() > 6 {
        bail!("invalid: choose 1–6 exact citations");
    }
    let p = project_sources(tx, project).await?;
    let mut ids = vec![];
    for e in ev {
        if e.line < 1 || e.quote.trim().is_empty() || e.quote.len() > 2000 {
            bail!("invalid: evidence needs nonempty exact lines up to 2000 bytes");
        }
        let row=sqlx::query("SELECT d.source_id,r.body FROM documents d JOIN revisions r ON r.document_id=d.id WHERE d.id=? AND r.revision=?").bind(&e.document_id).bind(e.revision).fetch_optional(&mut **tx).await?.context("invalid evidence revision")?;
        let source: String = row.get("source_id");
        if !p.contains(&source)
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
    let p = project_sources(tx, project).await?;
    let ids = sqlx::query("SELECT id,allow_external FROM sources")
        .fetch_all(&mut **tx)
        .await?
        .into_iter()
        .filter(|r| access.permits(r.get("id"), r.get("allow_external")))
        .map(|r| r.get::<String, _>("id"))
        .collect::<Vec<_>>();
    if !p.iter().any(|s| ids.contains(s)) {
        bail!("record not found");
    }
    Ok(ids)
}
fn terms(s: &str) -> Vec<String> {
    let stop = [
        "the", "and", "for", "that", "this", "with", "what", "when", "where", "which", "who",
        "why", "how", "does", "have", "has", "are", "was", "will", "would", "could", "should",
        "from", "into", "about", "our", "can", "not", "need", "know", "please", "any", "there",
        "today", "date",
    ];
    let mut terms = s
        .to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|s| s.chars().count() > 2 && !stop.contains(s))
        .map(String::from)
        .collect::<Vec<_>>();
    terms.sort();
    terms.dedup();
    terms.truncate(30);
    terms
}
impl Store {
    pub async fn migrate_knowledge(&self) -> Result<()> {
        sqlx::raw_sql(include_str!("schema.sql"))
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}
