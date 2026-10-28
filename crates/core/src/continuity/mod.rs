//! Local project continuity. Context snapshots and meeting baselines are derived records.
mod context;
mod meetings;
mod threads;

use crate::{
    Store,
    store::{hash, now, validate_text},
};
use anyhow::{Context, Result, bail};
use serde::Serialize;
use sqlx::{Row, Sqlite, Transaction};
use thesaurus_contracts::*;

pub(super) async fn operation(
    tx: &mut Transaction<'_, Sqlite>,
    kind: &str,
    target: &str,
    key: &str,
    req: &impl Serialize,
) -> Result<(String, Option<String>)> {
    validate_text(key, 200, "idempotency key")?;
    let digest = hash(&format!("{kind}:{target}:{}", serde_json::to_string(req)?));
    let old = sqlx::query("SELECT request_hash,result_id FROM workflow_operations WHERE key=?")
        .bind(key)
        .fetch_optional(&mut **tx)
        .await?;
    if let Some(old) = old {
        if old.get::<String, _>("request_hash") != digest {
            bail!("conflict: idempotency key used for a different operation");
        }
        return Ok((digest, Some(old.get("result_id"))));
    }
    Ok((digest, None))
}
pub(super) async fn remember(
    tx: &mut Transaction<'_, Sqlite>,
    key: &str,
    digest: &str,
    id: &str,
) -> Result<()> {
    sqlx::query("INSERT INTO workflow_operations(key,request_hash,result_id) VALUES(?,?,?)")
        .bind(key)
        .bind(digest)
        .bind(id)
        .execute(&mut **tx)
        .await?;
    Ok(())
}
pub(super) async fn event(tx: &mut Transaction<'_, Sqlite>, kind: &str, id: &str) -> Result<()> {
    sqlx::query("INSERT INTO events(kind,record_id) VALUES(?,?)")
        .bind(kind)
        .bind(id)
        .execute(&mut **tx)
        .await?;
    Ok(())
}
pub(super) fn normalize_sources(ids: &mut Vec<String>) -> Result<()> {
    ids.sort();
    ids.dedup();
    if ids.is_empty() || ids.len() > 50 {
        bail!("invalid: choose 1–50 sources");
    }
    Ok(())
}
pub(super) async fn permit(
    tx: &mut Transaction<'_, Sqlite>,
    ids: &[String],
    access: &Access,
) -> Result<()> {
    for id in ids {
        let allowed: Option<bool> =
            sqlx::query_scalar("SELECT allow_external FROM sources WHERE id=?")
                .bind(id)
                .fetch_optional(&mut **tx)
                .await?;
        if !allowed.is_some_and(|external| access.permits(id, external)) {
            bail!("record not found");
        }
    }
    Ok(())
}
pub(super) async fn project_sources(
    tx: &mut Transaction<'_, Sqlite>,
    id: &str,
) -> Result<Vec<String>> {
    let json: String = sqlx::query_scalar("SELECT source_ids FROM project_briefs WHERE id=?")
        .bind(id)
        .fetch_optional(&mut **tx)
        .await?
        .context("record not found")?;
    Ok(serde_json::from_str(&json)?)
}
pub(super) async fn documents(
    tx: &mut Transaction<'_, Sqlite>,
    ids: &[String],
) -> Result<Vec<Document>> {
    let mut result = vec![];
    for id in ids {
        for row in sqlx::query("SELECT * FROM documents WHERE source_id=? ORDER BY id")
            .bind(id)
            .fetch_all(&mut **tx)
            .await?
        {
            result.push(crate::store::document_row(&row));
        }
    }
    Ok(result)
}
pub(super) fn watermarks(docs: &[Document]) -> Vec<BriefSourceRevision> {
    docs.iter()
        .map(|d| BriefSourceRevision {
            source_id: d.source_id.clone(),
            document_id: d.id.clone(),
            title: d.title.clone(),
            revision: d.revision,
            content_hash: d.content_hash.clone(),
        })
        .collect()
}
pub(super) async fn tasks(
    tx: &mut Transaction<'_, Sqlite>,
    docs: &[Document],
) -> Result<Vec<Task>> {
    Ok(sqlx::query("SELECT * FROM tasks ORDER BY CASE status WHEN 'accepted' THEN 0 WHEN 'candidate' THEN 1 ELSE 2 END,due IS NULL,due,id").fetch_all(&mut **tx).await?.iter().map(crate::store::task_row).filter(|t|docs.iter().any(|d|d.id==t.evidence.document_id)).collect())
}
pub(super) async fn reviewed_brief(
    tx: &mut Transaction<'_, Sqlite>,
    project: &str,
    ids: &[String],
) -> Result<Option<ReviewedBrief>> {
    let scope = project_sources(tx, project).await?;
    if !scope.iter().all(|id| ids.contains(id)) {
        return Ok(None);
    }
    let record:Option<String>=sqlx::query_scalar("SELECT r.record FROM brief_revisions r JOIN project_briefs p ON p.id=r.brief_id AND p.accepted_revision=r.revision WHERE p.id=?").bind(project).fetch_optional(&mut **tx).await?;
    let Some(record) = record else {
        return Ok(None);
    };
    let rev: BriefRevision = serde_json::from_str(&record)?;
    let policies = crate::briefs::retrieval_policies_tx(tx, &scope).await?;
    let live = crate::briefs::live_policy_sources(&policies);
    if rev
        .sources
        .iter()
        .any(|source| live.contains(&source.source_id))
    {
        // Do not reuse an older accepted brief to bypass a current live-only policy.
        return Ok(None);
    }
    let old: Vec<_> = rev.sources.iter().map(|s| s.source_id.clone()).collect();
    let stale = !crate::briefs::changed_sources(tx, &rev.sources)
        .await?
        .is_empty()
        || rev.retrieval_policies != policies
        || rev.selected_source_ids != scope
        || scope
            .iter()
            .any(|id| !live.contains(id) && !old.contains(id));
    Ok(Some(ReviewedBrief {
        revision: rev.revision,
        stale,
        omitted_source_ids: rev.omitted_source_ids,
        omissions: rev.omissions,
        sections: rev
            .sections
            .into_iter()
            .map(|mut s| {
                s.stale = stale;
                s
            })
            .collect(),
    }))
}

