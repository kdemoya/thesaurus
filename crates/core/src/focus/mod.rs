//! Explainable local priorities and explicit waiting-for records. No model or outbound messages.
pub(crate) mod priorities;
pub(crate) mod waiting;
use crate::{
    Store,
    continuity::{
        documents, event, operation, permit, project_sources, remember, tasks, watermarks,
    },
    store::{now, validate_text},
};
use anyhow::{Context, Result, bail};
use chrono::{NaiveDate, Utc};
use sqlx::{Row, Sqlite, Transaction};
use thesaurus_contracts::*;

fn today() -> NaiveDate {
    Utc::now().date_naive()
}
fn date(value: &str) -> Result<NaiveDate> {
    let date =
        NaiveDate::parse_from_str(value, "%Y-%m-%d").context("invalid date; use YYYY-MM-DD")?;
    if date.format("%Y-%m-%d").to_string() != value {
        bail!("invalid date; use YYYY-MM-DD");
    }
    Ok(date)
}
fn optional_text(value: &str, max: usize, name: &str) -> Result<()> {
    if value.len() > max || value.contains('\0') {
        bail!("invalid {name}");
    }
    Ok(())
}
async fn project_access(
    tx: &mut Transaction<'_, Sqlite>,
    id: &str,
    access: &Access,
) -> Result<Vec<String>> {
    let scope = project_sources(tx, id).await?;
    permit(tx, &scope, access).await?;
    Ok(scope)
}
impl Store {
    pub async fn migrate_focus(&self) -> Result<()> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        sqlx::raw_sql(include_str!("schema.sql"))
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(())
    }
}
