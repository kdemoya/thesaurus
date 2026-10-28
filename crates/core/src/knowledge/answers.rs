use super::*;
use std::collections::BTreeMap;
async fn build(
    tx: &mut Transaction<'_, Sqlite>,
    req: KnowledgeAnswerRequest,
    access: &Access,
) -> Result<KnowledgeAnswer> {
    validate_text(&req.question, 1000, "knowledge question")?;
    let day = req
        .valid_as_of
        .clone()
        .unwrap_or_else(|| Utc::now().date_naive().to_string());
    date(&day)?;
    let cutoff = if let Some(s) = req.known_by.as_deref() {
        DateTime::parse_from_rfc3339(s)
            .context("invalid known_by; use an RFC3339 timestamp")?
            .with_timezone(&Utc)
    } else {
        Utc::now()
    };
    if cutoff > Utc::now() {
        bail!("invalid: known_by cannot be in the future");
    }
    let known_by = cutoff.to_rfc3339_opts(chrono::SecondsFormat::Nanos, true);
    let p = project_sources(tx, &req.project_id).await?;
    let allowed = allowed(tx, &req.project_id, access).await?;
    let mut scope = req
        .source_ids
        .clone()
        .unwrap_or_else(|| p.iter().filter(|s| allowed.contains(s)).cloned().collect());
    scope.sort();
    scope.dedup();
    if scope.is_empty() || scope.len() > 50 || !scope.iter().all(|s| p.contains(s)) {
        bail!("invalid: choose 1–50 project sources");
    }
    permit(tx, &scope, access).await?;
    let historical_snapshot = req.known_by.is_some()
        || req
            .valid_as_of
            .as_deref()
            .is_some_and(|d| d != Utc::now().date_naive().to_string());
    let mut policies = BTreeMap::<String, String>::new();
    let mut live_only_excluded = vec![];
    for source in &scope {
        let policy: Option<String> =
            sqlx::query_scalar("SELECT policy FROM retrieval_policies WHERE source_id=?")
                .bind(source)
                .fetch_optional(&mut **tx)
                .await?;
        policies.insert(source.clone(), policy.unwrap_or_else(|| "index".into()));
    }
    if !historical_snapshot {
        scope.retain(|source| {
            let live = policies.get(source).is_some_and(|p| p == "live");
            if live {
                live_only_excluded.push(source.clone());
            }
            !live
        });
    }
    let words = terms(&req.question);
    let mut all_claims = vec![];
    let mut facets = BTreeMap::<(String, String), Vec<(String, String)>>::new();
    for row in
        sqlx::query("SELECT id,source_ids FROM knowledge_claims WHERE project_id=? ORDER BY id")
            .bind(&req.project_id)
            .fetch_all(&mut **tx)
            .await?
    {
        let ids: Vec<String> = serde_json::from_str(row.get("source_ids"))?;
        if !ids.iter().all(|s| scope.contains(s)) {
            continue;
        }
        let s:Option<String>=sqlx::query_scalar("SELECT record FROM knowledge_claim_revisions WHERE id=? AND observed_at<=? ORDER BY version DESC LIMIT 1").bind(row.get::<String,_>("id")).bind(&known_by).fetch_optional(&mut **tx).await?;
        let Some(s) = s else {
            continue;
        };
        let item: DatedClaim = serde_json::from_str(&s)?;
        if item.status != "active"
            || item.valid_from > day
            || item.valid_until.as_ref().is_some_and(|until| &day >= until)
        {
            continue;
        }
        let text = format!("{} {} {}", item.subject, item.predicate, item.value).to_lowercase();
        let score = words.iter().filter(|w| text.contains(w.as_str())).count();
        if !words.is_empty() && score == 0 {
            continue;
        }
        facets
            .entry((item.subject.to_lowercase(), item.predicate.to_lowercase()))
            .or_default()
            .push((item.value.clone(), item.id.clone()));
        all_claims.push((score, item));
    }
    all_claims.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.id.cmp(&b.1.id)));
    let claims_omitted = all_claims.len().saturating_sub(30);
    let claims = all_claims
        .into_iter()
        .take(30)
        .map(|(_, c)| c)
        .collect::<Vec<_>>();
    let mut contradictions = vec![];
    for ((subject, predicate), items) in facets {
        let mut values = items.iter().map(|(v, _)| v.clone()).collect::<Vec<_>>();
        values.sort();
        values.dedup();
        if values.len() > 1 {
            values.truncate(10);
            contradictions.push(AnswerContradiction{subject,predicate,values,claim_ids:items.into_iter().map(|(_,id)|id).take(20).collect(),explanation:"Different active values under the same registered subject/facet at this validity date. These may reflect a conflict or a multivalued facet; user review is required.".into()});
        }
    }
    let contradiction_count = contradictions.len();
    contradictions.truncate(20);
    while serde_json::to_vec(&contradictions)?.len() > 60000 {
        if contradictions.pop().is_none() {
            break;
        }
    }
    let contradictions_omitted = contradiction_count.saturating_sub(contradictions.len());
    let mut passages = vec![];
    let mut passage_matches = 0usize;
    if !words.is_empty() {
        for source in &scope {
            let d: Option<String> =
                sqlx::query_scalar("SELECT id FROM documents WHERE source_id=?")
                    .bind(source)
                    .fetch_optional(&mut **tx)
                    .await?;
            let Some(id) = d else {
                continue;
            };
            let rows =
                sqlx::query("SELECT * FROM revisions WHERE document_id=? ORDER BY revision DESC")
                    .bind(&id)
                    .fetch_all(&mut **tx)
                    .await?;
            let mut chosen = None;
            for row in rows {
                let t = DateTime::parse_from_rfc3339(&row.get::<String, _>("updated_at"))?
                    .with_timezone(&Utc);
                if t <= cutoff {
                    chosen = Some(row);
                    break;
                }
            }
            let Some(row) = chosen else {
                continue;
            };
            let upstream_record: Option<String> =
                sqlx::query_scalar("SELECT record FROM retrieval_upstreams WHERE source_id=?")
                    .bind(source)
                    .fetch_optional(&mut **tx)
                    .await?;
            let upstream = upstream_record
                .map(|s| serde_json::from_str::<UpstreamConnection>(&s))
                .transpose()?;
            let fetches = sqlx::query("SELECT created_at FROM retrieval_fetches WHERE document_id=? AND revision=? AND status='completed' ORDER BY created_at DESC")
                .bind(&id).bind(row.get::<i64,_>("revision")).fetch_all(&mut **tx).await?;
            let mut fetched_at = None;
            for fetch in fetches {
                let stamp: String = fetch.get("created_at");
                if DateTime::parse_from_rfc3339(&stamp)?.with_timezone(&Utc) <= cutoff {
                    fetched_at = Some(stamp);
                    break;
                }
            }
            let policy = policies
                .get(source)
                .cloned()
                .unwrap_or_else(|| "index".into());
            let path = if historical_snapshot {
                "historical_snapshot"
            } else if upstream.is_some() {
                "indexed_upstream_cache"
            } else {
                "indexed_document"
            };
            let error = upstream
                .as_ref()
                .and_then(|u| u.last_error.clone())
                .map(|e| {
                    format!(
                        "Current connector status (may be later than the observation cutoff): {e}"
                    )
                });
            let body: String = row.get("body");
            for (n, line) in body.lines().enumerate() {
                if line.trim().is_empty() || line.len() > 2000 {
                    continue;
                }
                let lower = line.to_lowercase();
                let hits = words
                    .iter()
                    .filter(|w| lower.contains(w.as_str()))
                    .cloned()
                    .collect::<Vec<_>>();
                if hits.is_empty() {
                    continue;
                }
                passage_matches += 1;
                passages.push(AnswerPassage{source_id:source.clone(),source_title:row.get("title"),evidence:Evidence{document_id:id.clone(),revision:row.get("revision"),line:n as i64+1,quote:line.into()},observed_at:row.get("updated_at"),validity:"No validity interval asserted for this passage; observation time is its import timestamp.".into(),match_terms:hits,retrieval_path:path.into(),retrieval_policy:policy.clone(),fetched_at:fetched_at.clone(),stale:true,error:error.clone()});
                passages.sort_by(|a, b| {
                    b.match_terms
                        .len()
                        .cmp(&a.match_terms.len())
                        .then_with(|| a.evidence.document_id.cmp(&b.evidence.document_id))
                        .then_with(|| a.evidence.line.cmp(&b.evidence.line))
                });
                passages.truncate(20);
            }
        }
    }
    let mut missing = vec![];
    if !live_only_excluded.is_empty() {
        missing.push(format!("{} selected live-only source(s) were excluded from indexed claims/passages. Use explicitly approved federated live retrieval for current data; no live fetch was made.", live_only_excluded.len()));
    }
    if historical_snapshot {
        missing.push("This is an explicit historical snapshot query. Stored revisions are labeled historical_snapshot and never represented as live results; current upstream freshness is unverified.".into());
    }
    if claims.is_empty() {
        missing.push("No matching explicit dated claims establish an answer for the requested validity date and observation cutoff.".into());
    }
    if passages.is_empty() {
        missing.push("No matching source passages were found in the permitted source scope at the observation cutoff.".into());
    }
    if claims.iter().any(|c| c.certainty == "uncertain") {
        missing.push("Some registered claims are explicitly uncertain.".into());
    }
    if !contradictions.is_empty() {
        missing.push("Different values under the same registered facet require review before choosing an answer.".into());
    }
    if contradictions_omitted > 0 {
        missing.push(format!("{contradictions_omitted} additional differing-value groups omitted; narrow the question."));
    }
    missing.push("Passage observation dates are import dates, not proof that their content was valid on the requested date. Retrieval is lexical; absent matches do not establish absence of information.".into());
    let answer = if claims.is_empty() {
        if passages.is_empty() {
            "Insufficient evidence in the selected scope.".into()
        } else {
            "Matching source passages are available below. They do not establish a dated answer by themselves.".into()
        }
    } else {
        claims
            .iter()
            .take(8)
            .map(|c| {
                format!(
                    "{} · {}: {} [{}; claim {} v{}]",
                    c.subject, c.predicate, c.value, c.certainty, c.id, c.version
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    };
    let mut result=KnowledgeAnswer{project_id:req.project_id,question:req.question,valid_as_of:day,known_by,generated_at:now(),answer,claims,passages,contradictions,missing_evidence:missing,source_ids:scope,claims_omitted,passages_omitted:passage_matches,method:"Local extractive evidence panel; user-authored dated claims; no generated synthesis or semantic inference".into()};
    while serde_json::to_vec(&result)?.len() > 235000 {
        if result.passages.pop().is_some() {
            continue;
        }
        if result.claims.pop().is_some() {
            result.claims_omitted += 1;
            continue;
        }
        bail!("invalid: evidence metadata exceeds response budget");
    }
    result.answer = if result.claims.is_empty() {
        if result.passages.is_empty() {
            "Insufficient evidence in the selected scope.".into()
        } else {
            "Matching source passages are available below. They do not establish a dated answer by themselves.".into()
        }
    } else {
        result
            .claims
            .iter()
            .take(8)
            .map(|c| {
                format!(
                    "{} · {}: {} [{}; claim {} v{}]",
                    c.subject, c.predicate, c.value, c.certainty, c.id, c.version
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    };
    result.passages_omitted = passage_matches.saturating_sub(result.passages.len());
    Ok(result)
}
impl Store {
    pub async fn answer_knowledge(
        &self,
        req: KnowledgeAnswerRequest,
        access: &Access,
    ) -> Result<KnowledgeAnswer> {
        let mut tx = self.pool.begin().await?;
        let result = build(&mut tx, req, access).await?;
        tx.commit().await?;
        Ok(result)
    }
    pub async fn compare_knowledge(
        &self,
        req: KnowledgeAnswerRequest,
        access: &Access,
    ) -> Result<KnowledgeComparison> {
        let mut tx = self.pool.begin().await?;
        let requested = build(&mut tx, req.clone(), access).await?;
        let mut current = req;
        current.valid_as_of = Some(Utc::now().date_naive().to_string());
        current.known_by = None;
        let today = build(&mut tx, current, access).await?;
        let mut pairs = BTreeMap::<(String, String), (Vec<String>, Vec<String>)>::new();
        for c in &requested.claims {
            pairs
                .entry((c.subject.clone(), c.predicate.clone()))
                .or_default()
                .0
                .push(c.value.clone());
        }
        for c in &today.claims {
            pairs
                .entry((c.subject.clone(), c.predicate.clone()))
                .or_default()
                .1
                .push(c.value.clone());
        }
        let mut changes = vec![];
        for ((subject, predicate), (mut before, mut after)) in pairs {
            before.sort();
            before.dedup();
            after.sort();
            after.dedup();
            if before != after {
                changes.push(KnowledgeFacetChange {
                    subject,
                    predicate,
                    before,
                    today: after,
                });
            }
        }
        tx.commit().await?;
        Ok(KnowledgeComparison {
            requested,
            today,
            changes,
        })
    }
}
