use super::*;
use crate::store::hash;

fn enabled(p: &PluginRecord, key: &str) -> bool {
    p.configuration[key].as_bool().unwrap_or(true)
}
fn bounded(value: &str, max: usize) -> String {
    value
        .chars()
        .scan(0, |bytes, c| {
            *bytes += c.len_utf8();
            (*bytes <= max).then_some(c)
        })
        .collect()
}
fn candidate(
    kind: &str,
    id: &str,
    version: i64,
    title: &str,
    contribution: &str,
    note: &str,
    sources: Vec<String>,
    evidence: Vec<Evidence>,
    date: Option<String>,
) -> BragbookEntry {
    let timestamp = now();
    BragbookEntry {
        id: uuid::Uuid::new_v4().to_string(),
        version: 1,
        status: "candidate".into(),
        title: bounded(title, 300),
        contribution: bounded(contribution, 4000),
        impact: String::new(),
        occurred_on: date,
        source_ids: sources,
        evidence,
        origin_kind: kind.into(),
        origin_id: id.into(),
        origin_version: version,
        origin_note: note.into(),
        collected_at: timestamp.clone(),
        updated_at: timestamp,
        reviewed_at: None,
        stale_evidence: false,
        support_changed: false,
    }
}
async fn insert(
    tx: &mut Transaction<'_, Sqlite>,
    p: &PluginRecord,
    key: &str,
    item: BragbookEntry,
) -> Result<usize> {
    if item.evidence.is_empty()
        || item.evidence.len() > 10
        || item
            .evidence
            .iter()
            .any(|e| e.quote.len() > 2000 || e.line < 1)
    {
        return Ok(0);
    }
    if allowed(tx, p, &item, &Access::admin()).await.is_err() {
        return Ok(0);
    }
    // Validate historical evidence before persisting, even for first-party effects.
    for e in &item.evidence {
        let row=sqlx::query("SELECT d.source_id,r.body FROM documents d JOIN revisions r ON r.document_id=d.id WHERE d.id=? AND r.revision=?")
            .bind(&e.document_id).bind(e.revision).fetch_optional(&mut **tx).await?;
        let Some(row) = row else {
            return Ok(0);
        };
        if !item.source_ids.contains(&row.get::<String, _>("source_id"))
            || row
                .get::<String, _>("body")
                .lines()
                .nth(e.line as usize - 1)
                != Some(e.quote.as_str())
        {
            return Ok(0);
        }
    }
    let exists: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM bragbook_entries WHERE origin_key=?)")
            .bind(key)
            .fetch_one(&mut **tx)
            .await?;
    if exists {
        return Ok(0);
    }
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM bragbook_entries")
        .fetch_one(&mut **tx)
        .await?;
    if count >= 10000 {
        bail!("invalid: bragbook reached its 10,000 entry limit");
    }
    sqlx::query("INSERT INTO bragbook_entries(id,origin_key,status,version,collected_at,record) VALUES(?,?,?,1,?,?)")
        .bind(&item.id).bind(key).bind(&item.status).bind(&item.collected_at).bind(serde_json::to_string(&item)?).execute(&mut **tx).await?;
    save(tx, &item, ID).await?;
    Ok(1)
}
async fn collect_task(
    tx: &mut Transaction<'_, Sqlite>,
    p: &PluginRecord,
    task: &Task,
    recorded_at: Option<&str>,
) -> Result<usize> {
    if !enabled(p, "completed-tasks") || task.status != "completed" {
        return Ok(0);
    }
    let source: Option<String> = sqlx::query_scalar("SELECT source_id FROM documents WHERE id=?")
        .bind(&task.evidence.document_id)
        .fetch_optional(&mut **tx)
        .await?;
    let Some(source) = source else {
        return Ok(0);
    };
    let item = candidate(
        "task",
        &task.id,
        task.version,
        &task.title,
        &task.title,
        &format!(
            "Task marked completed. Recorded owner: {}. Confirm your own contribution; completion does not establish impact.",
            task.owner.as_deref().unwrap_or("unassigned")
        ),
        vec![source],
        vec![task.evidence.clone()],
        recorded_at.map(|d| d[..10].to_string()),
    );
    insert(tx, p, &format!("task:{}", task.id), item).await
}
async fn collect_delivery(
    tx: &mut Transaction<'_, Sqlite>,
    p: &PluginRecord,
    id: &str,
) -> Result<usize> {
    if !enabled(p, "confirmed-deliveries") {
        return Ok(0);
    }
    let record: Option<String> =
        sqlx::query_scalar("SELECT record FROM delivery_commitments WHERE id=?")
            .bind(id)
            .fetch_optional(&mut **tx)
            .await?;
    let Some(record) = record else {
        return Ok(0);
    };
    let c: Commitment = serde_json::from_str(&record)?;
    if c.status != "completed" {
        return Ok(0);
    }
    // A task and its linked delivery are one accomplishment, not two.
    let key = c
        .task_id
        .as_ref()
        .map(|id| format!("task:{id}"))
        .unwrap_or_else(|| format!("delivery:{}", c.id));
    let mut evidence = c.promise_evidence.clone();
    if let Some(proposal) = &c.completion_proposal {
        for link in c.links.iter().filter(|l| proposal.link_ids.contains(&l.id)) {
            evidence.extend(link.evidence.clone());
        }
    }
    evidence.dedup();
    evidence.truncate(10);
    let item = candidate(
        "delivery",
        &c.id,
        c.version,
        &c.title,
        &c.title,
        &format!(
            "Delivery completion explicitly confirmed. Recorded owner: {}. Confirm your contribution and any impact.",
            c.owner
        ),
        c.source_ids,
        evidence,
        Some(c.updated_at[..10].into()),
    );
    insert(tx, p, &key, item).await
}
async fn collect_document(
    tx: &mut Transaction<'_, Sqlite>,
    p: &PluginRecord,
    id: &str,
    revision: i64,
) -> Result<BragbookCollection> {
    let mut result = BragbookCollection::default();
    if !enabled(p, "source-markers") {
        return Ok(result);
    }
    let row=sqlx::query("SELECT d.source_id,r.body FROM documents d JOIN revisions r ON r.document_id=d.id WHERE d.id=? AND r.revision=?")
        .bind(id).bind(revision).fetch_optional(&mut **tx).await?;
    let Some(row) = row else {
        return Ok(result);
    };
    let source: String = row.get("source_id");
    if contributions::scope_tx(tx, p, std::slice::from_ref(&source), &[])
        .await
        .is_err()
    {
        return Ok(result);
    }
    result.documents_considered = 1;
    let body: String = row.get("body");
    let mut matched = 0;
    for (index, line) in body.lines().enumerate() {
        let trimmed = line.trim().trim_start_matches(['-', '*']).trim();
        let Some((prefix, text)) = trimmed.split_once(':') else {
            continue;
        };
        if !["accomplishment", "achievement", "impact", "shipped"]
            .contains(&prefix.to_ascii_lowercase().as_str())
            || text.trim().is_empty()
        {
            continue;
        }
        if line.len() > 2000 || matched >= 100 {
            result.skipped_lines += 1;
            continue;
        }
        matched += 1;
        let item = candidate(
            "source",
            id,
            revision,
            text.trim(),
            text.trim(),
            "Explicit accomplishment marker in a source. The event date and personal ownership are unverified.",
            vec![source.clone()],
            vec![Evidence {
                document_id: id.into(),
                revision,
                line: index as i64 + 1,
                quote: line.into(),
            }],
            None,
        );
        // Stable across line moves, unchanged revisions and worker restarts.
        let key = format!("source:{id}:{}", hash(&trimmed.to_lowercase()));
        result.created += insert(tx, p, &key, item).await?;
    }
    Ok(result)
}
async fn invalidate(tx: &mut Transaction<'_, Sqlite>, kind: &str, id: &str) -> Result<()> {
    let rows=sqlx::query_scalar::<_,String>("SELECT record FROM bragbook_entries WHERE json_extract(record,'$.origin_kind')=? AND json_extract(record,'$.origin_id')=? AND status='accepted'")
        .bind(kind).bind(id).fetch_all(&mut **tx).await?;
    for record in rows {
        let item: BragbookEntry = serde_json::from_str(&record)?;
        let mut item = refresh(tx, item).await?;
        if item.support_changed {
            item.status = "candidate".into();
            item.reviewed_at = None;
            item.version += 1;
            item.updated_at = now();
            save(tx, &item, ID).await?;
        }
    }
    Ok(())
}

