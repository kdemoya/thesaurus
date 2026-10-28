use super::*;
use crate::store::hash;
use serde_json::json;

const ALGORITHM: u32 = 1;
fn reference(e: &Evidence) -> EvidenceReference {
    EvidenceReference {
        document_id: e.document_id.clone(),
        revision: e.revision,
        line: e.line,
        quote: (e.quote.len() <= 1000).then(|| e.quote.clone()),
    }
}
fn task_ref(task: &Task) -> PriorityTask {
    PriorityTask {
        id: task.id.clone(),
        title: task.title.clone(),
        owner: task.owner.clone(),
        due: task.due.clone(),
        version: task.version,
        evidence: reference(&task.evidence),
    }
}
fn default_preference(project: &str, task: &str) -> PriorityPreference {
    PriorityPreference {
        project_id: project.into(),
        task_id: task.into(),
        importance: "normal".into(),
        note: String::new(),
        version: 0,
        updated_at: None,
    }
}
pub(crate) async fn preferences(
    tx: &mut Transaction<'_, Sqlite>,
    project: &str,
) -> Result<Vec<PriorityPreference>> {
    sqlx::query("SELECT record FROM priority_preferences WHERE project_id=? ORDER BY task_id")
        .bind(project)
        .fetch_all(&mut **tx)
        .await?
        .iter()
        .map(|r| Ok(serde_json::from_str(r.get("record"))?))
        .collect()
}
fn urgency(task: &Task, day: NaiveDate) -> PrioritySignal {
    let Some(due) = task.due.as_deref() else {
        return PrioritySignal {
            code: "deadline".into(),
            label: "No explicit deadline".into(),
            points: 0,
            state: "none".into(),
        };
    };
    let Ok(date) = date(due) else {
        return PrioritySignal {
            code: "deadline".into(),
            label: format!("Unrecognized deadline: {due}; no urgency points"),
            points: 0,
            state: format!("invalid:{due}"),
        };
    };
    let days = date.signed_duration_since(day).num_days();
    let (bucket, points) = if days < 0 {
        ("overdue", 60)
    } else if days == 0 {
        ("today", 50)
    } else if days <= 3 {
        ("within 3 days", 35)
    } else if days <= 7 {
        ("within 7 days", 20)
    } else {
        ("later", 5)
    };
    PrioritySignal {
        code: "deadline".into(),
        label: format!("Due {due} · {bucket}"),
        points,
        state: format!("{due}:{bucket}"),
    }
}
async fn latest(tx: &mut Transaction<'_, Sqlite>, project: &str) -> Result<Option<PriorityReview>> {
    let record: Option<String> = sqlx::query_scalar(
        "SELECT record FROM priority_reviews WHERE project_id=? ORDER BY sequence DESC LIMIT 1",
    )
    .bind(project)
    .fetch_optional(&mut **tx)
    .await?;
    record.map(|r| Ok(serde_json::from_str(&r)?)).transpose()
}
async fn build(
    tx: &mut Transaction<'_, Sqlite>,
    project: &str,
    access: &Access,
) -> Result<PriorityBoard> {
    let scope = project_access(tx, project, access).await?;
    let docs = documents(tx, &scope).await?;
    let all_tasks = tasks(tx, &docs).await?;
    let prefs = preferences(tx, project).await?;
    let day = today();
    let waiting = super::waiting::for_sources(tx, project, &scope, day).await?;
    let baseline = latest(tx, project).await?;
    let mut ranked = vec![];
    for task in all_tasks.iter().filter(|t| t.status == "accepted") {
        let preference = prefs
            .iter()
            .find(|p| p.task_id == task.id)
            .cloned()
            .unwrap_or_else(|| default_preference(project, &task.id));
        let points = match preference.importance.as_str() {
            "low" => 0,
            "high" => 25,
            "critical" => 40,
            _ => 10,
        };
        let linked = waiting
            .iter()
            .filter(|w| w.task_id.as_deref() == Some(task.id.as_str()))
            .collect::<Vec<_>>();
        let due = linked.iter().any(|w| w.follow_up_due);
        let earliest = linked.iter().map(|w| w.follow_up_on.as_str()).min();
        let (waiting_points, waiting_label) = if linked.is_empty() {
            (0, "No active waiting-for request".into())
        } else if due {
            (
                20,
                format!("Follow-up due · {} open requests", linked.len()),
            )
        } else {
            (
                -20,
                format!(
                    "Awaiting response · next follow-up {}",
                    earliest.unwrap_or_default()
                ),
            )
        };
        let current = docs.iter().find(|d| d.id == task.evidence.document_id);
        let stale = current.is_none_or(|d| d.revision != task.evidence.revision);
        let signals = vec![
            urgency(task, day),
            PrioritySignal {
                code: "importance".into(),
                label: format!(
                    "{} importance{}",
                    preference.importance,
                    if preference.version == 0 {
                        " · default"
                    } else {
                        " · your preference"
                    }
                ),
                points,
                state: format!("{}:{}", preference.importance, hash(&preference.note)),
            },
            PrioritySignal {
                code: "waiting".into(),
                label: waiting_label,
                points: waiting_points,
                state: format!(
                    "{due}:{}",
                    hash(&serde_json::to_string(
                        &linked
                            .iter()
                            .map(|w| (&w.id, w.version, &w.follow_up_on))
                            .collect::<Vec<_>>()
                    )?)
                ),
            },
            PrioritySignal {
                code: "evidence".into(),
                label: if stale {
                    format!(
                        "Evidence needs review · cited r{}, current {}",
                        task.evidence.revision,
                        current
                            .map(|d| format!("r{}", d.revision))
                            .unwrap_or_else(|| "unavailable".into())
                    )
                } else {
                    format!("Evidence current · r{}", task.evidence.revision)
                },
                points: 0,
                state: current
                    .map(|d| format!("{}:{}", d.revision, d.content_hash))
                    .unwrap_or_else(|| "unavailable".into()),
            },
        ];
        let score = signals.iter().map(|s| s.points).sum();
        ranked.push(PriorityItem {
            current: PrioritySavedItem {
                task: task_ref(task),
                rank: 0,
                score,
                preference,
                signals,
            },
            previous_rank: None,
            previous_score: None,
            changes: vec![],
            influences: vec![],
            influences_omitted: 0,
            new_since_review: false,
            stale_evidence: stale,
            waiting_ids: linked.iter().map(|w| w.id.clone()).collect(),
        });
    }
    if ranked.len() > 500 {
        bail!(
            "invalid: priority review supports at most 500 accepted tasks per project; create a narrower project scope"
        );
    }
    // Sort dated ties by earliest date, with missing/invalid dates after explicit dates.
    ranked.sort_by(|a, b| {
        b.current
            .score
            .cmp(&a.current.score)
            .then_with(|| {
                let due = |t: &PriorityTask| t.due.as_deref().and_then(|d| date(d).ok());
                match (due(&a.current.task), due(&b.current.task)) {
                    (Some(a), Some(b)) => a.cmp(&b),
                    (Some(_), None) => std::cmp::Ordering::Less,
                    (None, Some(_)) => std::cmp::Ordering::Greater,
                    _ => std::cmp::Ordering::Equal,
                }
            })
            .then_with(|| a.current.task.id.cmp(&b.current.task.id))
    });
    for (i, item) in ranked.iter_mut().enumerate() {
        item.current.rank = (i + 1) as i64;
    }
    let mut departed = vec![];
    if let Some(old) = &baseline {
        for previous in &old.items {
            if !ranked.iter().any(|i| i.current.task.id == previous.task.id) {
                let status = all_tasks
                    .iter()
                    .find(|t| t.id == previous.task.id)
                    .map(|t| t.status.clone())
                    .unwrap_or_else(|| "outside current project".into());
                departed.push(PriorityDeparture {
                    task: previous.task.clone(),
                    previous_rank: previous.rank,
                    status,
                });
            }
        }
        let all_current = ranked.iter().map(|i| i.current.clone()).collect::<Vec<_>>();
        for item in &mut ranked {
            let previous = old.items.iter().find(|p| p.task.id == item.current.task.id);
            item.new_since_review = previous.is_none();
            if let Some(previous) = previous {
                item.previous_rank = Some(previous.rank);
                item.previous_score = Some(previous.score);
                for signal in &item.current.signals {
                    if let Some(before) = previous.signals.iter().find(|s| s.code == signal.code) {
                        if before.state != signal.state || before.points != signal.points {
                            item.changes.push(PrioritySignalChange {
                                code: signal.code.clone(),
                                before: before.label.clone(),
                                after: if before.label == signal.label {
                                    format!(
                                        "{} · underlying {} record changed",
                                        signal.label, signal.code
                                    )
                                } else {
                                    signal.label.clone()
                                },
                                points_delta: signal.points - before.points,
                            });
                        }
                    }
                }
                if item.current.rank != previous.rank {
                    for other in &all_current {
                        if other.task.id == item.current.task.id {
                            continue;
                        }
                        let old_other = old.items.iter().find(|p| p.task.id == other.task.id);
                        let detail = match old_other {
                            None if other.rank < item.current.rank => {
                                Some("New accepted work now ranks ahead")
                            }
                            Some(o) if o.rank > previous.rank && other.rank < item.current.rank => {
                                Some("Moved ahead relative to this task")
                            }
                            Some(o) if o.rank < previous.rank && other.rank > item.current.rank => {
                                Some("Moved behind relative to this task")
                            }
                            _ => None,
                        };
                        if let Some(detail) = detail {
                            item.influences.push(PriorityInfluence {
                                task_id: other.task.id.clone(),
                                title: other.task.title.clone(),
                                detail: if let Some(previous) = old_other {
                                    let changed = other
                                        .signals
                                        .iter()
                                        .filter(|s| {
                                            previous.signals.iter().any(|old| {
                                                old.code == s.code
                                                    && (old.state != s.state
                                                        || old.points != s.points)
                                            })
                                        })
                                        .map(|s| s.label.clone())
                                        .take(2)
                                        .collect::<Vec<_>>()
                                        .join("; ");
                                    format!(
                                        "{detail} · score {} → {}{}",
                                        previous.score,
                                        other.score,
                                        if changed.is_empty() {
                                            String::new()
                                        } else {
                                            format!(" · {changed}")
                                        }
                                    )
                                } else {
                                    format!("{detail} · score {}", other.score)
                                },
                                evidence: other.task.evidence.clone(),
                            });
                        }
                    }
                    for left in &departed {
                        if left.previous_rank < previous.rank {
                            item.influences.push(PriorityInfluence {
                                task_id: left.task.id.clone(),
                                title: left.task.title.clone(),
                                detail: format!("Previously ahead; now {}", left.status),
                                evidence: left.task.evidence.clone(),
                            });
                        }
                    }
                    item.influences_omitted = item.influences.len().saturating_sub(5);
                    item.influences.truncate(5);
                }
            }
        }
    }
    let sources = watermarks(&docs);
    let watermark = sqlx::query_scalar("SELECT COALESCE(MAX(sequence),0) FROM events")
        .fetch_one(&mut **tx)
        .await?;
    let signature = hash(&serde_json::to_string(
        &json!({"algorithm":ALGORITHM,"date":day.to_string(),"sources":sources,"tasks":all_tasks.iter().map(|t|(&t.id,t.version,&t.status)).collect::<Vec<_>>(),"preferences":prefs,"waiting":waiting.iter().map(|w|(&w.id,w.version,w.follow_up_due)).collect::<Vec<_>>(),"baseline":baseline.as_ref().map(|r|&r.id)}),
    )?);
    let baseline = baseline.map(|r| PriorityReviewSummary {
        id: r.id,
        as_of_date: r.as_of_date,
        created_at: r.created_at,
        task_count: r.items.len(),
    });
    let total_items = ranked.len();
    Ok(PriorityBoard {
        project_id: project.into(),
        as_of_date: day.to_string(),
        created_at: now(),
        algorithm_version: ALGORITHM,
        watermark,
        signature,
        baseline,
        sources,
        items: ranked,
        departed,
        total_items,
        offset: 0,
        limit: total_items,
        items_omitted: 0,
        departed_omitted: 0,
    })
}
impl Store {
    pub async fn priorities(&self, project: &str, access: &Access) -> Result<PriorityBoard> {
        let mut tx = self.pool.begin().await?;
        let board = build(&mut tx, project, access).await?;
        tx.commit().await?;
        Ok(board)
    }
    pub async fn set_priority_preference(
        &self,
        project: &str,
        task_id: &str,
        req: PriorityPreferenceRequest,
    ) -> Result<PriorityPreference> {
        if !["low", "normal", "high", "critical"].contains(&req.importance.as_str()) {
            bail!("invalid importance preference");
        }
        optional_text(&req.note, 1000, "priority note")?;
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let (digest, old) = operation(
            &mut tx,
            "priority-preference",
            &format!("{project}/{task_id}"),
            &req.idempotency_key,
            &req,
        )
        .await?;
        let mut preference = preferences(&mut tx, project)
            .await?
            .into_iter()
            .find(|p| p.task_id == task_id)
            .unwrap_or_else(|| default_preference(project, task_id));
        if old.is_none() {
            let scope = project_access(&mut tx, project, &Access::admin()).await?;
            let docs = documents(&mut tx, &scope).await?;
            let all = tasks(&mut tx, &docs).await?;
            if !all
                .iter()
                .any(|t| t.id == task_id && t.status == "accepted")
            {
                bail!("invalid: choose an accepted project task");
            }
            if preference.version != req.expected_version {
                bail!("conflict: preference changed; refresh before editing");
            }
            preference.importance = req.importance;
            preference.note = req.note;
            preference.version += 1;
            preference.updated_at = Some(now());
            sqlx::query("INSERT INTO priority_preferences(project_id,task_id,record) VALUES(?,?,?) ON CONFLICT(project_id,task_id) DO UPDATE SET record=excluded.record").bind(project).bind(task_id).bind(serde_json::to_string(&preference)?).execute(&mut *tx).await?;
            remember(&mut tx, &req.idempotency_key, &digest, task_id).await?;
            event(&mut tx, "priority", project).await?;
        }
        tx.commit().await?;
        Ok(preference)
    }
    pub async fn review_priorities(
        &self,
        project: &str,
        req: PriorityReviewRequest,
    ) -> Result<PriorityReview> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let (digest, old) = operation(
            &mut tx,
            "priority-review",
            project,
            &req.idempotency_key,
            &req,
        )
        .await?;
        let id = if let Some(id) = old {
            id
        } else {
            let board = build(&mut tx, project, &Access::admin()).await?;
            if board.signature != req.signature
                || board.baseline.as_ref().map(|r| &r.id) != req.expected_review_id.as_ref()
            {
                bail!("conflict: priorities changed; refresh before saving this review");
            }
            let review = PriorityReview {
                id: uuid::Uuid::new_v4().to_string(),
                project_id: project.into(),
                as_of_date: board.as_of_date,
                created_at: now(),
                algorithm_version: ALGORITHM,
                watermark: board.watermark,
                sources: board.sources,
                items: board.items.into_iter().map(|i| i.current).collect(),
            };
            sqlx::query("INSERT INTO priority_reviews(id,project_id,record) VALUES(?,?,?)")
                .bind(&review.id)
                .bind(project)
                .bind(serde_json::to_string(&review)?)
                .execute(&mut *tx)
                .await?;
            remember(&mut tx, &req.idempotency_key, &digest, &review.id).await?;
            event(&mut tx, "priority-review", project).await?;
            review.id
        };
        let record: String =
            sqlx::query_scalar("SELECT record FROM priority_reviews WHERE id=? AND project_id=?")
                .bind(id)
                .bind(project)
                .fetch_one(&mut *tx)
                .await?;
        tx.commit().await?;
        Ok(serde_json::from_str(&record)?)
    }
    pub async fn priority_reviews(&self, project: &str) -> Result<Vec<PriorityReviewSummary>> {
        let rows=sqlx::query("SELECT record FROM priority_reviews WHERE project_id=? ORDER BY sequence DESC LIMIT 30").bind(project).fetch_all(&self.pool).await?;
        rows.iter()
            .map(|r| {
                let r: PriorityReview = serde_json::from_str(r.get("record"))?;
                Ok(PriorityReviewSummary {
                    id: r.id,
                    as_of_date: r.as_of_date,
                    created_at: r.created_at,
                    task_count: r.items.len(),
                })
            })
            .collect()
    }
    pub async fn priority_review(&self, project: &str, id: &str) -> Result<PriorityReview> {
        let record: String =
            sqlx::query_scalar("SELECT record FROM priority_reviews WHERE id=? AND project_id=?")
                .bind(id)
                .bind(project)
                .fetch_optional(&self.pool)
                .await?
                .context("record not found")?;
        Ok(serde_json::from_str(&record)?)
    }
}
