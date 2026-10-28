use super::*;
pub(super) async fn raw(tx: &mut Transaction<'_, Sqlite>, id: &str) -> Result<DatedClaim> {
    let s:String=sqlx::query_scalar("SELECT r.record FROM knowledge_claim_revisions r JOIN knowledge_claims c ON c.id=r.id AND c.version=r.version WHERE c.id=?").bind(id).fetch_optional(&mut **tx).await?.context("record not found")?;
    Ok(serde_json::from_str(&s)?)
}
pub(super) fn observed() -> String {
    Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Nanos, true)
}
async fn append(tx: &mut Transaction<'_, Sqlite>, item: &DatedClaim) -> Result<()> {
    sqlx::query(
        "INSERT INTO knowledge_claim_revisions(id,version,observed_at,record) VALUES(?,?,?,?)",
    )
    .bind(&item.id)
    .bind(item.version)
    .bind(&item.observed_at)
    .bind(serde_json::to_string(item)?)
    .execute(&mut **tx)
    .await?;
    sqlx::query("UPDATE knowledge_claims SET version=?,source_ids=? WHERE id=?")
        .bind(item.version)
        .bind(serde_json::to_string(&item.source_ids)?)
        .bind(&item.id)
        .execute(&mut **tx)
        .await?;
    event(tx, "knowledge_claim", &item.id).await
}
impl Store {
    pub async fn dated_claim(&self, id: &str, access: &Access) -> Result<DatedClaim> {
        let mut tx = self.pool.begin().await?;
        let item = raw(&mut tx, id).await?;
        permit(&mut tx, &item.source_ids, access).await?;
        tx.commit().await?;
        Ok(item)
    }
    pub async fn dated_claims(
        &self,
        project: &str,
        q: KnowledgeQuery,
        access: &Access,
    ) -> Result<ClaimPage> {
        let (offset, limit) = page(&q)?;
        let mut tx = self.pool.begin().await?;
        let allowed = allowed(&mut tx, project, access).await?;
        let query = q.query.unwrap_or_default().to_lowercase();
        if query.len() > 1000 {
            bail!("invalid claim query");
        }
        let mut total = 0;
        let mut items = vec![];
        for row in sqlx::query("SELECT r.record FROM knowledge_claims c JOIN knowledge_claim_revisions r ON r.id=c.id AND r.version=c.version WHERE c.project_id=? ORDER BY r.observed_at DESC,c.id").bind(project).fetch_all(&mut *tx).await?{let item:DatedClaim=serde_json::from_str(row.get("record"))?;if !item.source_ids.iter().all(|s|allowed.contains(s))||(!q.include_archived&&item.status!="active")||(!query.is_empty()&&!format!("{} {} {}",item.subject,item.predicate,item.value).to_lowercase().contains(&query)){continue;}if total>=offset&&items.len()<limit{items.push(item);}total+=1;}
        while serde_json::to_vec(&items)?.len() > 250000 {
            if items.pop().is_none() {
                break;
            }
        }
        let next_offset = (offset + items.len() < total).then_some(offset + items.len());
        tx.commit().await?;
        Ok(ClaimPage {
            project_id: project.into(),
            items,
            total,
            offset,
            next_offset,
        })
    }
    pub async fn claim_history(
        &self,
        id: &str,
        q: KnowledgeQuery,
        access: &Access,
    ) -> Result<ClaimPage> {
        let (offset, limit) = page(&q)?;
        let mut tx = self.pool.begin().await?;
        let current = raw(&mut tx, id).await?;
        permit(&mut tx, &current.source_ids, access).await?;
        let total: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM knowledge_claim_revisions WHERE id=?")
                .bind(id)
                .fetch_one(&mut *tx)
                .await?;
        let rows=sqlx::query("SELECT record FROM knowledge_claim_revisions WHERE id=? ORDER BY version DESC LIMIT ? OFFSET ?").bind(id).bind(limit as i64).bind(offset as i64).fetch_all(&mut *tx).await?;
        let mut items = vec![];
        for r in rows {
            items.push(serde_json::from_str::<DatedClaim>(r.get("record"))?);
        }
        while serde_json::to_vec(&items)?.len() > 250000 {
            if items.pop().is_none() {
                break;
            }
        }
        let next_offset = (offset + items.len() < total as usize).then_some(offset + items.len());
        tx.commit().await?;
        Ok(ClaimPage {
            project_id: current.project_id,
            items,
            total: total as usize,
            offset,
            next_offset,
        })
    }
    pub async fn create_dated_claim(&self, req: ClaimCreateRequest) -> Result<DatedClaim> {
        validate_text(&req.subject, 300, "claim subject")?;
        validate_text(&req.predicate, 200, "claim facet")?;
        validate_text(&req.value, 2000, "claim value")?;
        optional(&req.note, 2000)?;
        interval(&req.valid_from, req.valid_until.as_deref())?;
        if !["asserted", "uncertain"].contains(&req.certainty.as_str()) {
            bail!("invalid claim certainty");
        }
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let (digest, old) = operation(
            &mut tx,
            "knowledge-claim-create",
            "",
            &req.idempotency_key,
            &req,
        )
        .await?;
        let id = if let Some(id) = old {
            id
        } else {
            let ids = citations(&mut tx, &req.project_id, &req.evidence).await?;
            let count: i64 =
                sqlx::query_scalar("SELECT COUNT(*) FROM knowledge_claims WHERE project_id=?")
                    .bind(&req.project_id)
                    .fetch_one(&mut *tx)
                    .await?;
            if count >= 5000 {
                bail!("invalid: project has 5000 claims");
            }
            let item = DatedClaim {
                id: uuid::Uuid::new_v4().to_string(),
                project_id: req.project_id,
                subject: req.subject.trim().into(),
                predicate: req.predicate.trim().into(),
                value: req.value.trim().into(),
                certainty: req.certainty,
                valid_from: req.valid_from,
                valid_until: req.valid_until,
                observed_at: observed(),
                version: 1,
                status: "active".into(),
                source_ids: ids,
                evidence: req.evidence,
                contributor: "You".into(),
                note: req.note,
            };
            sqlx::query(
                "INSERT INTO knowledge_claims(id,project_id,source_ids,version) VALUES(?,?,?,1)",
            )
            .bind(&item.id)
            .bind(&item.project_id)
            .bind(serde_json::to_string(&item.source_ids)?)
            .execute(&mut *tx)
            .await?;
            append(&mut tx, &item).await?;
            remember(&mut tx, &req.idempotency_key, &digest, &item.id).await?;
            item.id
        };
        tx.commit().await?;
        self.dated_claim(&id, &Access::admin()).await
    }
    pub async fn update_dated_claim(
        &self,
        id: &str,
        req: ClaimUpdateRequest,
    ) -> Result<DatedClaim> {
        validate_text(&req.note, 2000, "claim revision reason")?;
        if !["amend", "retract", "restore"].contains(&req.action.as_str()) {
            bail!("invalid claim action");
        }
        if req.action != "amend"
            && (req.value.is_some()
                || req.certainty.is_some()
                || req.valid_from.is_some()
                || req.valid_until.is_some()
                || req.clear_valid_until
                || !req.evidence.is_empty())
        {
            bail!("invalid: claim field changes require amend");
        }
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let (digest, old) = operation(
            &mut tx,
            "knowledge-claim-update",
            id,
            &req.idempotency_key,
            &req,
        )
        .await?;
        if old.is_none() {
            let mut item = raw(&mut tx, id).await?;
            if item.version != req.expected_version {
                bail!("conflict: dated claim changed; refresh");
            }
            match req.action.as_str() {
                "amend" => {
                    if item.status != "active" {
                        bail!("conflict: restore claim before amending");
                    }
                    if req.evidence.is_empty() {
                        bail!("invalid: amendments require exact supporting evidence");
                    }
                    if let Some(v) = req.value {
                        validate_text(&v, 2000, "claim value")?;
                        item.value = v.trim().into();
                    }
                    if let Some(c) = req.certainty {
                        if !["asserted", "uncertain"].contains(&c.as_str()) {
                            bail!("invalid claim certainty");
                        }
                        item.certainty = c;
                    }
                    if let Some(v) = req.valid_from {
                        item.valid_from = v;
                    }
                    if req.clear_valid_until && req.valid_until.is_some() {
                        bail!("invalid: choose valid_until or clear_valid_until");
                    }
                    if req.clear_valid_until {
                        item.valid_until = None;
                    } else if let Some(v) = req.valid_until {
                        item.valid_until = Some(v);
                    }
                    interval(&item.valid_from, item.valid_until.as_deref())?;
                    item.source_ids
                        .extend(citations(&mut tx, &item.project_id, &req.evidence).await?);
                    item.source_ids.sort();
                    item.source_ids.dedup();
                    item.evidence = req.evidence;
                }
                "retract" => {
                    if item.status != "active" {
                        bail!("conflict: claim already retracted");
                    }
                    item.status = "retracted".into();
                }
                "restore" => {
                    if item.status != "retracted" {
                        bail!("conflict: claim already active");
                    }
                    item.status = "active".into();
                }
                _ => {}
            }
            item.version += 1;
            item.observed_at = observed();
            item.note = req.note;
            append(&mut tx, &item).await?;
            remember(&mut tx, &req.idempotency_key, &digest, id).await?;
        }
        tx.commit().await?;
        self.dated_claim(id, &Access::admin()).await
    }
}
