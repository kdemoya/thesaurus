//! Durable promises, exact revision evidence and explicit completion review.
mod repository;
use crate::{
    Store,
    continuity::{event, operation, permit, project_sources, remember},
    store::{now, validate_text},
};
use anyhow::{Context, Result, bail};
use chrono::{NaiveDate, Utc};
use sqlx::{Row, Sqlite, Transaction};
use thesaurus_contracts::*;

fn date(s: &str) -> Result<NaiveDate> {
    let d = NaiveDate::parse_from_str(s, "%Y-%m-%d").context("invalid date; use YYYY-MM-DD")?;
    if d.to_string() != s {
        bail!("invalid date; use YYYY-MM-DD");
    }
    Ok(d)
}
fn note(s: &str) -> Result<()> {
    if s.len() > 2000 || s.contains('\0') {
        bail!("invalid: note must be at most 2000 bytes");
    }
    Ok(())
}
async fn raw(tx: &mut Transaction<'_, Sqlite>, id: &str) -> Result<Commitment> {
    let json: String = sqlx::query_scalar("SELECT record FROM delivery_commitments WHERE id=?")
        .bind(id)
        .fetch_optional(&mut **tx)
        .await?
        .context("record not found")?;
    Ok(serde_json::from_str(&json)?)
}
async fn citations(
    tx: &mut Transaction<'_, Sqlite>,
    project: &str,
    evidence: &[Evidence],
) -> Result<Vec<String>> {
    if evidence.is_empty() || evidence.len() > 6 {
        bail!("invalid: choose 1–6 exact source citations");
    }
    let scope = project_sources(tx, project).await?;
    let mut ids = vec![];
    for e in evidence {
        if e.line < 1 || e.quote.len() > 2000 || e.quote.trim().is_empty() {
            bail!("invalid: evidence requires a nonempty exact line up to 2000 bytes");
        }
        let row=sqlx::query("SELECT d.source_id,r.body FROM documents d JOIN revisions r ON r.document_id=d.id WHERE d.id=? AND r.revision=?").bind(&e.document_id).bind(e.revision).fetch_optional(&mut **tx).await?.context("invalid evidence revision")?;
        let source: String = row.get("source_id");
        if !scope.contains(&source)
            || row
                .get::<String, _>("body")
                .lines()
                .nth(e.line as usize - 1)
                != Some(e.quote.as_str())
        {
            bail!("invalid evidence scope or locator");
        }
        ids.push(source);
    }
    ids.sort();
    ids.dedup();
    Ok(ids)
}
async fn history(
    tx: &mut Transaction<'_, Sqlite>,
    id: &str,
    offset: usize,
    limit: usize,
) -> Result<Vec<CommitmentEvent>> {
    let rows=sqlx::query("SELECT sequence,record FROM delivery_history WHERE commitment_id=? ORDER BY sequence DESC LIMIT ? OFFSET ?").bind(id).bind(limit as i64).bind(offset as i64).fetch_all(&mut **tx).await?;
    let mut events = vec![];
    for row in rows {
        let mut e: CommitmentEvent = serde_json::from_str(row.get("record"))?;
        e.sequence = row.get("sequence");
        events.push(e);
    }
    Ok(events)
}
async fn refresh(
    tx: &mut Transaction<'_, Sqlite>,
    mut item: Commitment,
    include_history: bool,
) -> Result<Commitment> {
    item.stale_evidence = false;
    for e in item
        .promise_evidence
        .iter()
        .chain(item.links.iter().flat_map(|l| l.evidence.iter()))
    {
        let current: Option<i64> = sqlx::query_scalar("SELECT revision FROM documents WHERE id=?")
            .bind(&e.document_id)
            .fetch_optional(&mut **tx)
            .await?;
        if current != Some(e.revision) {
            item.stale_evidence = true;
        }
    }
    item.linked_task_status = if let Some(id) = &item.task_id {
        sqlx::query_scalar("SELECT status FROM tasks WHERE id=?")
            .bind(id)
            .fetch_optional(&mut **tx)
            .await?
    } else {
        None
    };
    item.history = if include_history {
        history(tx, &item.id, 0, 30).await?
    } else {
        vec![]
    };
    while serde_json::to_vec(&item)?.len() > 180000 {
        if item.history.pop().is_none() {
            bail!("invalid: commitment exceeds the response limit");
        }
    }
    item.history_omitted = item.history_count.saturating_sub(item.history.len());
    Ok(item)
}
async fn save(
    tx: &mut Transaction<'_, Sqlite>,
    item: &Commitment,
    action: &str,
    note: &str,
    evidence: &[Evidence],
    link_ids: &[String],
) -> Result<()> {
    let mut meta = item.clone();
    meta.history.clear();
    meta.history_omitted = 0;
    meta.stale_evidence = false;
    meta.linked_task_status = None;
    if serde_json::to_vec(&meta)?.len() > 120000 {
        bail!("invalid: commitment reached its evidence size limit; start a successor commitment");
    }
    sqlx::query("INSERT INTO delivery_commitments(id,project_id,record) VALUES(?,?,?) ON CONFLICT(id) DO UPDATE SET record=excluded.record").bind(&item.id).bind(&item.project_id).bind(serde_json::to_string(&meta)?).execute(&mut **tx).await?;
    let e = CommitmentEvent {
        sequence: 0,
        action: action.into(),
        note: note.into(),
        contributor: "You".into(),
        created_at: now(),
        version: item.version,
        title: item.title.clone(),
        owner: item.owner.clone(),
        due: item.due.clone(),
        status: item.status.clone(),
        evidence: evidence.to_vec(),
        link_ids: link_ids.to_vec(),
    };
    sqlx::query("INSERT INTO delivery_history(commitment_id,record) VALUES(?,?)")
        .bind(&item.id)
        .bind(serde_json::to_string(&e)?)
        .execute(&mut **tx)
        .await?;
    event(tx, "commitment", &item.id).await
}
impl Store {
    pub async fn migrate_delivery(&self) -> Result<()> {
        sqlx::raw_sql(include_str!("schema.sql"))
            .execute(&self.pool)
            .await?;
        Ok(())
    }
    pub async fn commitment(&self, id: &str, access: &Access) -> Result<Commitment> {
        let mut tx = self.pool.begin().await?;
        let item = raw(&mut tx, id).await?;
        permit(&mut tx, &item.source_ids, access).await?;
        if !repository::allowed(&mut tx, &item, access).await? {
            bail!("record not found");
        }
        let item = refresh(&mut tx, item, true).await?;
        tx.commit().await?;
        Ok(item)
    }
    pub async fn commitment_list(
        &self,
        project: &str,
        query: CommitmentQuery,
        access: &Access,
    ) -> Result<CommitmentList> {
        let status = query.status.as_deref().unwrap_or("open");
        if !["open", "proposed", "completed", "cancelled", "all"].contains(&status) {
            bail!("invalid commitment status");
        }
        let offset = query.offset.unwrap_or(0) as usize;
        let limit = query.limit.unwrap_or(25) as usize;
        if offset > 10000 || !(1..=50).contains(&limit) {
            bail!("invalid: offset 0–10000, limit 1–50");
        }
        let mut tx = self.pool.begin().await?;
        let scope = project_sources(&mut tx, project).await?;
        let allowed: Vec<String> = sqlx::query("SELECT id,allow_external FROM sources ORDER BY id")
            .fetch_all(&mut *tx)
            .await?
            .into_iter()
            .filter(|r| access.permits(r.get("id"), r.get("allow_external")))
            .map(|r| r.get("id"))
            .collect();
        if !scope.iter().any(|s| allowed.contains(s)) {
            bail!("record not found");
        }
        let mut items = vec![];
        let mut total = 0;
        for row in sqlx::query("SELECT record FROM delivery_commitments WHERE project_id=? ORDER BY json_extract(record,'$.updated_at') DESC,id").bind(project).fetch_all(&mut *tx).await? {
            let item:Commitment=serde_json::from_str(row.get("record"))?;
            if !item.source_ids.iter().all(|s|allowed.contains(s)) || (status!="all" && item.status!=status) || !repository::allowed(&mut tx,&item,access).await? {continue;}
            if total>=offset && items.len()<limit {items.push(refresh(&mut tx,item,false).await?);}total+=1;
        }
        while serde_json::to_vec(&items)?.len() > 300000 {
            if items.pop().is_none() {
                break;
            }
        }
        let next_offset = (offset + items.len() < total).then_some(offset + items.len());
        tx.commit().await?;
        Ok(CommitmentList {
            project_id: project.into(),
            items,
            total,
            offset,
            limit,
            next_offset,
        })
    }
    pub async fn commitment_history(
        &self,
        id: &str,
        query: CommitmentQuery,
        access: &Access,
    ) -> Result<CommitmentHistory> {
        let offset = query.offset.unwrap_or(0) as usize;
        let limit = query.limit.unwrap_or(25) as usize;
        if offset > 100000 || !(1..=50).contains(&limit) {
            bail!("invalid history page");
        }
        let mut tx = self.pool.begin().await?;
        let item = raw(&mut tx, id).await?;
        permit(&mut tx, &item.source_ids, access).await?;
        if !repository::allowed(&mut tx, &item, access).await? {
            bail!("record not found");
        }
        let mut items = history(&mut tx, id, offset, limit).await?;
        while serde_json::to_vec(&items)?.len() > 180000 {
            if items.pop().is_none() {
                break;
            }
        }
        let next_offset =
            (offset + items.len() < item.history_count).then_some(offset + items.len());
        tx.commit().await?;
        Ok(CommitmentHistory {
            commitment_id: id.into(),
            items,
            total: item.history_count,
            offset,
            next_offset,
        })
    }
    pub async fn create_commitment(&self, mut req: CommitmentCreateRequest) -> Result<Commitment> {
        validate_text(&req.title, 300, "commitment title")?;
        validate_text(&req.owner, 200, "commitment owner")?;
        note(&req.note)?;
        let promised = date(&req.promised_on)?;
        if promised > Utc::now().date_naive() {
            bail!("invalid: promise date cannot be in the future");
        }
        if let Some(due) = &req.due {
            if date(due)? < promised {
                bail!("invalid: due date precedes promise date");
            }
        }
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let (digest, old) =
            operation(&mut tx, "commitment-create", "", &req.idempotency_key, &req).await?;
        let id = if let Some(id) = old {
            id
        } else {
            let scope = project_sources(&mut tx, &req.project_id).await?;
            if let Some(task) = &req.task_id {
                let row=sqlx::query("SELECT t.*,d.source_id FROM tasks t JOIN documents d ON d.id=t.document_id WHERE t.id=?").bind(task).fetch_optional(&mut *tx).await?.context("record not found")?;
                if row.get::<String, _>("status") != "accepted"
                    || !scope.contains(&row.get::<String, _>("source_id"))
                {
                    bail!("invalid: choose an accepted task in this project");
                }
                let e = crate::store::task_row(&row).evidence;
                if !req.evidence.contains(&e) {
                    req.evidence.push(e);
                }
            }
            let source_ids = citations(&mut tx, &req.project_id, &req.evidence).await?;
            let count: i64 =
                sqlx::query_scalar("SELECT COUNT(*) FROM delivery_commitments WHERE project_id=?")
                    .bind(&req.project_id)
                    .fetch_one(&mut *tx)
                    .await?;
            if count >= 10000 {
                bail!("invalid: project commitment limit reached");
            }
            let item = Commitment {
                id: uuid::Uuid::new_v4().to_string(),
                project_id: req.project_id.clone(),
                task_id: req.task_id.clone(),
                title: req.title.trim().into(),
                owner: req.owner.trim().into(),
                promised_on: req.promised_on.clone(),
                due: req.due.clone(),
                status: "open".into(),
                version: 1,
                source_ids,
                promise_evidence: req.evidence.clone(),
                links: vec![],
                completion_proposal: None,
                created_at: now(),
                updated_at: now(),
                stale_evidence: false,
                linked_task_status: None,
                history_count: 1,
                history: vec![],
                history_omitted: 0,
            };
            save(&mut tx, &item, "created", &req.note, &req.evidence, &[]).await?;
            remember(&mut tx, &req.idempotency_key, &digest, &item.id).await?;
            item.id
        };
        tx.commit().await?;
        self.commitment(&id, &Access::admin()).await
    }
    pub async fn update_commitment(
        &self,
        id: &str,
        req: CommitmentUpdateRequest,
    ) -> Result<Commitment> {
        note(&req.note)?;
        if ![
            "amend",
            "link",
            "propose_completion",
            "confirm_completion",
            "reject_completion",
            "cancel",
            "reopen",
        ]
        .contains(&req.action.as_str())
        {
            bail!("invalid commitment action");
        }
        if req.action != "amend"
            && (req.title.is_some() || req.owner.is_some() || req.due.is_some() || req.clear_due)
        {
            bail!("invalid: promise changes require amend");
        }
        if req.action != "link" && req.kind.is_some() {
            bail!("invalid: link kind requires link action");
        }
        if req.action != "propose_completion" && !req.link_ids.is_empty() {
            bail!("invalid: link_ids require propose_completion");
        }
        if !["amend", "link"].contains(&req.action.as_str()) && !req.evidence.is_empty() {
            bail!("invalid: attach evidence through link or amend first");
        }
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let (digest, old) =
            operation(&mut tx, "commitment-update", id, &req.idempotency_key, &req).await?;
        if old.is_none() {
            let mut item = raw(&mut tx, id).await?;
            if item.version != req.expected_version {
                bail!("conflict: commitment changed; refresh before editing");
            }
            if req.action == "reopen" {
                if !["completed", "cancelled"].contains(&item.status.as_str()) {
                    bail!("conflict: commitment is already active");
                }
            } else if ["completed", "cancelled"].contains(&item.status.as_str()) {
                bail!("conflict: reopen commitment before editing");
            }
            if req.action == "amend" {
                if req.evidence.is_empty() {
                    bail!("invalid: promise amendments require exact supporting evidence");
                }
                if let Some(title) = &req.title {
                    validate_text(title, 300, "commitment title")?;
                    item.title = title.trim().into();
                }
                if let Some(owner) = &req.owner {
                    validate_text(owner, 200, "commitment owner")?;
                    item.owner = owner.trim().into();
                }
                if req.clear_due && req.due.is_some() {
                    bail!("invalid: choose due or clear_due");
                }
                if req.clear_due {
                    item.due = None;
                } else if let Some(d) = &req.due {
                    if date(d)? < date(&item.promised_on)? {
                        bail!("invalid: due date precedes promise");
                    }
                    item.due = Some(d.clone());
                }
            }
            let mut added_link_ids = vec![];
            if ["amend", "link"].contains(&req.action.as_str()) {
                let kind = if req.action == "amend" {
                    "amendment"
                } else {
                    req.kind
                        .as_deref()
                        .context("invalid: choose discussion or implementation link kind")?
                };
                if !["amendment", "discussion", "implementation"].contains(&kind) {
                    bail!("invalid delivery link kind");
                }
                if req.action == "link" && kind == "amendment" {
                    bail!("invalid: use amend to revise a promise");
                }
                let sources = citations(&mut tx, &item.project_id, &req.evidence).await?;
                if item.links.len() >= 100 {
                    bail!("invalid: commitment already has 100 evidence links");
                }
                item.source_ids.extend(sources);
                item.source_ids.sort();
                item.source_ids.dedup();
                let link = DeliveryLink {
                    id: uuid::Uuid::new_v4().to_string(),
                    kind: kind.into(),
                    note: req.note.clone(),
                    evidence: req.evidence.clone(),
                    repository: None,
                    contributor: "You".into(),
                    created_at: now(),
                };
                added_link_ids.push(link.id.clone());
                item.links.push(link);
                // Changing the evidence or promise always invalidates a prior completion proposal.
                item.completion_proposal = None;
                item.status = "open".into();
            }
            match req.action.as_str() {
                "propose_completion" => {
                    validate_text(&req.note, 2000, "completion rationale")?;
                    if item.status == "proposed" {
                        bail!("conflict: review the existing completion proposal first");
                    }
                    if req.link_ids.is_empty() || req.link_ids.len() > 20 {
                        bail!("invalid: choose 1–20 implementation evidence links");
                    }
                    let mut ids = req.link_ids.clone();
                    ids.sort();
                    ids.dedup();
                    if !ids.iter().all(|id| {
                        item.links
                            .iter()
                            .any(|l| &l.id == id && l.kind == "implementation")
                    }) {
                        bail!(
                            "invalid: completion requires existing implementation evidence links"
                        );
                    }
                    item.completion_proposal = Some(CompletionProposal {
                        link_ids: ids.clone(),
                        rationale: req.note.clone(),
                        contributor: "You".into(),
                        proposed_at: now(),
                        proposed_version: item.version + 1,
                    });
                    item.status = "proposed".into();
                    added_link_ids = ids;
                }
                "confirm_completion" => {
                    let proposal = item
                        .completion_proposal
                        .as_ref()
                        .context("conflict: propose completion with evidence before confirming")?;
                    if item.status != "proposed" || proposal.proposed_version != item.version {
                        bail!("conflict: completion proposal changed");
                    }
                    added_link_ids = proposal.link_ids.clone();
                    item.status = "completed".into();
                }
                "reject_completion" => {
                    if item.status != "proposed" {
                        bail!("conflict: no completion proposal to reject");
                    }
                    item.completion_proposal = None;
                    item.status = "open".into();
                }
                "cancel" => {
                    validate_text(&req.note, 2000, "cancellation reason")?;
                    item.status = "cancelled".into();
                    item.completion_proposal = None;
                }
                "reopen" => {
                    item.status = "open".into();
                    item.completion_proposal = None;
                }
                _ => {}
            }
            item.version += 1;
            item.updated_at = now();
            item.history_count += 1;
            save(
                &mut tx,
                &item,
                &req.action,
                &req.note,
                &req.evidence,
                &added_link_ids,
            )
            .await?;
            remember(&mut tx, &req.idempotency_key, &digest, id).await?;
        }
        tx.commit().await?;
        self.commitment(id, &Access::admin()).await
    }
}
