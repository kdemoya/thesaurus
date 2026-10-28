//! User annotations and reversible correction overlays never rewrite source revisions.
mod corrections;
mod notes;
use crate::{
    Store,
    continuity::{event, operation, permit, remember},
    store::{hash, now, validate_text},
};
use anyhow::{Context, Result, bail};
pub(crate) use notes::annotation_context_tx;
use sqlx::{Row, Sqlite, Transaction};
use thesaurus_contracts::*;
fn page(query: &AnnotationQuery) -> Result<(usize, usize)> {
    let o = query.offset.unwrap_or(0) as usize;
    let l = query.limit.unwrap_or(25) as usize;
    if o > 100000 || !(1..=50).contains(&l) {
        bail!("invalid annotation page");
    }
    Ok((o, l))
}
impl Store {
    pub async fn migrate_annotations(&self) -> Result<()> {
        sqlx::raw_sql(include_str!("schema.sql"))
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}
