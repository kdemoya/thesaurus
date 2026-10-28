use super::*;
use std::collections::HashSet;
fn question(line: &str) -> bool {
    let s = line.trim();
    let l = s.to_lowercase();
    s.ends_with('?')
        || ["question:", "open question:", "unanswered:", "tbd:"]
            .iter()
            .any(|p| l.starts_with(p))
}
fn terms(question: &str) -> Vec<String> {
    let stop = [
        "the", "and", "for", "that", "this", "with", "what", "when", "where", "which", "who",
        "why", "how", "does", "have", "has", "are", "was", "will", "would", "could", "should",
        "from", "into", "about", "our", "can", "not", "need", "know", "please", "any", "there",
    ];
    let mut words = question
        .to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|s| s.chars().count() >= 3 && !stop.contains(s))
        .map(String::from)
        .collect::<Vec<_>>();
    words.sort();
    words.dedup();
    words.truncate(20);
    words
}
async fn refresh(tx: &mut Transaction<'_, Sqlite>, project: &str) -> Result<DiscoveryRefresh> {
    let scope = project_sources(tx, project).await?;
    let docs = documents(tx, &scope).await?;
    let mut out = DiscoveryRefresh {
        project_id: project.into(),
        revisions_checked: 0,
        cues_created: 0,
        cues_updated: 0,
        suggestions_created: 0,
        cue_lines_omitted: 0,
        suggestions_omitted: 0,
        detection: DETECTION.into(),
    };
    let mut cue_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM discovery_cues WHERE project_id=?")
            .bind(project)
            .fetch_one(&mut **tx)
            .await?;
    for doc in &docs {
        let old: Option<i64> = sqlx::query_scalar(
            "SELECT revision FROM discovery_watermarks WHERE project_id=? AND document_id=?",
        )
        .bind(project)
        .bind(&doc.id)
        .fetch_optional(&mut **tx)
        .await?;
        if old == Some(doc.revision) {
            continue;
        }
        out.revisions_checked += 1;
        let kind: String = sqlx::query_scalar("SELECT kind FROM sources WHERE id=?")
            .bind(&doc.source_id)
            .fetch_one(&mut **tx)
            .await?;
        if ["transcript", "markdown"].contains(&kind.as_str()) {
            let mut count = 0;
            for (n, line) in doc.body.lines().enumerate() {
                if !question(line) {
                    continue;
                }
                if line.len() > 2000 || count >= 100 {
                    out.cue_lines_omitted += 1;
                    continue;
                }
                count += 1;
                let key = hash(&serde_json::to_string(&(
                    project,
                    &doc.source_id,
                    line.trim().to_lowercase(),
                ))?);
                let id = format!("cue-{}", &key[..32]);
                let evidence = Evidence {
                    document_id: doc.id.clone(),
                    revision: doc.revision,
                    line: n as i64 + 1,
                    quote: line.into(),
                };
                let exists: bool =
                    sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM discovery_cues WHERE id=?)")
                        .bind(&id)
                        .fetch_one(&mut **tx)
                        .await?;
                if !exists {
                    if cue_count >= 5000 {
                        out.cue_lines_omitted += 1;
                        continue;
                    }
                    let item=QuestionCue{id:id.clone(),project_id:project.into(),kind:"question_candidate".into(),text:line.trim().chars().take(250).collect(),status:"unreviewed".into(),source_ids:vec![doc.source_id.clone()],evidence:vec![evidence.clone()],original_evidence:vec![evidence],version:1,contributor:"Local explicit question rule".into(),first_seen:now(),updated_at:now(),note:"A question marker was observed; whether it is unanswered requires user review.".into()};
                    cues::save(tx, &item, "detected", "Local explicit question rule", "").await?;
                    cue_count += 1;
                    out.cues_created += 1;
                } else {
                    let mut item = cues::raw(tx, &id).await?;
                    if item.status != "resolved" && item.evidence != vec![evidence.clone()] {
                        item.evidence = vec![evidence];
                        item.version += 1;
                        item.updated_at = now();
                        cues::save(
                            tx,
                            &item,
                            "source_revision",
                            "Local explicit question rule",
                            "Dismissal and review status retained",
                        )
                        .await?;
                        out.cues_updated += 1;
                    }
                }
            }
        }
        sqlx::query("INSERT INTO discovery_watermarks(project_id,document_id,revision) VALUES(?,?,?) ON CONFLICT(project_id,document_id) DO UPDATE SET revision=excluded.revision").bind(project).bind(&doc.id).bind(doc.revision).execute(&mut **tx).await?;
    }
    let rows=sqlx::query("SELECT record FROM discovery_gaps WHERE project_id=? AND json_extract(record,'$.status') IN ('open','needs_review')").bind(project).fetch_all(&mut **tx).await?;
    for row in rows {
        let mut gap: KnowledgeGap = serde_json::from_str(row.get("record"))?;
        let mut pruned = 0;
        for s in gap.suggestions.iter().filter(|s| s.status == "dismissed") {
            sqlx::query("INSERT OR IGNORE INTO discovery_suggestion_dispositions(gap_id,suggestion_id,status) VALUES(?,?,'dismissed')").bind(&gap.id).bind(&s.id).execute(&mut **tx).await?;
            pruned += 1;
        }
        gap.suggestions.retain(|s| s.status != "dismissed");
        let mut changed = pruned > 0;
        let mut added = 0;
        let mut omitted = pruned;
        let query = terms(&gap.question);
        for doc in docs
            .iter()
            .filter(|d| gap.required_source_ids.contains(&d.source_id))
        {
            if gap.sources.iter().any(|s| {
                s.document_id == doc.id
                    && s.revision == doc.revision
                    && s.content_hash == doc.content_hash
            }) {
                continue;
            }
            changed = true;
            let mut candidates = vec![];
            if !query.is_empty() {
                for (n, line) in doc.body.lines().enumerate() {
                    if line.trim().is_empty() || line.len() > 2000 {
                        continue;
                    }
                    let wordset = line
                        .to_lowercase()
                        .split(|c: char| !c.is_alphanumeric())
                        .map(String::from)
                        .collect::<HashSet<_>>();
                    let hits = query
                        .iter()
                        .filter(|w| wordset.contains(*w))
                        .cloned()
                        .collect::<Vec<_>>();
                    if hits.len() >= query.len().min(2) {
                        candidates.push((n, line.to_string(), hits));
                    }
                }
            }
            candidates.sort_by_key(|(n, _, hits)| (std::cmp::Reverse(hits.len()), *n));
            for (index, (n, line, hits)) in candidates.into_iter().enumerate() {
                if index >= 10 || gap.suggestions.len() >= 60 {
                    omitted += 1;
                    continue;
                }
                let id = format!(
                    "gap-evidence-{}",
                    &hash(&format!("{}:{}:{}:{}", gap.id, doc.id, doc.revision, n))[..32]
                );
                if gap.suggestions.iter().any(|s| s.id == id) {
                    continue;
                }
                let dismissed:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM discovery_suggestion_dispositions WHERE gap_id=? AND suggestion_id=? AND status='dismissed')").bind(&gap.id).bind(&id).fetch_one(&mut **tx).await?;
                if dismissed {
                    continue;
                }
                let suggestion = GapSuggestion {
                    id,
                    evidence: Evidence {
                        document_id: doc.id.clone(),
                        revision: doc.revision,
                        line: n as i64 + 1,
                        quote: line,
                    },
                    source_id: doc.source_id.clone(),
                    matching_terms: hits,
                    status: "pending".into(),
                    discovered_at: now(),
                };
                if serde_json::to_vec(&gap)?.len() + serde_json::to_vec(&suggestion)?.len() > 130000
                {
                    omitted += 1;
                    continue;
                }
                gap.suggestions.push(suggestion);
                added += 1;
            }
            gap.sources.retain(|s| s.document_id != doc.id);
            gap.sources.push(BriefSourceRevision {
                source_id: doc.source_id.clone(),
                document_id: doc.id.clone(),
                title: doc.title.clone(),
                revision: doc.revision,
                content_hash: doc.content_hash.clone(),
            });
        }
        if changed {
            gap.sources
                .sort_by(|a, b| a.document_id.cmp(&b.document_id));
            gap.version += 1;
            gap.updated_at = now();
            gap.suggestions_omitted += omitted;
            if added > 0 {
                gap.status = "needs_review".into();
            }
            gaps::save(tx,&gap,"revisited","Local lexical rule",&format!("{added} possible relevant passages; {omitted} omitted. Relevance and sufficiency require user review.")).await?;
            out.suggestions_created += added;
            out.suggestions_omitted += omitted;
        }
    }
    Ok(out)
}
impl Store {
    /// Incremental local refresh. No paid model, network fetch or outbound notification.
    pub async fn refresh_discovery(&self, project: &str) -> Result<DiscoveryRefresh> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let result = refresh(&mut tx, project).await?;
        tx.commit().await?;
        Ok(result)
    }
    /// Call after a source import. Projects that contain that source revisit their durable questions.
    pub async fn refresh_discovery_document(&self, document: &str) -> Result<()> {
        let source: String = sqlx::query_scalar("SELECT source_id FROM documents WHERE id=?")
            .bind(document)
            .fetch_optional(&self.pool)
            .await?
            .context("record not found")?;
        let rows = sqlx::query("SELECT id,source_ids FROM project_briefs")
            .fetch_all(&self.pool)
            .await?;
        for row in rows {
            let ids: Vec<String> = serde_json::from_str(row.get("source_ids"))?;
            if ids.contains(&source) {
                self.refresh_discovery(&row.get::<String, _>("id")).await?;
            }
        }
        Ok(())
    }
}
