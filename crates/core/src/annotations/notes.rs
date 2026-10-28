use super::*;
async fn raw(tx: &mut Transaction<'_, Sqlite>, id: &str) -> Result<AnchoredNote> {
    let s: String = sqlx::query_scalar("SELECT record FROM annotation_notes WHERE id=?")
        .bind(id)
        .fetch_optional(&mut **tx)
        .await?
        .context("record not found")?;
    Ok(serde_json::from_str(&s)?)
}
async fn evidence_source(tx: &mut Transaction<'_, Sqlite>, e: &Evidence) -> Result<String> {
    if e.line < 1 || e.quote.len() > 2000 || e.quote.trim().is_empty() {
        bail!("invalid: anchor needs a nonempty exact source line up to 2000 bytes");
    }
    let row=sqlx::query("SELECT d.source_id,r.body FROM documents d JOIN revisions r ON r.document_id=d.id WHERE d.id=? AND r.revision=?").bind(&e.document_id).bind(e.revision).fetch_optional(&mut **tx).await?.context("invalid anchor revision")?;
    if row
        .get::<String, _>("body")
        .lines()
        .nth(e.line as usize - 1)
        != Some(e.quote.as_str())
    {
        bail!("invalid anchor locator");
    }
    Ok(row.get("source_id"))
}
async fn save(tx: &mut Transaction<'_, Sqlite>, n: &mut AnchoredNote, action: &str) -> Result<()> {
    if let Some(id) = &n.observation_id {
        crate::memory_review::suppress_memory_tx(
            tx,
            id,
            &format!(
                "Anchored note {} superseded by user revision {}",
                n.id, n.version
            ),
        )
        .await?;
    }
    n.observation_id = if n.status == "active" {
        let id = uuid::Uuid::new_v4().to_string();
        let body = format!(
            "User {} anchored at {} r{} line {}: {}",
            if n.emphasis { "EMPHASIS" } else { "note" },
            n.evidence.document_id,
            n.evidence.revision,
            n.evidence.line,
            n.body
        );
        sqlx::query("INSERT INTO observations(id,body,contributor,source_ids,evidence,idempotency_key,created_at) VALUES(?,?,?,?,?,?,?)").bind(&id).bind(body).bind("You").bind(serde_json::to_string(&vec![&n.source_id])?).bind(serde_json::to_string(&vec![&n.evidence])?).bind(format!("annotation:{}:{}",n.id,n.version)).bind(&n.updated_at).execute(&mut **tx).await?;
        Some(id)
    } else {
        None
    };
    let json = serde_json::to_string(n)?;
    sqlx::query("INSERT INTO annotation_notes(id,document_id,source_id,record) VALUES(?,?,?,?) ON CONFLICT(id) DO UPDATE SET record=excluded.record").bind(&n.id).bind(&n.evidence.document_id).bind(&n.source_id).bind(&json).execute(&mut **tx).await?;
    sqlx::query("INSERT INTO annotation_note_history(note_id,record,action) VALUES(?,?,?)")
        .bind(&n.id)
        .bind(&json)
        .bind(action)
        .execute(&mut **tx)
        .await?;
    event(tx, "annotation", &n.id).await
}
impl Store {
    pub async fn anchored_note(&self, id: &str, access: &Access) -> Result<AnchoredNote> {
        let mut tx = self.pool.begin().await?;
        let n = raw(&mut tx, id).await?;
        permit(&mut tx, &[n.source_id.clone()], access).await?;
        tx.commit().await?;
        Ok(n)
    }
    pub async fn anchored_notes(
        &self,
        document: &str,
        q: AnnotationQuery,
        access: &Access,
    ) -> Result<NotePage> {
        let (offset, limit) = page(&q)?;
        let doc = self.document(document, q.revision, access).await?;
        let mut items = vec![];
        let mut total = 0;
        for row in sqlx::query("SELECT record FROM annotation_notes WHERE document_id=? ORDER BY json_extract(record,'$.emphasis') DESC,json_extract(record,'$.evidence.line'),id").bind(document).fetch_all(&self.pool).await?{
            let n:AnchoredNote=serde_json::from_str(row.get("record"))?;if (!q.include_archived && n.status!="active")||q.revision.is_some_and(|r|r!=n.evidence.revision){continue;}
            if total>=offset&&items.len()<limit{items.push(n);}total+=1;
        }
        let _ = doc;
        let next_offset = (offset + items.len() < total).then_some(offset + items.len());
        Ok(NotePage {
            document_id: document.into(),
            items,
            total,
            offset,
            next_offset,
        })
    }
    pub async fn create_anchored_note(&self, req: NoteCreateRequest) -> Result<AnchoredNote> {
        validate_text(&req.body, 4000, "anchored note")?;
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let (digest, old) =
            operation(&mut tx, "note-create", "", &req.idempotency_key, &req).await?;
        let id = if let Some(id) = old {
            id
        } else {
            let source_id = evidence_source(&mut tx, &req.evidence).await?;
            let count: i64 =
                sqlx::query_scalar("SELECT COUNT(*) FROM annotation_notes WHERE document_id=?")
                    .bind(&req.evidence.document_id)
                    .fetch_one(&mut *tx)
                    .await?;
            if count >= 10000 {
                bail!("invalid: document note limit reached");
            }
            let mut n = AnchoredNote {
                id: uuid::Uuid::new_v4().to_string(),
                source_id,
                evidence: req.evidence.clone(),
                body: req.body.trim().into(),
                emphasis: req.emphasis,
                status: "active".into(),
                version: 1,
                contributor: "You".into(),
                created_at: now(),
                updated_at: now(),
                observation_id: None,
            };
            save(&mut tx, &mut n, "created").await?;
            remember(&mut tx, &req.idempotency_key, &digest, &n.id).await?;
            n.id
        };
        tx.commit().await?;
        self.anchored_note(&id, &Access::admin()).await
    }
    pub async fn update_anchored_note(
        &self,
        id: &str,
        req: NoteUpdateRequest,
    ) -> Result<AnchoredNote> {
        validate_text(&req.body, 4000, "anchored note")?;
        if !["active", "archived"].contains(&req.status.as_str()) {
            bail!("invalid note status");
        }
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let (digest, old) =
            operation(&mut tx, "note-update", id, &req.idempotency_key, &req).await?;
        if old.is_none() {
            let mut n = raw(&mut tx, id).await?;
            if n.version != req.expected_version {
                bail!("conflict: note changed; refresh before editing");
            }
            n.body = req.body.trim().into();
            n.emphasis = req.emphasis;
            n.status = req.status;
            n.version += 1;
            n.updated_at = now();
            save(&mut tx, &mut n, "updated").await?;
            remember(&mut tx, &req.idempotency_key, &digest, id).await?;
        }
        tx.commit().await?;
        self.anchored_note(id, &Access::admin()).await
    }
    pub async fn anchored_note_history(
        &self,
        id: &str,
        q: AnnotationQuery,
        access: &Access,
    ) -> Result<NoteHistoryPage> {
        let (offset, limit) = page(&q)?;
        let mut tx = self.pool.begin().await?;
        let n = raw(&mut tx, id).await?;
        permit(&mut tx, &[n.source_id], access).await?;
        let total: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM annotation_note_history WHERE note_id=?")
                .bind(id)
                .fetch_one(&mut *tx)
                .await?;
        let rows=sqlx::query("SELECT * FROM annotation_note_history WHERE note_id=? ORDER BY id DESC LIMIT ? OFFSET ?").bind(id).bind(limit as i64).bind(offset as i64).fetch_all(&mut *tx).await?;
        let mut items = vec![];
        for row in rows {
            items.push(NoteHistory {
                id: row.get("id"),
                note_id: id.into(),
                action: row.get("action"),
                record: serde_json::from_str(row.get("record"))?,
            });
        }
        let next_offset = (offset + items.len() < total as usize).then_some(offset + items.len());
        tx.commit().await?;
        Ok(NoteHistoryPage {
            note_id: id.into(),
            items,
            total: total as usize,
            offset,
            next_offset,
        })
    }
    /// User emphasis for summary prompts, with original source evidence. Caller enforces source permissions.
    pub async fn annotation_context(&self, sources: &[String], max_bytes: usize) -> Result<String> {
        let mut tx = self.pool.begin().await?;
        let result = annotation_context_tx(&mut tx, sources, max_bytes).await?;
        tx.commit().await?;
        Ok(result)
    }
}
pub(crate) async fn annotation_context_tx(
    tx: &mut Transaction<'_, Sqlite>,
    sources: &[String],
    max_bytes: usize,
) -> Result<String> {
    let mut text = String::new();
    let mut omitted = 0usize;
    for row in sqlx::query("SELECT record FROM annotation_notes WHERE json_extract(record,'$.status')='active' ORDER BY json_extract(record,'$.emphasis') DESC,json_extract(record,'$.updated_at') DESC,id").fetch_all(&mut **tx).await?{
        let n:AnchoredNote=serde_json::from_str(row.get("record"))?;if !sources.contains(&n.source_id){continue;}
        let Some(observation_id)=&n.observation_id else{continue;};
        let row=sqlx::query("SELECT * FROM observations WHERE id=?").bind(observation_id).fetch_optional(&mut **tx).await?;
        let Some(row)=row else{continue;};let obs=crate::store::observation_row(&row)?;
        let effective=crate::memory_review::effective_observations_tx(tx,vec![obs]).await?;
        let Some(obs)=effective.first() else{continue;};
        let line=format!("\n{}",serde_json::to_string(&serde_json::json!({"note_id":n.id,"source_id":n.source_id,"evidence":n.evidence,"emphasis":n.emphasis,"body":obs.body,"contributor":obs.contributor,"updated_at":n.updated_at}))?);if text.len()+line.len()<=max_bytes.saturating_sub(100){text.push_str(&line);}else{omitted+=1;}
    }
    if omitted > 0 {
        text.push_str(&format!("\n{omitted} additional anchored notes omitted."));
    }
    Ok(text)
}