impl Store {
    /// Durable event cursor and effects advance together. Disabled periods are
    /// skipped by the ordinary plugin enable cursor; backfill is explicit.
    pub async fn collect_bragbook_events(&self) -> Result<usize> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let p = load(&mut tx, ID).await?;
        if !p.enabled || !p.grant.operations.iter().any(|o| o == "sources.read") {
            return Ok(0);
        }
        let cursor: i64 = sqlx::query_scalar(
            "SELECT COALESCE((SELECT sequence FROM plugin_cursors WHERE plugin_id=?),0)",
        )
        .bind(ID)
        .fetch_one(&mut *tx)
        .await?;
        let rows=sqlx::query("SELECT sequence,kind,record_id,record_revision FROM plugin_events WHERE sequence>? ORDER BY sequence LIMIT 20")
            .bind(cursor).fetch_all(&mut *tx).await?;
        let mut last = cursor;
        let mut count = 0;
        for row in rows {
            last = row.get("sequence");
            let kind: String = row.get("kind");
            let id: String = row.get("record_id");
            let revision: i64 = row.get("record_revision");
            match kind.as_str() {
                "document.revision.created" => {
                    count += collect_document(&mut tx, &p, &id, revision).await?.created;
                }
                "task.completed" => {
                    let record: Option<String> = sqlx::query_scalar(
                        "SELECT record FROM insight_task_history WHERE task_id=? AND version=?",
                    )
                    .bind(&id)
                    .bind(revision)
                    .fetch_optional(&mut *tx)
                    .await?;
                    if let Some(record) = record {
                        let transition: TaskTransition = serde_json::from_str(&record)?;
                        count += collect_task(
                            &mut tx,
                            &p,
                            &transition.task,
                            Some(&transition.recorded_at),
                        )
                        .await?;
                    }
                }
                "task.changed" | "task.accepted" => {
                    invalidate(&mut tx, "task", &id).await?;
                }
                "delivery.changed" => {
                    invalidate(&mut tx, "delivery", &id).await?;
                    count += collect_delivery(&mut tx, &p, &id).await?;
                }
                _ => {}
            }
        }
        sqlx::query("INSERT INTO plugin_cursors(plugin_id,sequence) VALUES(?,?) ON CONFLICT(plugin_id) DO UPDATE SET sequence=excluded.sequence")
            .bind(ID).bind(last).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(count)
    }
    /// Explicit backfill of bounded current work. It never assigns an unknown
    /// completion date from a task's import or migration timestamp.
    pub async fn collect_existing_bragbook(&self, access: &Access) -> Result<BragbookCollection> {
        admin(access)?;
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let p = load(&mut tx, ID).await?;
        if !p.enabled || !p.grant.operations.iter().any(|o| o == "sources.read") {
            bail!("invalid: enable Bragbook and grant source reading first");
        }
        let mut sources = p.grant.source_ids.clone();
        for project in &p.grant.project_ids {
            sources.extend(crate::continuity::project_sources(&mut tx, project).await?);
        }
        sources.sort();
        sources.dedup();
        let scope = serde_json::to_string(&sources)?;
        let docs=sqlx::query("SELECT id,revision FROM documents WHERE source_id IN (SELECT value FROM json_each(?)) ORDER BY id LIMIT 201")
            .bind(&scope).fetch_all(&mut *tx).await?;
        let tasks=sqlx::query("SELECT * FROM tasks WHERE status='completed' AND document_id IN (SELECT id FROM documents WHERE source_id IN (SELECT value FROM json_each(?))) ORDER BY id LIMIT 1001")
            .bind(&scope).fetch_all(&mut *tx).await?;
        let deliveries=sqlx::query_scalar::<_,String>("SELECT id FROM delivery_commitments WHERE json_extract(record,'$.status')='completed' AND NOT EXISTS(SELECT 1 FROM json_each(record,'$.source_ids') s WHERE s.value NOT IN (SELECT value FROM json_each(?))) ORDER BY id LIMIT 1001")
            .bind(&scope).fetch_all(&mut *tx).await?;
        if docs.len() > 200 || tasks.len() > 1000 || deliveries.len() > 1000 {
            bail!(
                "invalid: narrow Bragbook to at most 200 documents, 1000 completed tasks and 1000 deliveries per backfill"
            );
        }
        let mut result = BragbookCollection::default();
        for row in docs {
            let r = collect_document(&mut tx, &p, row.get("id"), row.get("revision")).await?;
            result.created += r.created;
            result.documents_considered += r.documents_considered;
            result.skipped_lines += r.skipped_lines;
        }
        // Prefer the richer delivery trace when the task and promise are linked.
        for id in deliveries {
            result.created += collect_delivery(&mut tx, &p, &id).await?;
        }
        for row in tasks {
            let task = crate::store::task_row(&row);
            let record:Option<String>=sqlx::query_scalar("SELECT record FROM insight_task_history WHERE task_id=? AND json_extract(record,'$.task.status')='completed' AND json_extract(record,'$.previous_status')!='completed' AND json_extract(record,'$.origin')!='migration_baseline_prior_history_unknown' ORDER BY version DESC LIMIT 1")
                .bind(&task.id).fetch_optional(&mut *tx).await?;
            let transition = record
                .map(|s| serde_json::from_str::<TaskTransition>(&s))
                .transpose()?;
            result.created += collect_task(
                &mut tx,
                &p,
                &task,
                transition.as_ref().map(|t| t.recorded_at.as_str()),
            )
            .await?;
        }
        tx.commit().await?;
        Ok(result)
    }
}
