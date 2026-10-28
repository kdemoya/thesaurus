//! Reviewable memory overlays. Originals and every decision remain immutable.
use crate::{
    Store,
    continuity::{event, permit},
    store::{hash, now, observation_row, validate_text},
};
use anyhow::{Context, Result, bail};
use serde::{Serialize, de::DeserializeOwned};
use sqlx::{Row, Sqlite, Transaction};
use thesaurus_contracts::*;

fn admin(access: &Access) -> Result<()> {
    if access.source_ids.is_some()
        || access.external
        || !access.allow_write
        || access.contributor != "You"
    {
        bail!("forbidden: memory decisions require the user");
    }
    Ok(())
}
async fn original(
    tx: &mut Transaction<'_, Sqlite>,
    id: &str,
    access: &Access,
) -> Result<Observation> {
    let row = sqlx::query("SELECT * FROM observations WHERE id=?")
        .bind(id)
        .fetch_optional(&mut **tx)
        .await?
        .context("record not found")?;
    let item = observation_row(&row)?;
    permit(tx, &item.source_ids, access).await?;
    if item.source_ids.is_empty()
        && access.source_ids.is_some()
        && item.contributor != access.contributor
    {
        bail!("record not found");
    }
    Ok(item)
}
fn initial(item: &Observation) -> MemoryRevision {
    MemoryRevision {
        version: 0,
        body: item.body.clone(),
        status: "active".into(),
        action: "contributed".into(),
        reason: "Original observation; source citations are retained below.".into(),
        contributor: item.contributor.clone(),
        created_at: item.created_at.clone(),
    }
}
async fn current(tx: &mut Transaction<'_, Sqlite>, item: &Observation) -> Result<MemoryRevision> {
    let record:Option<String>=sqlx::query_scalar("SELECT record FROM memory_review_revisions WHERE observation_id=? ORDER BY version DESC LIMIT 1").bind(&item.id).fetch_optional(&mut **tx).await?;
    Ok(match record {
        Some(s) => serde_json::from_str(&s)?,
        None => initial(item),
    })
}
async fn append(
    tx: &mut Transaction<'_, Sqlite>,
    id: &str,
    revision: &MemoryRevision,
) -> Result<()> {
    sqlx::query("INSERT INTO memory_review_revisions(observation_id,version,record) VALUES(?,?,?)")
        .bind(id)
        .bind(revision.version)
        .bind(serde_json::to_string(revision)?)
        .execute(&mut **tx)
        .await?;
    event(tx, "memory_review", id).await
}
pub(crate) async fn suppress_memory_tx(
    tx: &mut Transaction<'_, Sqlite>,
    id: &str,
    reason: &str,
) -> Result<()> {
    let item = original(tx, id, &Access::admin()).await?;
    let mut head = current(tx, &item).await?;
    if head.status == "suppressed" {
        return Ok(());
    }
    head.version += 1;
    head.status = "suppressed".into();
    head.action = "superseded".into();
    head.reason = reason.into();
    head.contributor = "You".into();
    head.created_at = now();
    append(tx, id, &head).await
}
pub(crate) async fn effective_observations_tx(
    tx: &mut Transaction<'_, Sqlite>,
    items: Vec<Observation>,
) -> Result<Vec<Observation>> {
    let mut result = Vec::new();
    for mut item in items {
        let head = current(tx, &item).await?;
        if head.status == "suppressed" {
            continue;
        }
        if head.version > 0 {
            item.body = head.body;
            item.contributor = format!("{} (originally {})", head.contributor, item.contributor);
        }
        result.push(item);
    }
    Ok(result)
}
async fn replay<T: DeserializeOwned>(
    tx: &mut Transaction<'_, Sqlite>,
    key: &str,
    kind: &str,
    target: &str,
    req: &impl Serialize,
) -> Result<(String, Option<T>)> {
    validate_text(key, 200, "idempotency key")?;
    let digest = hash(&format!("{kind}:{target}:{}", serde_json::to_string(req)?));
    let old = sqlx::query("SELECT digest,response FROM memory_review_operations WHERE key=?")
        .bind(key)
        .fetch_optional(&mut **tx)
        .await?;
    if let Some(row) = old {
        if row.get::<String, _>("digest") != digest {
            bail!("conflict: idempotency key has different content");
        }
        return Ok((digest, Some(serde_json::from_str(row.get("response"))?)));
    }
    Ok((digest, None))
}
async fn receipt(
    tx: &mut Transaction<'_, Sqlite>,
    key: &str,
    digest: &str,
    response: &impl Serialize,
) -> Result<()> {
    sqlx::query("INSERT INTO memory_review_operations(key,digest,response) VALUES(?,?,?)")
        .bind(key)
        .bind(digest)
        .bind(serde_json::to_string(response)?)
        .execute(&mut **tx)
        .await?;
    Ok(())
}
async fn inspect(tx: &mut Transaction<'_, Sqlite>, item: Observation) -> Result<MemoryInspection> {
    let mut history = Vec::new();
    for row in sqlx::query("SELECT record FROM memory_review_revisions WHERE observation_id=? ORDER BY version DESC LIMIT 30").bind(&item.id).fetch_all(&mut **tx).await? { history.push(serde_json::from_str(row.get("record"))?); }
    let head = current(tx, &item).await?;
    let history_count = head.version + 1;
    if history.len() < 30 {
        history.push(initial(&item));
    }
    let mut stale = false;
    for e in &item.evidence {
        let revision: Option<i64> = sqlx::query_scalar("SELECT revision FROM documents WHERE id=?")
            .bind(&e.document_id)
            .fetch_optional(&mut **tx)
            .await?;
        stale |= revision != Some(e.revision);
    }
    let mut result = MemoryInspection {
        original: item,
        current: head,
        history,
        history_count,
        stale_evidence: stale,
    };
    while serde_json::to_vec(&result)?.len() > 100000 && !result.history.is_empty() {
        result.history.pop();
    }
    if serde_json::to_vec(&result)?.len() > 100000 {
        bail!(
            "invalid: original evidence exceeds memory inspection response budget; read the memory revision and its source documents separately"
        );
    }
    Ok(result)
}
async fn conflict(
    tx: &mut Transaction<'_, Sqlite>,
    id: &str,
    access: &Access,
) -> Result<MemoryConflict> {
    let json: String = sqlx::query_scalar("SELECT record FROM memory_review_conflicts WHERE id=?")
        .bind(id)
        .fetch_optional(&mut **tx)
        .await?
        .context("record not found")?;
    let c: MemoryConflict = serde_json::from_str(&json)?;
    permit(tx, &c.source_ids, access).await?;
    for claim in &c.claims {
        original(tx, &claim.observation_id, access).await?;
    }
    Ok(c)
}
async fn save_conflict(tx: &mut Transaction<'_, Sqlite>, c: &MemoryConflict) -> Result<()> {
    let record = serde_json::to_string(c)?;
    sqlx::query("INSERT INTO memory_review_conflicts(id,record) VALUES(?,?) ON CONFLICT(id) DO UPDATE SET record=excluded.record").bind(&c.id).bind(&record).execute(&mut **tx).await?;
    sqlx::query(
        "INSERT INTO memory_review_conflict_history(conflict_id,version,record) VALUES(?,?,?)",
    )
    .bind(&c.id)
    .bind(c.version)
    .bind(record)
    .execute(&mut **tx)
    .await?;
    event(tx, "memory_conflict", &c.id).await
}
/// A caller-owned transaction pins the same snapshot as the rest of an export.
pub(crate) async fn memory_export_tx(
    tx: &mut Transaction<'_, Sqlite>,
    source_ids: &[String],
    access: &Access,
) -> Result<Vec<serde_json::Value>> {
    permit(tx, source_ids, access).await?;
    let mut result = Vec::new();
    let mut exported = Vec::new();
    for row in sqlx::query("SELECT * FROM observations ORDER BY created_at,id")
        .fetch_all(&mut **tx)
        .await?
    {
        let item = observation_row(&row)?;
        if item.source_ids.is_empty() || !item.source_ids.iter().all(|id| source_ids.contains(id)) {
            continue;
        }
        permit(tx, &item.source_ids, access).await?;
        let mut history = vec![initial(&item)];
        for r in sqlx::query(
            "SELECT record FROM memory_review_revisions WHERE observation_id=? ORDER BY version",
        )
        .bind(&item.id)
        .fetch_all(&mut **tx)
        .await?
        {
            history.push(serde_json::from_str::<MemoryRevision>(r.get("record"))?);
        }
        let effective = history.last().cloned().context("missing original memory")?;
        exported.push(item.id.clone());
        result.push(serde_json::json!({"kind":"observation","original":item,"effective":effective,"history":history}));
    }
    for row in sqlx::query("SELECT record FROM memory_review_conflicts ORDER BY id")
        .fetch_all(&mut **tx)
        .await?
    {
        let c: MemoryConflict = serde_json::from_str(row.get("record"))?;
        if !c
            .claims
            .iter()
            .all(|claim| exported.contains(&claim.observation_id))
        {
            continue;
        }
        let mut history = Vec::new();
        for r in sqlx::query("SELECT record FROM memory_review_conflict_history WHERE conflict_id=? ORDER BY version").bind(&c.id).fetch_all(&mut **tx).await?{history.push(serde_json::from_str::<MemoryConflict>(r.get("record"))?);}
        result.push(serde_json::json!({"kind":"conflict","current":c,"history":history}));
    }
    Ok(result)
}
impl Store {
    pub async fn migrate_memory_review(&self) -> Result<()> {
        sqlx::raw_sql("CREATE TABLE IF NOT EXISTS memory_review_revisions(observation_id TEXT NOT NULL REFERENCES observations(id), version INTEGER NOT NULL, record TEXT NOT NULL, PRIMARY KEY(observation_id,version)); CREATE TABLE IF NOT EXISTS memory_review_conflicts(id TEXT PRIMARY KEY,record TEXT NOT NULL); CREATE TABLE IF NOT EXISTS memory_review_conflict_history(conflict_id TEXT NOT NULL REFERENCES memory_review_conflicts(id),version INTEGER NOT NULL,record TEXT NOT NULL,PRIMARY KEY(conflict_id,version)); CREATE TABLE IF NOT EXISTS memory_review_operations(key TEXT PRIMARY KEY,digest TEXT NOT NULL,response TEXT NOT NULL);").execute(&self.pool).await?;
        Ok(())
    }
    pub async fn inspect_memory(&self, id: &str, access: &Access) -> Result<MemoryInspection> {
        let mut tx = self.pool.begin().await?;
        let item = original(&mut tx, id, access).await?;
        inspect(&mut tx, item).await
    }
    pub async fn memory_catalog(
        &self,
        offset: u32,
        access: &Access,
    ) -> Result<Vec<MemoryInspection>> {
        if offset > 10000 {
            bail!("invalid memory offset");
        }
        let mut tx = self.pool.begin().await?;
        let mut result = Vec::new();
        let mut skipped = 0;
        for row in sqlx::query("SELECT id FROM observations ORDER BY created_at DESC,id")
            .fetch_all(&mut *tx)
            .await?
        {
            let id: String = row.get("id");
            let item = match original(&mut tx, &id, access).await {
                Ok(item) => item,
                Err(e) if e.to_string() == "record not found" => continue,
                Err(e) => return Err(e),
            };
            if skipped < offset {
                skipped += 1;
                continue;
            }
            let mut inspection = inspect(&mut tx, item).await?;
            inspection.history.clear();
            result.push(inspection);
            if result.len() == 25 {
                break;
            }
        }
        Ok(result)
    }
    pub async fn memory_revision(
        &self,
        id: &str,
        version: i64,
        access: &Access,
    ) -> Result<MemoryRevision> {
        let mut tx = self.pool.begin().await?;
        let item = original(&mut tx, id, access).await?;
        if version == 0 {
            return Ok(initial(&item));
        }
        let json: String = sqlx::query_scalar(
            "SELECT record FROM memory_review_revisions WHERE observation_id=? AND version=?",
        )
        .bind(id)
        .bind(version)
        .fetch_optional(&mut *tx)
        .await?
        .context("record not found")?;
        Ok(serde_json::from_str(&json)?)
    }
    /// Apply current overlays everywhere existing memory is returned to agents or the UI.
    pub async fn effective_observations(
        &self,
        items: Vec<Observation>,
    ) -> Result<Vec<Observation>> {
        let mut tx = self.pool.begin().await?;
        effective_observations_tx(&mut tx, items).await
    }
    pub async fn revise_memory(
        &self,
        id: &str,
        req: MemoryReviseRequest,
        access: &Access,
    ) -> Result<MemoryInspection> {
        admin(access)?;
        validate_text(&req.reason, 2000, "correction reason")?;
        if !["correct", "suppress", "restore"].contains(&req.action.as_str()) {
            bail!("invalid memory action");
        }
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let (digest, old) =
            replay::<MemoryInspection>(&mut tx, &req.idempotency_key, "revise", id, &req).await?;
        if let Some(old) = old {
            return Ok(old);
        }
        let item = original(&mut tx, id, access).await?;
        let mut head = current(&mut tx, &item).await?;
        if head.version != req.expected_version {
            bail!("conflict: memory changed; inspect the latest version");
        }
        match req.action.as_str() {
            "correct" => {
                let body = req
                    .body
                    .as_deref()
                    .context("invalid: correction body required")?;
                validate_text(body, 16000, "corrected memory")?;
                head.body = body.into();
                head.status = "active".into();
                if req.restore_version.is_some() {
                    bail!("invalid restore version on correction");
                }
            }
            "suppress" => {
                if req.body.is_some() || req.restore_version.is_some() {
                    bail!("invalid suppress fields");
                }
                head.status = "suppressed".into();
            }
            "restore" => {
                if req.body.is_some() {
                    bail!("invalid body on restore");
                }
                let version = req
                    .restore_version
                    .context("invalid: restore version required")?;
                if version < 0 || version > head.version {
                    bail!("invalid restore version");
                }
                let source = if version == 0 {
                    initial(&item)
                } else {
                    let json:String=sqlx::query_scalar("SELECT record FROM memory_review_revisions WHERE observation_id=? AND version=?").bind(id).bind(version).fetch_one(&mut *tx).await?;
                    serde_json::from_str(&json)?
                };
                head.body = source.body;
                head.status = source.status;
            }
            _ => unreachable!(),
        }
        head.version += 1;
        head.action = req.action;
        head.reason = req.reason;
        head.contributor = "You".into();
        head.created_at = now();
        append(&mut tx, id, &head).await?;
        let result = inspect(&mut tx, item).await?;
        receipt(&mut tx, &req.idempotency_key, &digest, &result).await?;
        tx.commit().await?;
        Ok(result)
    }
    pub async fn create_memory_conflict(
        &self,
        req: MemoryConflictCreate,
        access: &Access,
    ) -> Result<MemoryConflict> {
        admin(access)?;
        validate_text(&req.reason, 2000, "conflict reason")?;
        let mut ids = req.observation_ids.clone();
        ids.sort();
        ids.dedup();
        if ids.len() < 2 || ids.len() > 8 {
            bail!("invalid: choose 2–8 distinct observations");
        }
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let (digest, old) =
            replay::<MemoryConflict>(&mut tx, &req.idempotency_key, "create_conflict", "", &req)
                .await?;
        if let Some(old) = old {
            return Ok(old);
        }
        let mut claims = Vec::new();
        let mut source_ids = Vec::new();
        for id in ids {
            let item = original(&mut tx, &id, access).await?;
            let head = current(&mut tx, &item).await?;
            if head.status == "suppressed" {
                bail!("invalid: suppressed memory cannot open a conflict");
            }
            source_ids.extend(item.source_ids.clone());
            claims.push(MemoryClaim {
                observation_id: id,
                version: head.version,
                body: head.body,
                contributed_at: item.created_at,
                contributor: item.contributor,
                source_ids: item.source_ids,
                evidence: item.evidence,
            });
        }
        source_ids.sort();
        source_ids.dedup();
        let stamp = now();
        let c = MemoryConflict {
            id: uuid::Uuid::new_v4().to_string(),
            version: 1,
            claims,
            source_ids,
            reason: req.reason,
            status: "pending".into(),
            decision: None,
            resolution_note: None,
            resolved_by: None,
            created_at: stamp.clone(),
            updated_at: stamp,
        };
        save_conflict(&mut tx, &c).await?;
        receipt(&mut tx, &req.idempotency_key, &digest, &c).await?;
        tx.commit().await?;
        Ok(c)
    }
    pub async fn memory_conflicts(&self, access: &Access) -> Result<Vec<MemoryConflict>> {
        let mut tx = self.pool.begin().await?;
        let mut result = Vec::new();
        for row in
            sqlx::query("SELECT id FROM memory_review_conflicts ORDER BY rowid DESC LIMIT 1000")
                .fetch_all(&mut *tx)
                .await?
        {
            let id: String = row.get("id");
            match conflict(&mut tx, &id, access).await {
                Ok(c) => {
                    result.push(c);
                    if result.len() >= 100 {
                        break;
                    }
                }
                Err(e) if e.to_string() == "record not found" => continue,
                Err(e) => return Err(e),
            }
        }
        Ok(result)
    }
    pub async fn memory_conflict(&self, id: &str, access: &Access) -> Result<MemoryConflict> {
        let mut tx = self.pool.begin().await?;
        conflict(&mut tx, id, access).await
    }
    pub async fn memory_conflict_history(
        &self,
        id: &str,
        offset: u32,
        access: &Access,
    ) -> Result<MemoryConflictHistory> {
        if offset > 10000 {
            bail!("invalid history offset");
        }
        let mut tx = self.pool.begin().await?;
        let c = conflict(&mut tx, id, access).await?;
        let mut records = Vec::new();
        for row in sqlx::query("SELECT record FROM memory_review_conflict_history WHERE conflict_id=? ORDER BY version DESC LIMIT 20 OFFSET ?").bind(id).bind(offset).fetch_all(&mut *tx).await?{records.push(serde_json::from_str(row.get("record"))?);}
        Ok(MemoryConflictHistory {
            records,
            total: c.version,
            offset,
        })
    }
    pub async fn resolve_memory_conflict(
        &self,
        id: &str,
        req: MemoryConflictResolve,
        access: &Access,
    ) -> Result<MemoryConflict> {
        admin(access)?;
        validate_text(&req.note, 2000, "resolution note")?;
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let (digest, old) =
            replay::<MemoryConflict>(&mut tx, &req.idempotency_key, "resolve_conflict", id, &req)
                .await?;
        if let Some(old) = old {
            return Ok(old);
        }
        let mut c = conflict(&mut tx, id, access).await?;
        if c.version != req.expected_version {
            bail!("conflict: review changed; refresh");
        }
        if req.decision == "reopen" {
            for claim in &mut c.claims {
                let item = original(&mut tx, &claim.observation_id, access).await?;
                let head = current(&mut tx, &item).await?;
                claim.version = head.version;
                claim.body = head.body;
            }
            c.status = "pending".into();
            c.decision = None;
        } else {
            if c.status != "pending" {
                bail!("conflict: resolution requires a pending conflict");
            }
            let keep = req.decision.strip_prefix("keep:");
            if req.decision != "keep_both"
                && req.decision != "suppress_all"
                && !keep.is_some_and(|id| c.claims.iter().any(|x| x.observation_id == id))
            {
                bail!("invalid resolution decision");
            }
            for claim in &c.claims {
                let item = original(&mut tx, &claim.observation_id, access).await?;
                let mut head = current(&mut tx, &item).await?;
                if head.version != claim.version {
                    bail!("conflict: a claim changed; reopen or create a fresh review");
                }
                let status = if req.decision == "suppress_all"
                    || keep.is_some_and(|id| id != claim.observation_id)
                {
                    "suppressed"
                } else {
                    "active"
                };
                if head.status != status {
                    head.version += 1;
                    head.status = status.into();
                    head.action = "conflict_resolution".into();
                    head.reason = format!("Conflict {id}: {}", req.note);
                    head.contributor = "You".into();
                    head.created_at = now();
                    append(&mut tx, &claim.observation_id, &head).await?;
                }
            }
            c.status = "resolved".into();
            c.decision = Some(req.decision.clone());
        }
        c.version += 1;
        c.resolution_note = Some(req.note);
        c.resolved_by = Some("You".into());
        c.updated_at = now();
        save_conflict(&mut tx, &c).await?;
        receipt(&mut tx, &req.idempotency_key, &digest, &c).await?;
        tx.commit().await?;
        Ok(c)
    }
}
