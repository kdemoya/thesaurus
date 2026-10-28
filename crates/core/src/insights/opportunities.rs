use super::*;
fn normalized(text: &str) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}
// Distinct import IDs alone do not establish independent origins. Reject obvious
// copied snapshots and repeated supporting passages before asking for user review.
async fn independent_origins(
    tx: &mut Transaction<'_, Sqlite>,
    evidence: &[Evidence],
) -> Result<Vec<String>> {
    let mut sources = Vec::new();
    let mut bodies = std::collections::BTreeMap::<String, String>::new();
    let mut quotes = std::collections::BTreeMap::<String, String>::new();
    let mut origins = std::collections::BTreeMap::<String, String>::new();
    for e in evidence {
        let row = sqlx::query("SELECT d.source_id,s.uri,r.body FROM documents d JOIN sources s ON s.id=d.source_id JOIN revisions r ON r.document_id=d.id WHERE d.id=? AND r.revision=?")
            .bind(&e.document_id).bind(e.revision).fetch_one(&mut **tx).await?;
        let source: String = row.get("source_id");
        let body = crate::store::hash(&normalized(&row.get::<String, _>("body")));
        let quote = normalized(&e.quote);
        let origin = row
            .get::<String, _>("uri")
            .trim()
            .trim_end_matches('/')
            .to_string();
        for (seen, value) in [
            (&mut bodies, body),
            (&mut quotes, quote),
            (&mut origins, origin),
        ] {
            if seen.get(&value).is_some_and(|prior| prior != &source) {
                bail!(
                    "invalid: duplicate or copied evidence origins do not establish independent recurring friction; choose independently originating source passages"
                );
            }
            seen.insert(value, source.clone());
        }
        if !sources.contains(&source) {
            sources.push(source);
        }
    }
    if sources.len() < 2 {
        bail!("invalid: recurring friction requires at least two independent source origins");
    }
    sources.sort();
    Ok(sources)
}
async fn citations(
    tx: &mut Transaction<'_, Sqlite>,
    scope: &[String],
    evidence: &[Evidence],
) -> Result<Vec<String>> {
    if evidence.len() > 20 {
        bail!("invalid: at most 20 evidence lines per evidence category");
    }
    let mut sources = Vec::new();
    for e in evidence {
        if e.line < 1 || e.quote.len() > 2000 || e.quote.trim().is_empty() {
            bail!("invalid opportunity evidence line");
        }
        let row=sqlx::query("SELECT d.source_id,r.body FROM documents d JOIN revisions r ON r.document_id=d.id WHERE d.id=? AND r.revision=?").bind(&e.document_id).bind(e.revision).fetch_optional(&mut **tx).await?.context("record not found")?;
        let source: String = row.get("source_id");
        if !scope.contains(&source)
            || row
                .get::<String, _>("body")
                .lines()
                .nth(e.line as usize - 1)
                != Some(e.quote.as_str())
        {
            bail!("invalid opportunity evidence scope or locator");
        }
        sources.push(source);
    }
    sources.sort();
    sources.dedup();
    Ok(sources)
}
async fn raw(
    tx: &mut Transaction<'_, Sqlite>,
    project: &str,
    id: &str,
    access: &Access,
) -> Result<Opportunity> {
    scope(tx, project, access).await?;
    let json: String =
        sqlx::query_scalar("SELECT record FROM insight_opportunities WHERE id=? AND project_id=?")
            .bind(id)
            .bind(project)
            .fetch_optional(&mut **tx)
            .await?
            .context("record not found")?;
    let item: Opportunity = serde_json::from_str(&json)?;
    permit(tx, &item.source_ids, access).await?;
    citations(tx, &item.source_ids, &item.evidence).await?;
    citations(tx, &item.source_ids, &item.counterevidence).await?;
    Ok(item)
}
impl Store {
    pub async fn discover_opportunity(
        &self,
        project: &str,
        topic: &str,
        access: &Access,
    ) -> Result<OpportunityDiscovery> {
        validate_text(topic, 100, "opportunity topic")?;
        let mut tx = self.pool.begin().await?;
        let ids = scope(&mut tx, project, access).await?;
        let docs = documents(&mut tx, &ids).await?;
        let needle = topic.to_lowercase();
        let mut evidence = Vec::new();
        let mut counter = Vec::new();
        let mut independent = Vec::new();
        let mut omitted = 0;
        for doc in &docs {
            let mut per_source = 0;
            for (index, line) in doc.body.lines().enumerate() {
                let text = line.to_lowercase();
                if !text.contains(&needle) {
                    continue;
                }
                let negative = [
                    "blocker",
                    "friction",
                    "pain",
                    "manual",
                    "slow",
                    "repeated",
                    "delay",
                    "failed",
                    "bottleneck",
                ]
                .iter()
                .any(|word| text.contains(word));
                let positive = [
                    "resolved",
                    "fixed",
                    "improved",
                    "no longer",
                    "works now",
                    "working well",
                ]
                .iter()
                .any(|word| text.contains(word));
                if !negative && !positive {
                    continue;
                }
                if !positive && per_source >= 3 {
                    omitted += 1;
                    continue;
                }
                if line.len() > 2000 {
                    omitted += 1;
                    continue;
                }
                let cite = Evidence {
                    document_id: doc.id.clone(),
                    revision: doc.revision,
                    line: index as i64 + 1,
                    quote: line.into(),
                };
                let list = if positive {
                    &mut counter
                } else {
                    &mut evidence
                };
                if list.len() == 20 {
                    omitted += 1;
                    continue;
                }
                list.push(cite);
                if !positive {
                    per_source += 1;
                }
                if !positive && !independent.contains(&doc.source_id) {
                    independent.push(doc.source_id.clone());
                }
            }
        }
        let candidate = if independent.len() >= 2
            && independent_origins(&mut tx, &evidence).await.is_ok()
        {
            let affected = tasks(&mut tx, &docs)
                .await?
                .into_iter()
                .filter(|t| t.title.to_lowercase().contains(&needle))
                .map(|t| t.id)
                .take(50)
                .collect();
            Some(OpportunityCreate {
                title: format!("Review recurring friction: {topic}"),
                hypothesis: format!(
                    "These distinct source lines mention {topic} and friction markers. Review their independent origins, whether they describe the same recurring problem, and the counterevidence before accepting an opportunity."
                ),
                evidence,
                counterevidence: counter,
                affected_task_ids: affected,
                idempotency_key: uuid::Uuid::new_v4().to_string(),
            })
        } else {
            None
        };
        Ok(OpportunityDiscovery{topic:topic.into(),candidate,explanation:"This is a provisional grouping of matching words across distinct source documents. Identical normalized snapshots and repeated supporting passages across sources are rejected as copied origins. Other shared origins cannot be inferred: accepting requires your explicit attestation after reviewing the original evidence and counterevidence. This does not establish semantic equivalence or causation.".into(),documents_considered:docs.len(),lines_omitted:omitted})
    }
    pub async fn create_opportunity(
        &self,
        project: &str,
        req: OpportunityCreate,
        access: &Access,
    ) -> Result<Opportunity> {
        admin(access)?;
        validate_text(&req.title, 300, "opportunity title")?;
        validate_text(&req.hypothesis, 4000, "opportunity hypothesis")?;
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let ids = scope(&mut tx, project, access).await?;
        let (digest, old) = operation(
            &mut tx,
            "opportunity-create",
            project,
            &req.idempotency_key,
            &req,
        )
        .await?;
        if let Some(id) = old {
            return raw(&mut tx, project, &id, access).await;
        }
        if citations(&mut tx, &ids, &req.evidence).await?.len() < 2 {
            bail!(
                "invalid: recurring friction requires evidence from at least two distinct source documents"
            );
        }
        independent_origins(&mut tx, &req.evidence).await?;
        citations(&mut tx, &ids, &req.counterevidence).await?;
        if req.affected_task_ids.len() > 50 {
            bail!("invalid: too many affected tasks");
        }
        let docs = documents(&mut tx, &ids).await?;
        let tasks = tasks(&mut tx, &docs).await?;
        if req
            .affected_task_ids
            .iter()
            .any(|id| !tasks.iter().any(|t| &t.id == id))
        {
            bail!("invalid: affected work is outside project scope");
        }
        let stamp = now();
        let item = Opportunity {
            id: uuid::Uuid::new_v4().to_string(),
            project_id: project.into(),
            source_ids: ids,
            version: 1,
            title: req.title,
            hypothesis: req.hypothesis,
            evidence: req.evidence,
            counterevidence: req.counterevidence,
            affected_task_ids: req.affected_task_ids,
            status: "candidate".into(),
            review_note: None,
            independence_review: None,
            created_at: stamp.clone(),
            updated_at: stamp,
        };
        sqlx::query("INSERT INTO insight_opportunities(id,project_id,record) VALUES(?,?,?)")
            .bind(&item.id)
            .bind(project)
            .bind(serde_json::to_string(&item)?)
            .execute(&mut *tx)
            .await?;
        history(&mut tx, "opportunity", &item.id, item.version, &item).await?;
        remember(&mut tx, &req.idempotency_key, &digest, &item.id).await?;
        tx.commit().await?;
        Ok(item)
    }
    pub async fn opportunities(&self, project: &str, access: &Access) -> Result<Vec<Opportunity>> {
        let mut tx = self.pool.begin().await?;
        scope(&mut tx, project, access).await?;
        let mut result = Vec::new();
        for row in sqlx::query(
            "SELECT id FROM insight_opportunities WHERE project_id=? ORDER BY rowid DESC LIMIT 50",
        )
        .bind(project)
        .fetch_all(&mut *tx)
        .await?
        {
            result.push(raw(&mut tx, project, row.get("id"), access).await?);
        }
        Ok(result)
    }
    pub async fn review_opportunity(
        &self,
        project: &str,
        id: &str,
        req: InsightReview,
        access: &Access,
    ) -> Result<Opportunity> {
        admin(access)?;
        validate_text(&req.note, 2000, "opportunity review note")?;
        if !["accepted", "rejected", "candidate"].contains(&req.decision.as_str()) {
            bail!("invalid opportunity decision");
        }
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let (digest, old) = operation(
            &mut tx,
            "opportunity-review",
            id,
            &req.idempotency_key,
            &req,
        )
        .await?;
        let mut item = raw(&mut tx, project, id, access).await?;
        if old.is_some() {
            return Ok(item);
        }
        if item.version != req.expected_version {
            bail!("conflict: opportunity changed");
        }
        if req.decision == "accepted" {
            if !req.attest_independent {
                bail!(
                    "invalid: accepting requires explicit attestation that the supporting evidence has independent origins, not copies of one report"
                );
            }
            let sources = independent_origins(&mut tx, &item.evidence).await?;
            item.independence_review = Some(OpportunityIndependenceReview {
                contributor: access.contributor.clone(),
                reviewed_at: now(),
                note: req.note.clone(),
                source_ids: sources,
            });
        } else {
            item.independence_review = None;
        }
        item.version += 1;
        item.status = req.decision;
        item.review_note = Some(req.note);
        item.updated_at = now();
        sqlx::query("UPDATE insight_opportunities SET record=? WHERE id=?")
            .bind(serde_json::to_string(&item)?)
            .bind(id)
            .execute(&mut *tx)
            .await?;
        history(&mut tx, "opportunity", id, item.version, &item).await?;
        remember(&mut tx, &req.idempotency_key, &digest, id).await?;
        tx.commit().await?;
        Ok(item)
    }
}
