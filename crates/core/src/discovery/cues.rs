use super::*;
pub(super) async fn raw(tx: &mut Transaction<'_, Sqlite>, id: &str) -> Result<QuestionCue> {
    let s: String = sqlx::query_scalar("SELECT record FROM discovery_cues WHERE id=?")
        .bind(id)
        .fetch_optional(&mut **tx)
        .await?
        .context("record not found")?;
    Ok(serde_json::from_str(&s)?)
}
pub(super) async fn save(
    tx: &mut Transaction<'_, Sqlite>,
    item: &QuestionCue,
    action: &str,
    contributor: &str,
    note: &str,
) -> Result<()> {
    sqlx::query("INSERT INTO discovery_cues(id,project_id,record) VALUES(?,?,?) ON CONFLICT(id) DO UPDATE SET record=excluded.record").bind(&item.id).bind(&item.project_id).bind(serde_json::to_string(item)?).execute(&mut **tx).await?;
    history(tx, "cue", &item.id, action, contributor, note, item).await
}
impl Store {
    pub async fn question_cue(&self, id: &str, access: &Access) -> Result<QuestionCue> {
        let mut tx = self.pool.begin().await?;
        let item = raw(&mut tx, id).await?;
        permit(&mut tx, &item.source_ids, access).await?;
        tx.commit().await?;
        Ok(item)
    }
    pub async fn question_cues(
        &self,
        project: &str,
        q: DiscoveryQuery,
        access: &Access,
    ) -> Result<CuePage> {
        let (offset, limit) = page(&q)?;
        let status = q.status.as_deref().unwrap_or("active");
        if ![
            "active",
            "unreviewed",
            "needs_answer",
            "resolved",
            "dismissed",
            "all",
        ]
        .contains(&status)
        {
            bail!("invalid cue status");
        }
        let mut tx = self.pool.begin().await?;
        let allowed = allowed(&mut tx, project, access).await?;
        let mut total = 0;
        let mut items = vec![];
        for row in sqlx::query("SELECT record FROM discovery_cues WHERE project_id=? ORDER BY json_extract(record,'$.updated_at') DESC,id").bind(project).fetch_all(&mut *tx).await?{let item:QuestionCue=serde_json::from_str(row.get("record"))?;if !item.source_ids.iter().all(|s|allowed.contains(s)) || (status=="active"&&! ["unreviewed","needs_answer"].contains(&item.status.as_str())) || (status!="active"&&status!="all"&&item.status!=status){continue;}if total>=offset&&items.len()<limit{items.push(item);}total+=1;}
        while serde_json::to_vec(&items)?.len() > 250000 {
            if items.pop().is_none() {
                break;
            }
        }
        let muted: bool = sqlx::query_scalar(
            "SELECT COALESCE((SELECT muted FROM discovery_settings WHERE project_id=?),0)",
        )
        .bind(project)
        .fetch_one(&mut *tx)
        .await?;
        let next_offset = (offset + items.len() < total).then_some(offset + items.len());
        tx.commit().await?;
        Ok(CuePage {
            project_id: project.into(),
            muted,
            items,
            total,
            offset,
            next_offset,
            detection: DETECTION.into(),
        })
    }
    pub async fn create_question_cue(&self, req: CueCreateRequest) -> Result<QuestionCue> {
        validate_text(&req.text, 1000, "question cue")?;
        optional_note(&req.note)?;
        if !["question", "conflict"].contains(&req.kind.as_str()) {
            bail!("invalid cue kind");
        }
        if req.kind == "conflict"
            && req
                .evidence
                .iter()
                .enumerate()
                .all(|(i, e)| req.evidence.iter().take(i).any(|a| a == e) || i == 0)
        {
            bail!("invalid: conflicting statements require two distinct evidence locators");
        }
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let (digest, old) =
            operation(&mut tx, "cue-create", "", &req.idempotency_key, &req).await?;
        let id = if let Some(id) = old {
            id
        } else {
            let sources = citations(
                &mut tx,
                &req.project_id,
                &req.evidence,
                if req.kind == "conflict" { 2 } else { 1 },
            )
            .await?;
            let count: i64 =
                sqlx::query_scalar("SELECT COUNT(*) FROM discovery_cues WHERE project_id=?")
                    .bind(&req.project_id)
                    .fetch_one(&mut *tx)
                    .await?;
            if count >= 5000 {
                bail!("invalid: project cue limit reached");
            }
            let item = QuestionCue {
                id: uuid::Uuid::new_v4().to_string(),
                project_id: req.project_id.clone(),
                kind: req.kind,
                text: req.text.trim().into(),
                status: "needs_answer".into(),
                source_ids: sources,
                evidence: req.evidence.clone(),
                original_evidence: req.evidence,
                version: 1,
                contributor: "You".into(),
                first_seen: now(),
                updated_at: now(),
                note: req.note.clone(),
            };
            save(&mut tx, &item, "created", "You", &req.note).await?;
            remember(&mut tx, &req.idempotency_key, &digest, &item.id).await?;
            item.id
        };
        tx.commit().await?;
        self.question_cue(&id, &Access::admin()).await
    }
    pub async fn update_question_cue(
        &self,
        id: &str,
        req: CueUpdateRequest,
    ) -> Result<QuestionCue> {
        if !["needs_answer", "resolved", "dismissed", "unreviewed"].contains(&req.status.as_str()) {
            bail!("invalid cue status");
        }
        optional_note(&req.note)?;
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let (digest, old) =
            operation(&mut tx, "cue-update", id, &req.idempotency_key, &req).await?;
        if old.is_none() {
            let mut item = raw(&mut tx, id).await?;
            if item.version != req.expected_version {
                bail!("conflict: question cue changed; refresh");
            }
            if req.status == "resolved" {
                validate_text(&req.note, 2000, "resolution explanation")?;
                if req.evidence.is_empty() {
                    bail!("invalid: resolving a question requires answer evidence");
                }
            }
            if !req.evidence.is_empty() {
                let sources = citations(&mut tx, &item.project_id, &req.evidence, 1).await?;
                item.source_ids.extend(sources);
                item.source_ids.sort();
                item.source_ids.dedup();
                item.evidence = req.evidence.clone();
            }
            item.status = req.status;
            item.note = req.note.clone();
            item.version += 1;
            item.updated_at = now();
            save(&mut tx, &item, "reviewed", "You", &req.note).await?;
            remember(&mut tx, &req.idempotency_key, &digest, id).await?;
        }
        tx.commit().await?;
        self.question_cue(id, &Access::admin()).await
    }
    pub async fn mute_question_cues(
        &self,
        project: &str,
        req: DiscoveryMuteRequest,
    ) -> Result<DiscoverySettings> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        project_sources(&mut tx, project).await?;
        let (digest, old) = operation(
            &mut tx,
            "discovery-mute",
            project,
            &req.idempotency_key,
            &req,
        )
        .await?;
        if old.is_none() {
            sqlx::query("INSERT INTO discovery_settings(project_id,muted) VALUES(?,?) ON CONFLICT(project_id) DO UPDATE SET muted=excluded.muted").bind(project).bind(req.muted).execute(&mut *tx).await?;
            event(&mut tx, "discovery", project).await?;
            remember(&mut tx, &req.idempotency_key, &digest, project).await?;
        }
        let muted: bool =
            sqlx::query_scalar("SELECT muted FROM discovery_settings WHERE project_id=?")
                .bind(project)
                .fetch_one(&mut *tx)
                .await?;
        tx.commit().await?;
        Ok(DiscoverySettings {
            project_id: project.into(),
            muted,
        })
    }
}
