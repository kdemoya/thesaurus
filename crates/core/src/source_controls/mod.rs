//! Local notification explanations, declared capture coverage and destination policies.
mod coverage;
mod notifications;
mod policy;
use crate::{
    Store,
    continuity::{event, operation, remember},
    store::{hash, now, validate_text},
};
use anyhow::{Context, Result, bail};
pub(crate) use policy::check_connection_tx;
use sqlx::{Row, Sqlite, Transaction};
use thesaurus_contracts::*;

fn admin(access: &Access) -> Result<()> {
    if access.source_ids.is_some()
        || access.external
        || !access.allow_write
        || access.contributor != "You"
    {
        bail!("forbidden: source controls require the user");
    }
    Ok(())
}
async fn save(
    tx: &mut Transaction<'_, Sqlite>,
    table: &str,
    source: &str,
    version: i64,
    record: &impl serde::Serialize,
) -> Result<()> {
    // Table names only originate in the fixed call sites in this module.
    let json = serde_json::to_string(record)?;
    sqlx::query(&format!("INSERT INTO {table}(source_id,record) VALUES(?,?) ON CONFLICT(source_id) DO UPDATE SET record=excluded.record")).bind(source).bind(&json).execute(&mut **tx).await?;
    sqlx::query(
        "INSERT INTO source_control_history(kind,source_id,version,record) VALUES(?,?,?,?)",
    )
    .bind(table)
    .bind(source)
    .bind(version)
    .bind(json)
    .execute(&mut **tx)
    .await?;
    event(tx, "source_control", source).await
}
impl Store {
    pub async fn migrate_source_controls(&self) -> Result<()> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        sqlx::raw_sql("CREATE TABLE IF NOT EXISTS source_control_policy(source_id TEXT PRIMARY KEY REFERENCES sources(id),record TEXT NOT NULL); CREATE TABLE IF NOT EXISTS source_control_coverage(source_id TEXT PRIMARY KEY REFERENCES sources(id),record TEXT NOT NULL); CREATE TABLE IF NOT EXISTS source_control_history(kind TEXT NOT NULL,source_id TEXT NOT NULL,version INTEGER NOT NULL,record TEXT NOT NULL,PRIMARY KEY(kind,source_id,version)); CREATE TABLE IF NOT EXISTS source_control_notifications(id TEXT PRIMARY KEY,record TEXT NOT NULL); CREATE TABLE IF NOT EXISTS source_control_mutes(id TEXT PRIMARY KEY,record TEXT NOT NULL);").execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(())
    }
}
