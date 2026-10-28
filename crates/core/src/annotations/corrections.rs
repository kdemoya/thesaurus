use super::*;
use crate::models::Candidate;
use serde_json::json;

async fn rules(tx: &mut Transaction<'_, Sqlite>, source: &str) -> Result<CorrectionSet> {
    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM sources WHERE id=?)")
        .bind(source)
        .fetch_one(&mut **tx)
        .await?;
    if !exists {
        bail!("record not found");
    }
    let value: Option<String> =
        sqlx::query_scalar("SELECT record FROM annotation_corrections WHERE source_id=?")
            .bind(source)
            .fetch_optional(&mut **tx)
            .await?;
    Ok(match value {
        Some(s) => serde_json::from_str(&s)?,
        None => CorrectionSet {
            source_id: source.into(),
            version: 0,
            rules: vec![],
            contributor: "You".into(),
            updated_at: None,
        },
    })
}
fn normalized(
    title: &str,
    owner: &Option<String>,
    set: &CorrectionSet,
) -> Result<(String, Option<String>)> {
    let mut terms = set
        .rules
        .iter()
        .filter(|r| r.kind == "term")
        .collect::<Vec<_>>();
    terms.sort_by_key(|r| std::cmp::Reverse(r.from.len()));
    let title = if terms.is_empty() {
        title.to_string()
    } else {
        let pattern = terms
            .iter()
            .map(|r| regex::escape(&r.from))
            .collect::<Vec<_>>()
            .join("|");
        let re = regex::Regex::new(&pattern)?;
        re.replace_all(title, |caps: &regex::Captures<'_>| {
            terms
                .iter()
                .find(|r| r.from == caps[0])
                .map(|r| r.to.as_str())
                .unwrap_or("")
        })
        .into_owned()
    };
    let owner = owner.as_ref().map(|o| {
        set.rules
            .iter()
            .find(|r| r.kind == "speaker" && r.from == *o)
            .map(|r| r.to.clone())
            .unwrap_or_else(|| o.clone())
    });
    Ok((title, owner))
}
async fn preview(tx: &mut Transaction<'_, Sqlite>, source: &str) -> Result<CorrectionPreview> {
    let set = rules(tx, source).await?;
    let rows=sqlx::query("SELECT t.*,d.revision AS current_revision FROM tasks t JOIN documents d ON d.id=t.document_id WHERE d.source_id=? AND t.status IN ('candidate','accepted','completed') ORDER BY t.id").bind(source).fetch_all(&mut **tx).await?;
    let mut items = vec![];
    let mut actionable_count = 0;
    let mut protected_count = 0;
    for row in &rows {
        let task = crate::store::task_row(row);
        let baseline =
            sqlx::query("SELECT title,owner FROM annotation_task_baselines WHERE task_id=?")
                .bind(&task.id)
                .fetch_optional(&mut **tx)
                .await?;
        let pending = if baseline.is_none() {
            sqlx::query("SELECT title,owner FROM annotation_candidate_inputs WHERE document_id=? AND revision=? AND line=? AND normalized_title=?").bind(&task.evidence.document_id).bind(task.evidence.revision).bind(task.evidence.line).bind(&task.title).fetch_optional(&mut **tx).await?
        } else {
            None
        };
        let base = baseline.or(pending);
        let base_title = base
            .as_ref()
            .map(|r| r.get::<String, _>("title"))
            .unwrap_or_else(|| task.title.clone());
        let base_owner = base
            .as_ref()
            .map(|r| r.get::<Option<String>, _>("owner"))
            .unwrap_or_else(|| task.owner.clone());
        let (title, owner) = normalized(&base_title, &base_owner, &set)?;
        if title == task.title && owner == task.owner {
            continue;
        }
        let mut reason = if task.status != "candidate" {
            "Reviewed tasks remain unchanged".into()
        } else if task.evidence.revision != row.get::<i64, _>("current_revision") {
            "Historical source revision remains unchanged".into()
        } else {
            String::new()
        };
        if title.trim().is_empty()
            || title.len() > 500
            || owner.as_ref().is_some_and(|o| o.len() > 100)
        {
            reason = "Corrected fields exceed extraction limits".into();
        }
        if rows.iter().any(|other| {
            other.get::<String, _>("id") != task.id
                && other.get::<String, _>("document_id") == task.evidence.document_id
                && other.get::<i64, _>("revision") == task.evidence.revision
                && other.get::<i64, _>("line") == task.evidence.line
                && other.get::<String, _>("title") == title
        }) {
            reason =
                "An extraction with the corrected title already exists on this source line".into();
        }
        let collision:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM tasks WHERE document_id=? AND revision=? AND line=? AND title=? AND id!=?)").bind(&task.evidence.document_id).bind(task.evidence.revision).bind(task.evidence.line).bind(&title).bind(&task.id).fetch_one(&mut **tx).await?;
        if collision {
            reason =
                "An extraction with the corrected title already exists on this source line".into();
        }
        let actionable = reason.is_empty();
        if actionable {
            actionable_count += 1;
        } else {
            protected_count += 1;
        }
        items.push(CorrectionImpact {
            task,
            corrected_title: title,
            corrected_owner: owner,
            actionable,
            reason,
        });
    }
    // Two changes converging on the same unique key cannot both be applied.
    let keys = items
        .iter()
        .filter(|i| i.actionable)
        .map(|i| {
            (
                i.task.evidence.document_id.clone(),
                i.task.evidence.revision,
                i.task.evidence.line,
                i.corrected_title.clone(),
            )
        })
        .collect::<Vec<_>>();
    for item in &mut items {
        let key = (
            item.task.evidence.document_id.clone(),
            item.task.evidence.revision,
            item.task.evidence.line,
            item.corrected_title.clone(),
        );
        if item.actionable && keys.iter().filter(|k| **k == key).count() > 1 {
            item.actionable = false;
            item.reason = "Multiple extractions would converge on the same title".into();
            actionable_count -= 1;
            protected_count += 1;
        }
    }
    if items.len() > 500 || serde_json::to_vec(&items)?.len() > 600000 {
        bail!(
            "invalid: correction preview exceeds 500 impacts / 600 KB; narrow the correction rules"
        );
    }
    let signature = hash(&serde_json::to_string(&json!({"set":set,"items":items}))?);
    Ok(CorrectionPreview {
        source_id: source.into(),
        signature,
        corrections_version: set.version,
        items,
        actionable_count,
        protected_count,
    })
}
async fn audit(
    tx: &mut Transaction<'_, Sqlite>,
    source: &str,
    kind: &str,
    detail: serde_json::Value,
) -> Result<()> {
    sqlx::query("INSERT INTO annotation_correction_history(source_id,kind,contributor,created_at,detail) VALUES(?,?,'You',?,?)").bind(source).bind(kind).bind(now()).bind(serde_json::to_string(&detail)?).execute(&mut **tx).await?;
    Ok(())
}
impl Store {
    pub async fn correction_set(&self, source: &str, access: &Access) -> Result<CorrectionSet> {
        let mut tx = self.pool.begin().await?;
        permit(&mut tx, &[source.into()], access).await?;
        let set = rules(&mut tx, source).await?;
        tx.commit().await?;
        Ok(set)
    }
    pub async fn set_corrections(
        &self,
        source: &str,
        req: CorrectionsRequest,
    ) -> Result<CorrectionSet> {
        if req.rules.len() > 100 {
            bail!("invalid: at most 100 source correction rules");
        }
        let mut seen = std::collections::HashSet::new();
        for r in &req.rules {
            if !["speaker", "term"].contains(&r.kind.as_str()) {
                bail!("invalid correction kind");
            }
            validate_text(&r.from, 100, "original alias")?;
            validate_text(&r.to, 100, "corrected alias")?;
            if r.from == r.to || !seen.insert((&r.kind, &r.from)) {
                bail!("invalid: duplicate or unchanged alias");
            }
        }
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let (digest, old) = operation(
            &mut tx,
            "corrections-set",
            source,
            &req.idempotency_key,
            &req,
        )
        .await?;
        if old.is_none() {
            let old = rules(&mut tx, source).await?;
            if old.version != req.expected_version {
                bail!("conflict: correction rules changed; refresh");
            }
            let set = CorrectionSet {
                source_id: source.into(),
                version: old.version + 1,
                rules: req.rules,
                contributor: "You".into(),
                updated_at: Some(now()),
            };
            sqlx::query("INSERT INTO annotation_corrections(source_id,record) VALUES(?,?) ON CONFLICT(source_id) DO UPDATE SET record=excluded.record").bind(source).bind(serde_json::to_string(&set)?).execute(&mut *tx).await?;
            audit(&mut tx, source, "rules", json!({"before":old,"after":set})).await?;
            event(&mut tx, "correction", source).await?;
            remember(&mut tx, &req.idempotency_key, &digest, source).await?;
        }
        tx.commit().await?;
        self.correction_set(source, &Access::admin()).await
    }
    pub async fn correction_preview(
        &self,
        source: &str,
        access: &Access,
    ) -> Result<CorrectionPreview> {
        let mut tx = self.pool.begin().await?;
        permit(&mut tx, &[source.into()], access).await?;
        let p = preview(&mut tx, source).await?;
        tx.commit().await?;
        Ok(p)
    }
    pub async fn apply_corrections(
        &self,
        source: &str,
        req: CorrectionApplyRequest,
    ) -> Result<CorrectionResult> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let (digest, old) = operation(
            &mut tx,
            "corrections-apply",
            source,
            &req.idempotency_key,
            &req,
        )
        .await?;
        let id = if let Some(id) = old {
            id
        } else {
            let p = preview(&mut tx, source).await?;
            if p.signature != req.signature {
                bail!("conflict: rules, source or affected extractions changed; preview again");
            }
            for impact in p.items.iter().filter(|i| i.actionable) {
                let pending=sqlx::query("SELECT title,owner FROM annotation_candidate_inputs WHERE document_id=? AND revision=? AND line=? AND normalized_title=?").bind(&impact.task.evidence.document_id).bind(impact.task.evidence.revision).bind(impact.task.evidence.line).bind(&impact.task.title).fetch_optional(&mut *tx).await?;
                let title = pending
                    .as_ref()
                    .map(|r| r.get::<String, _>("title"))
                    .unwrap_or_else(|| impact.task.title.clone());
                let owner = pending
                    .as_ref()
                    .map(|r| r.get::<Option<String>, _>("owner"))
                    .unwrap_or_else(|| impact.task.owner.clone());
                sqlx::query("INSERT OR IGNORE INTO annotation_task_baselines(task_id,title,owner) VALUES(?,?,?)").bind(&impact.task.id).bind(title).bind(owner).execute(&mut *tx).await?;
                let changed=sqlx::query("UPDATE tasks SET title=?,owner=?,version=version+1,updated_at=? WHERE id=? AND version=? AND status='candidate'").bind(&impact.corrected_title).bind(&impact.corrected_owner).bind(now()).bind(&impact.task.id).bind(impact.task.version).execute(&mut *tx).await?.rows_affected();
                if changed != 1 {
                    bail!("conflict: extraction changed while applying corrections");
                }
                let after_row = sqlx::query("SELECT * FROM tasks WHERE id=?")
                    .bind(&impact.task.id)
                    .fetch_one(&mut *tx)
                    .await?;
                let after = crate::store::task_row(&after_row);
                crate::insights::record_task_transition_tx(
                    &mut tx,
                    Some(&impact.task),
                    &after,
                    "user_source_correction",
                )
                .await?;
            }
            let id = uuid::Uuid::new_v4().to_string();
            let result = CorrectionResult {
                source_id: source.into(),
                applied: p.actionable_count,
                protected: p.protected_count,
                receipt_id: id.clone(),
                created_at: now(),
            };
            audit(
                &mut tx,
                source,
                "recompute",
                json!({"receipt":result,"preview":p}),
            )
            .await?;
            sqlx::query("INSERT INTO annotation_recomputations(id,record) VALUES(?,?)")
                .bind(&id)
                .bind(serde_json::to_string(&result)?)
                .execute(&mut *tx)
                .await?;
            event(&mut tx, "correction", source).await?;
            remember(&mut tx, &req.idempotency_key, &digest, &id).await?;
            id
        };
        let json: String =
            sqlx::query_scalar("SELECT record FROM annotation_recomputations WHERE id=?")
                .bind(id)
                .fetch_one(&mut *tx)
                .await?;
        let result = serde_json::from_str(&json)?;
        tx.commit().await?;
        Ok(result)
    }
    /// Normalize derived fields only. The original document and candidate quote stay unchanged.
    pub async fn normalize_candidates(
        &self,
        doc: &Document,
        candidates: Vec<Candidate>,
    ) -> Result<Vec<Candidate>> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let set = rules(&mut tx, &doc.source_id).await?;
        let mut result = vec![];
        for mut c in candidates {
            let (title, owner) = normalized(&c.title, &c.owner, &set)?;
            validate_text(&title, 500, "corrected candidate title")?;
            if owner.as_ref().is_some_and(|o| o.len() > 100) {
                bail!("invalid corrected owner");
            }
            sqlx::query("INSERT OR IGNORE INTO annotation_candidate_inputs(document_id,revision,line,normalized_title,title,owner) VALUES(?,?,?,?,?,?)").bind(&doc.id).bind(doc.revision).bind(c.line).bind(&title).bind(&c.title).bind(&c.owner).execute(&mut *tx).await?;
            c.title = title;
            c.owner = owner;
            result.push(c);
        }
        tx.commit().await?;
        Ok(result)
    }
    pub async fn correction_history(
        &self,
        source: &str,
        q: AnnotationQuery,
        access: &Access,
    ) -> Result<CorrectionAuditPage> {
        let (offset, limit) = page(&q)?;
        let mut tx = self.pool.begin().await?;
        permit(&mut tx, &[source.into()], access).await?;
        let total: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM annotation_correction_history WHERE source_id=?",
        )
        .bind(source)
        .fetch_one(&mut *tx)
        .await?;
        let rows=sqlx::query("SELECT * FROM annotation_correction_history WHERE source_id=? ORDER BY id DESC LIMIT ? OFFSET ?").bind(source).bind(limit as i64).bind(offset as i64).fetch_all(&mut *tx).await?;
        let mut items = vec![];
        for row in rows {
            let item = CorrectionAudit {
                id: row.get("id"),
                source_id: source.into(),
                kind: row.get("kind"),
                contributor: row.get("contributor"),
                created_at: row.get("created_at"),
                detail: serde_json::from_str(row.get("detail"))?,
            };
            items.push(item);
            if serde_json::to_vec(&items)?.len() > 700000 {
                if items.len() > 1 {
                    items.pop();
                }
                break;
            }
        }
        let next_offset = (offset + items.len() < total as usize).then_some(offset + items.len());
        tx.commit().await?;
        Ok(CorrectionAuditPage {
            items,
            total: total as usize,
            offset,
            next_offset,
        })
    }
}