impl Store {
    pub async fn migrate_continuity(&self) -> Result<()> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        sqlx::raw_sql(include_str!("schema.sql"))
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(())
    }
    /// Adding a source invalidates prior briefs, including their empty sections.
    pub async fn expand_project_sources(
        &self,
        id: &str,
        mut req: ProjectSourcesRequest,
    ) -> Result<ProjectBrief> {
        normalize_sources(&mut req.source_ids)?;
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let (digest, old) =
            operation(&mut tx, "project-sources", id, &req.idempotency_key, &req).await?;
        if old.is_none() {
            let old = project_sources(&mut tx, id).await?;
            let version: i64 = sqlx::query_scalar("SELECT version FROM project_briefs WHERE id=?")
                .bind(id)
                .fetch_one(&mut *tx)
                .await?;
            if version != req.expected_version {
                bail!("conflict: project changed; refresh before adding sources");
            }
            if !old.iter().all(|s| req.source_ids.contains(s)) {
                bail!(
                    "invalid: project sources can be added, not removed while derived history exists"
                );
            }
            permit(&mut tx, &req.source_ids, &Access::admin()).await?;
            sqlx::query(
                "UPDATE project_briefs SET source_ids=?,version=version+1,updated_at=? WHERE id=?",
            )
            .bind(serde_json::to_string(&req.source_ids)?)
            .bind(now())
            .bind(id)
            .execute(&mut *tx)
            .await?;
            remember(&mut tx, &req.idempotency_key, &digest, id).await?;
            event(&mut tx, "brief", id).await?;
        }
        tx.commit().await?;
        self.brief(id, &Access::admin()).await
    }
}
