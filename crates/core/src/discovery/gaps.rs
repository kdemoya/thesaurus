use super::*;
pub(super) async fn raw(tx: &mut Transaction<'_, Sqlite>, id: &str) -> Result<KnowledgeGap> {
    let s: String = sqlx::query_scalar("SELECT record FROM discovery_gaps WHERE id=?")
        .bind(id)
        .fetch_optional(&mut **tx)
        .await?
        .context("record not found")?;
    Ok(serde_json::from_str(&s)?)
}
pub(super) async fn save(
    tx: &mut Transaction<'_, Sqlite>,
    item: &KnowledgeGap,
    action: &str,
    contributor: &str,
    note: &str,
) -> Result<()> {
    if serde_json::to_vec(item)?.len() > 180000 {
        bail!("invalid: knowledge gap evidence limit reached");
    }
    sqlx::query("INSERT INTO discovery_gaps(id,project_id,record) VALUES(?,?,?) ON CONFLICT(id) DO UPDATE SET record=excluded.record").bind(&item.id).bind(&item.project_id).bind(serde_json::to_string(item)?).execute(&mut **tx).await?;
    history(tx, "gap", &item.id, action, contributor, note, item).await
}
async fn scope(
    tx: &mut Transaction<'_, Sqlite>,
    project: &str,
    ids: &mut Vec<String>,
) -> Result<()> {
    ids.sort();
    ids.dedup();
    let p = project_sources(tx, project).await?;
    if ids.is_empty() || ids.len() > 50 || !ids.iter().all(|s| p.contains(s)) {
        bail!("invalid: choose 1–50 existing project sources for revisiting this question");
    }
    Ok(())
}
impl Store {
    pub async fn knowledge_gap(&self, id: &str, access: &Access) -> Result<KnowledgeGap> {
        let mut tx = self.pool.begin().await?;
        let item = raw(&mut tx, id).await?;
        permit(&mut tx, &item.source_ids, access).await?;
        tx.commit().await?;
        Ok(item)
    }
    pub async fn knowledge_gaps(
        &self,
        project: &str,
        q: DiscoveryQuery,
        access: &Access,
    ) -> Result<GapPage> {
        let (offset, limit) = page(&q)?;
        let status = q.status.as_deref().unwrap_or("active");
        if ![
            "active",
            "open",
            "needs_review",
            "answered",
            "archived",
            "all",
        ]
        .contains(&status)
        {
            bail!("invalid knowledge gap status");
        }
        let mut tx = self.pool.begin().await?;
        let allowed = allowed(&mut tx, project, access).await?;
        let mut total = 0;
        let mut items = vec![];
        for row in sqlx::query("SELECT record FROM discovery_gaps WHERE project_id=? ORDER BY json_extract(record,'$.updated_at') DESC,id").bind(project).fetch_all(&mut *tx).await?{let item:KnowledgeGap=serde_json::from_str(row.get("record"))?;if !item.source_ids.iter().all(|s|allowed.contains(s)) || (status=="active"&&! ["open","needs_review"].contains(&item.status.as_str())) || (status!="active"&&status!="all"&&item.status!=status){continue;}if total>=offset&&items.len()<limit{items.push(item);}total+=1;}
        while serde_json::to_vec(&items)?.len() > 300000 {
            if items.pop().is_none() {
                break;
            }
        }
        let next_offset = (offset + items.len() < total).then_some(offset + items.len());
        tx.commit().await?;
        Ok(GapPage {
            project_id: project.into(),
            items,
            total,
            offset,
            next_offset,
        })
    }
    pub async fn create_knowledge_gap(&self, mut req: GapCreateRequest) -> Result<KnowledgeGap> {
        validate_text(&req.question, 1000, "knowledge gap question")?;
        validate_text(&req.missing_source, 1000, "missing source description")?;
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let (digest, old) =
            operation(&mut tx, "gap-create", "", &req.idempotency_key, &req).await?;
        let id = if let Some(id) = old {
            id
        } else {
            scope(&mut tx, &req.project_id, &mut req.required_source_ids).await?;
            let count: i64 =
                sqlx::query_scalar("SELECT COUNT(*) FROM discovery_gaps WHERE project_id=?")
                    .bind(&req.project_id)
                    .fetch_one(&mut *tx)
                    .await?;
            if count >= 1000 {
                bail!("invalid: project knowledge gap limit reached");
            }
            let docs = documents(&mut tx, &req.required_source_ids).await?;
            let item = KnowledgeGap {
                id: uuid::Uuid::new_v4().to_string(),
                project_id: req.project_id,
                question: req.question.trim().into(),
                missing_source: req.missing_source.trim().into(),
                required_source_ids: req.required_source_ids.clone(),
                source_ids: req.required_source_ids,
                status: "open".into(),
                answer: None,
                answer_evidence: vec![],
                version: 1,
                contributor: "You".into(),
                created_at: now(),
                updated_at: now(),
                sources: watermarks(&docs),
                suggestions: vec![],
                suggestions_omitted: 0,
            };
            save(&mut tx, &item, "created", "You", "").await?;
            remember(&mut tx, &req.idempotency_key, &digest, &item.id).await?;
            item.id
        };
        tx.commit().await?;
        self.knowledge_gap(&id, &Access::admin()).await
    }
    pub async fn update_knowledge_gap(
        &self,
        id: &str,
        req: GapUpdateRequest,
    ) -> Result<KnowledgeGap> {
        optional_note(&req.note)?;
        if ![
            "edit",
            "answer",
            "archive",
            "reopen",
            "accept_suggestion",
            "dismiss_suggestion",
        ]
        .contains(&req.action.as_str())
        {
            bail!("invalid knowledge gap action");
        }
        if req.action != "edit"
            && (req.question.is_some()
                || req.missing_source.is_some()
                || req.required_source_ids.is_some())
        {
            bail!("invalid: question and scope changes require edit");
        }
        if req.action != "answer" && (req.answer.is_some() || !req.evidence.is_empty()) {
            bail!("invalid: answer and evidence require answer action");
        }
        if !["accept_suggestion", "dismiss_suggestion"].contains(&req.action.as_str())
            && req.suggestion_id.is_some()
        {
            bail!("invalid: suggestion_id requires suggestion review action");
        }
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let (digest, old) =
            operation(&mut tx, "gap-update", id, &req.idempotency_key, &req).await?;
        if old.is_none() {
            let mut item = raw(&mut tx, id).await?;
            if item.version != req.expected_version {
                bail!("conflict: knowledge gap changed; refresh");
            }
            if ["answered", "archived"].contains(&item.status.as_str()) && req.action != "reopen" {
                bail!("conflict: reopen the knowledge gap before changing it");
            }
            match req.action.as_str() {
                "edit" => {
                    if let Some(q) = req.question {
                        validate_text(&q, 1000, "gap question")?;
                        item.question = q.trim().into();
                    }
                    if let Some(m) = req.missing_source {
                        validate_text(&m, 1000, "missing source description")?;
                        item.missing_source = m.trim().into();
                    }
                    if let Some(mut s) = req.required_source_ids {
                        scope(&mut tx, &item.project_id, &mut s).await?;
                        item.source_ids.extend(s.clone());
                        item.source_ids.sort();
                        item.source_ids.dedup();
                        item.required_source_ids = s;
                    }
                    item.sources.clear();
                }
                "answer" => {
                    let a = req
                        .answer
                        .as_deref()
                        .context("invalid: provide an answer")?;
                    validate_text(a, 4000, "gap answer")?;
                    let ids = citations(&mut tx, &item.project_id, &req.evidence, 1).await?;
                    item.source_ids.extend(ids);
                    item.source_ids.sort();
                    item.source_ids.dedup();
                    item.answer = Some(a.trim().into());
                    item.answer_evidence = req.evidence.clone();
                    item.status = "answered".into();
                }
                "archive" => item.status = "archived".into(),
                "reopen" => {
                    if !["answered", "archived"].contains(&item.status.as_str()) {
                        bail!("conflict: gap is already active");
                    }
                    item.status = "open".into();
                    item.answer = None;
                    item.answer_evidence.clear();
                    item.sources.clear();
                }
                "accept_suggestion" | "dismiss_suggestion" => {
                    let suggestion = req
                        .suggestion_id
                        .as_deref()
                        .context("invalid: choose a suggestion")?;
                    let s = item
                        .suggestions
                        .iter_mut()
                        .find(|s| s.id == suggestion)
                        .context("invalid suggestion id")?;
                    s.status = if req.action == "accept_suggestion" {
                        "accepted"
                    } else {
                        "dismissed"
                    }
                    .into();
                    sqlx::query("INSERT INTO discovery_suggestion_dispositions(gap_id,suggestion_id,status) VALUES(?,?,?) ON CONFLICT(gap_id,suggestion_id) DO UPDATE SET status=excluded.status").bind(&item.id).bind(&s.id).bind(&s.status).execute(&mut *tx).await?;
                    item.status = if item.suggestions.iter().any(|s| s.status == "pending") {
                        "needs_review"
                    } else {
                        "open"
                    }
                    .into();
                }
                _ => {}
            }
            item.version += 1;
            item.updated_at = now();
            save(&mut tx, &item, &req.action, "You", &req.note).await?;
            remember(&mut tx, &req.idempotency_key, &digest, id).await?;
        }
        tx.commit().await?;
        self.knowledge_gap(id, &Access::admin()).await
    }
}
