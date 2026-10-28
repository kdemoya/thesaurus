//! Reuse recommendations are based on explicit successful-outcome reviews, never runtime completion alone.
use crate::{
    Store,
    continuity::{event, operation, permit, project_sources, remember},
    store::{hash, now, validate_text},
};
use anyhow::{Context, Result, bail};
use serde::Serialize;
use sqlx::{Row, Sqlite, Transaction};
use std::collections::BTreeMap;
use thesaurus_contracts::*;
fn page(q: &ProcedureQuery) -> Result<(usize, usize)> {
    let o = q.offset.unwrap_or(0) as usize;
    let l = q.limit.unwrap_or(25) as usize;
    if o > 100000 || !(1..=50).contains(&l) {
        bail!("invalid procedure page");
    }
    Ok((o, l))
}
async fn run(tx: &mut Transaction<'_, Sqlite>, id: &str) -> Result<RuntimeRun> {
    let s: String = sqlx::query_scalar("SELECT record FROM runtime_runs WHERE id=?")
        .bind(id)
        .fetch_optional(&mut **tx)
        .await?
        .context("record not found")?;
    Ok(serde_json::from_str(&s)?)
}
async fn raw(tx: &mut Transaction<'_, Sqlite>, id: &str) -> Result<ProcedureSuggestion> {
    let s: String = sqlx::query_scalar("SELECT record FROM procedure_suggestions WHERE id=?")
        .bind(id)
        .fetch_optional(&mut **tx)
        .await?
        .context("record not found")?;
    Ok(serde_json::from_str(&s)?)
}
async fn history(
    tx: &mut Transaction<'_, Sqlite>,
    kind: &str,
    id: &str,
    action: &str,
    note: &str,
    item: &impl Serialize,
) -> Result<()> {
    let e = ProcedureEvent {
        sequence: 0,
        action: action.into(),
        contributor: if ["suggested", "support_changed"].contains(&action) {
            "Local procedure rule"
        } else {
            "You"
        }
        .into(),
        created_at: now(),
        note: note.into(),
        snapshot: serde_json::to_value(item)?,
    };
    sqlx::query("INSERT INTO procedure_history(kind,record_id,record) VALUES(?,?,?)")
        .bind(kind)
        .bind(id)
        .bind(serde_json::to_string(&e)?)
        .execute(&mut **tx)
        .await?;
    event(tx, "procedure", id).await
}
async fn save(
    tx: &mut Transaction<'_, Sqlite>,
    s: &ProcedureSuggestion,
    action: &str,
    note: &str,
) -> Result<()> {
    sqlx::query("INSERT INTO procedure_suggestions(id,project_id,record) VALUES(?,?,?) ON CONFLICT(id) DO UPDATE SET record=excluded.record").bind(&s.id).bind(&s.project_id).bind(serde_json::to_string(s)?).execute(&mut **tx).await?;
    history(tx, "suggestion", &s.id, action, note, s).await
}
fn template(prompt: &str) -> String {
    let prompt = prompt
        .split("\n\nPinned evidence:\n")
        .next()
        .unwrap_or(prompt);
    let prompt = prompt
        .split_once("\n\nUser analysis:\n")
        .map(|(_, p)| p)
        .unwrap_or(prompt);
    let mut text = prompt.to_string();
    while text.len() > 6000 {
        text.pop();
    }
    text
}
async fn refresh(tx: &mut Transaction<'_, Sqlite>, project: &str) -> Result<ProcedureRefresh> {
    let scope = project_sources(tx, project).await?;
    let rows =
        sqlx::query("SELECT record FROM procedure_run_reviews WHERE project_id=? ORDER BY run_id")
            .bind(project)
            .fetch_all(&mut **tx)
            .await?;
    let mut groups = BTreeMap::<String, Vec<(ProcedureRunReview, RuntimeRun)>>::new();
    for row in rows {
        let review: ProcedureRunReview = serde_json::from_str(row.get("record"))?;
        if !review.successful {
            continue;
        }
        let r = run(tx, &review.run_id).await?;
        if r.status != "completed" || !r.source_ids.iter().all(|id| scope.contains(id)) {
            continue;
        }
        groups
            .entry(review.procedure.trim().to_lowercase())
            .or_default()
            .push((review, r));
    }
    // Existing suggestions are revisited too, so withdrawing success cannot leave stale support marked sufficient.
    let existing = sqlx::query("SELECT record FROM procedure_suggestions WHERE project_id=?")
        .bind(project)
        .fetch_all(&mut **tx)
        .await?;
    let mut old_by_name = BTreeMap::new();
    for row in existing {
        let s: ProcedureSuggestion = serde_json::from_str(row.get("record"))?;
        groups.entry(s.name.trim().to_lowercase()).or_default();
        old_by_name.insert(s.name.trim().to_lowercase(), s);
    }
    let mut result = ProcedureRefresh {
        project_id: project.into(),
        created: 0,
        updated: 0,
    };
    for (key, group) in groups {
        let old = old_by_name.remove(&key);
        if group.len() < 2 && old.is_none() {
            continue;
        }
        let first = group.first();
        let mut sources = group
            .iter()
            .flat_map(|(_, r)| r.source_ids.clone())
            .collect::<Vec<_>>();
        sources.sort();
        sources.dedup();
        let all_count = group.len();
        let supporting = group
            .iter()
            .take(20)
            .map(|(review, r)| ProcedureSupportingRun {
                run_id: r.id.clone(),
                connection: r.connection.clone(),
                source_ids: r.source_ids.clone(),
                created_at: r.created_at.clone(),
                outcome_review: review.note.clone(),
                review_version: review.version,
                output_excerpt: r.output.chars().take(1000).collect(),
                output_truncated: r.output.chars().count() > 1000,
            })
            .collect::<Vec<_>>();
        let mut s = if let Some(old) = old {
            old
        } else {
            let (rev, r) = first.context("missing supporting run")?;
            ProcedureSuggestion{id:format!("procedure-{}",&hash(&format!("{project}:{key}"))[..32]),project_id:project.into(),name:rev.procedure.clone(),template:template(&r.prompt),source_ids:vec![],connection:r.connection.clone(),supporting_runs:vec![],supporting_runs_omitted:0,support_sufficient:false,status:"proposed".into(),version:0,recipe_id:None,created_at:now(),updated_at:now(),explanation:"At least two distinct completed runs were explicitly reviewed by you as successful for this named procedure. Review the proposed template and supporting outputs; this is not proof of general reliability.".into()}
        };
        // Historical source scope never shrinks when support reviews are withdrawn.
        sources.extend(s.source_ids.clone());
        sources.sort();
        sources.dedup();
        let unchanged = s.support_sufficient == (all_count >= 2)
            && serde_json::to_value(&s.supporting_runs)? == serde_json::to_value(&supporting)?
            && s.source_ids == sources
            && s.supporting_runs_omitted == all_count.saturating_sub(supporting.len());
        if unchanged {
            continue;
        }
        let fresh = s.version == 0;
        s.supporting_runs_omitted = all_count.saturating_sub(supporting.len());
        s.supporting_runs = supporting;
        s.support_sufficient = all_count >= 2;
        s.source_ids = sources;
        s.version += 1;
        s.updated_at = now();
        save(tx,&s,if fresh{"suggested"}else{"support_changed"},"Supporting successful-outcome reviews refreshed; existing dismissal/installation retained").await?;
        if fresh {
            result.created += 1;
        } else {
            result.updated += 1;
        }
    }
    Ok(result)
}
impl Store {
    pub async fn migrate_procedures(&self) -> Result<()> {
        sqlx::raw_sql(include_str!("schema.sql"))
            .execute(&self.pool)
            .await?;
        Ok(())
    }
    pub async fn review_procedure_run(
        &self,
        req: ProcedureReviewRequest,
    ) -> Result<ProcedureRunReview> {
        validate_text(&req.procedure, 200, "procedure name")?;
        validate_text(&req.note, 2000, "run outcome explanation")?;
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let (digest, old) = operation(
            &mut tx,
            "procedure-run-review",
            &format!("{}:{}", req.project_id, req.run_id),
            &req.idempotency_key,
            &req,
        )
        .await?;
        if old.is_none() {
            let scope = project_sources(&mut tx, &req.project_id).await?;
            let r = run(&mut tx, &req.run_id).await?;
            if r.source_ids.is_empty() || !r.source_ids.iter().all(|s| scope.contains(s)) {
                bail!("invalid: select a run whose sources are within this project");
            }
            if req.successful && r.status != "completed" {
                bail!("invalid: only a completed run can be reviewed as successful");
            }
            let old: Option<String> = sqlx::query_scalar(
                "SELECT record FROM procedure_run_reviews WHERE project_id=? AND run_id=?",
            )
            .bind(&req.project_id)
            .bind(&req.run_id)
            .fetch_optional(&mut *tx)
            .await?;
            let previous = old
                .map(|s| serde_json::from_str::<ProcedureRunReview>(&s))
                .transpose()?;
            if previous.as_ref().map_or(0, |r| r.version) != req.expected_version {
                bail!("conflict: run outcome review changed; refresh");
            }
            let review = ProcedureRunReview {
                project_id: req.project_id.clone(),
                run_id: req.run_id.clone(),
                procedure: req.procedure.trim().into(),
                successful: req.successful,
                note: req.note.clone(),
                version: req.expected_version + 1,
                contributor: "You".into(),
                created_at: previous.map(|r| r.created_at).unwrap_or_else(now),
                updated_at: now(),
            };
            sqlx::query("INSERT INTO procedure_run_reviews(project_id,run_id,record) VALUES(?,?,?) ON CONFLICT(project_id,run_id) DO UPDATE SET record=excluded.record").bind(&req.project_id).bind(&req.run_id).bind(serde_json::to_string(&review)?).execute(&mut *tx).await?;
            history(
                &mut tx,
                "run",
                &format!("{}:{}", req.project_id, req.run_id),
                "reviewed",
                &req.note,
                &review,
            )
            .await?;
            refresh(&mut tx, &req.project_id).await?;
            remember(&mut tx, &req.idempotency_key, &digest, &req.run_id).await?;
        }
        let s: String = sqlx::query_scalar(
            "SELECT record FROM procedure_run_reviews WHERE project_id=? AND run_id=?",
        )
        .bind(&req.project_id)
        .bind(&req.run_id)
        .fetch_one(&mut *tx)
        .await?;
        let result = serde_json::from_str(&s)?;
        tx.commit().await?;
        Ok(result)
    }
    pub async fn procedure_suggestion(
        &self,
        id: &str,
        access: &Access,
    ) -> Result<ProcedureSuggestion> {
        let mut tx = self.pool.begin().await?;
        let s = raw(&mut tx, id).await?;
        permit(&mut tx, &s.source_ids, access).await?;
        tx.commit().await?;
        Ok(s)
    }
    pub async fn procedure_suggestions(
        &self,
        project: &str,
        q: ProcedureQuery,
        access: &Access,
    ) -> Result<ProcedurePage> {
        let (offset, limit) = page(&q)?;
        let mut tx = self.pool.begin().await?;
        let scope = project_sources(&mut tx, project).await?;
        permit(&mut tx, &scope, access).await?;
        let mut items = vec![];
        let mut total = 0;
        for row in sqlx::query("SELECT record FROM procedure_suggestions WHERE project_id=? ORDER BY json_extract(record,'$.updated_at') DESC,id").bind(project).fetch_all(&mut *tx).await?{let s:ProcedureSuggestion=serde_json::from_str(row.get("record"))?;if s.status=="dismissed"&&!q.include_dismissed{continue;}permit(&mut tx,&s.source_ids,access).await?;if total>=offset&&items.len()<limit{items.push(s);}total+=1;}
        while serde_json::to_vec(&items)?.len() > 300000 {
            if items.pop().is_none() {
                break;
            }
        }
        let next_offset = (offset + items.len() < total).then_some(offset + items.len());
        tx.commit().await?;
        Ok(ProcedurePage {
            project_id: project.into(),
            items,
            total,
            offset,
            next_offset,
        })
    }
    pub async fn refresh_procedures(&self, project: &str) -> Result<ProcedureRefresh> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let r = refresh(&mut tx, project).await?;
        tx.commit().await?;
        Ok(r)
    }
    pub async fn decide_procedure(
        &self,
        id: &str,
        req: ProcedureDecisionRequest,
    ) -> Result<ProcedureSuggestion> {
        validate_text(&req.note, 2000, "procedure decision reason")?;
        if !["dismiss", "reopen"].contains(&req.action.as_str()) {
            bail!("invalid procedure decision");
        }
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let (digest, old) = operation(
            &mut tx,
            "procedure-decision",
            id,
            &req.idempotency_key,
            &req,
        )
        .await?;
        if old.is_none() {
            let mut s = raw(&mut tx, id).await?;
            if s.version != req.expected_version {
                bail!("conflict: suggestion changed; refresh");
            }
            if s.status == "installed" {
                bail!("conflict: manage the installed recipe through Recipes");
            }
            s.status = if req.action == "dismiss" {
                "dismissed"
            } else {
                "proposed"
            }
            .into();
            s.version += 1;
            s.updated_at = now();
            save(&mut tx, &s, &req.action, &req.note).await?;
            remember(&mut tx, &req.idempotency_key, &digest, id).await?;
        }
        tx.commit().await?;
        self.procedure_suggestion(id, &Access::admin()).await
    }
    pub async fn procedure_reviews(
        &self,
        project: &str,
        q: ProcedureQuery,
    ) -> Result<ProcedureReviewPage> {
        let (offset, limit) = page(&q)?;
        let mut tx = self.pool.begin().await?;
        project_sources(&mut tx, project).await?;
        let total: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM procedure_run_reviews WHERE project_id=?")
                .bind(project)
                .fetch_one(&mut *tx)
                .await?;
        let rows=sqlx::query("SELECT record FROM procedure_run_reviews WHERE project_id=? ORDER BY json_extract(record,'$.updated_at') DESC,run_id LIMIT ? OFFSET ?").bind(project).bind(limit as i64).bind(offset as i64).fetch_all(&mut *tx).await?;
        let items = rows
            .into_iter()
            .map(|r| serde_json::from_str::<ProcedureRunReview>(r.get("record")))
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let next_offset = (offset + items.len() < total as usize).then_some(offset + items.len());
        tx.commit().await?;
        Ok(ProcedureReviewPage {
            items,
            total: total as usize,
            offset,
            next_offset,
        })
    }
    pub async fn procedure_history(
        &self,
        id: &str,
        q: ProcedureQuery,
        access: &Access,
    ) -> Result<ProcedureHistoryPage> {
        let (offset, limit) = page(&q)?;
        let mut tx = self.pool.begin().await?;
        let s = raw(&mut tx, id).await?;
        permit(&mut tx, &s.source_ids, access).await?;
        let total: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM procedure_history WHERE kind='suggestion' AND record_id=?",
        )
        .bind(id)
        .fetch_one(&mut *tx)
        .await?;
        let rows=sqlx::query("SELECT sequence,record FROM procedure_history WHERE kind='suggestion' AND record_id=? ORDER BY sequence DESC LIMIT ? OFFSET ?").bind(id).bind(limit as i64).bind(offset as i64).fetch_all(&mut *tx).await?;
        let mut items = vec![];
        for row in rows {
            let mut e: ProcedureEvent = serde_json::from_str(row.get("record"))?;
            e.sequence = row.get("sequence");
            items.push(e);
            if serde_json::to_vec(&items)?.len() > 300000 {
                if items.len() > 1 {
                    items.pop();
                }
                break;
            }
        }
        let next_offset = (offset + items.len() < total as usize).then_some(offset + items.len());
        tx.commit().await?;
        Ok(ProcedureHistoryPage {
            items,
            total: total as usize,
            offset,
            next_offset,
        })
    }
}
impl Store {
    /// Install only the concrete recipe reviewed in this request; this never executes it.
    pub async fn install_procedure(
        &self,
        id: &str,
        mut req: ProcedureInstallRequest,
    ) -> Result<ProcedureInstallation> {
        validate_text(&req.name, 200, "recipe name")?;
        validate_text(&req.template, 6000, "recipe template")?;
        validate_text(&req.connection, 100, "recipe connection")?;
        validate_text(&req.note, 2000, "installation review note")?;
        if !["markdown", "json"].contains(&req.output_format.as_str())
            || !(1000..=14000).contains(&req.character_budget)
            || !(1..=100).contains(&req.max_runs_per_day)
            || req.inputs.len() > 12
        {
            bail!("invalid recipe format or budget");
        }
        req.inputs.sort();
        req.inputs.dedup();
        req.source_ids.sort();
        req.source_ids.dedup();
        if req.source_ids.is_empty() || req.source_ids.len() > 50 {
            bail!("invalid recipe source scope");
        }
        for input in &req.inputs {
            if input.is_empty()
                || input.len() > 50
                || !input.chars().enumerate().all(|(i, c)| {
                    if i == 0 {
                        c.is_ascii_alphabetic()
                    } else {
                        c.is_ascii_alphanumeric() || c == '_'
                    }
                })
            {
                bail!("invalid recipe input name");
            }
        }
        let regex = regex::Regex::new(r"\{\{([A-Za-z][A-Za-z0-9_]*)\}\}")?;
        let mut variables = regex
            .captures_iter(&req.template)
            .map(|c| c[1].to_string())
            .collect::<Vec<_>>();
        variables.sort();
        variables.dedup();
        if variables != req.inputs {
            bail!("invalid: declare exactly the template placeholders as inputs");
        }
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let (digest, old) =
            operation(&mut tx, "procedure-install", id, &req.idempotency_key, &req).await?;
        let recipe_id = if let Some(id) = old {
            id
        } else {
            let initial = raw(&mut tx, id).await?;
            refresh(&mut tx, &initial.project_id).await?;
            let mut s = raw(&mut tx, id).await?;
            if s.version != req.expected_version {
                bail!("conflict: procedure support changed; review again");
            }
            if s.status != "proposed" || !s.support_sufficient {
                bail!(
                    "conflict: install requires a proposed procedure with two successful reviewed runs"
                );
            }
            let scope = project_sources(&mut tx, &s.project_id).await?;
            if !req.source_ids.iter().all(|source| scope.contains(source))
                || !s
                    .source_ids
                    .iter()
                    .all(|source| req.source_ids.contains(source))
            {
                bail!(
                    "invalid: installed scope must cover the suggestion's historical sources within the current project"
                );
            }
            permit(&mut tx, &req.source_ids, &Access::admin()).await?;
            let count: i64 =
                sqlx::query_scalar("SELECT COUNT(*) FROM work_recipes WHERE project_id=?")
                    .bind(&s.project_id)
                    .fetch_one(&mut *tx)
                    .await?;
            if count >= 200 {
                bail!("invalid: project recipe limit reached");
            }
            let recipe = WorkRecipe {
                id: uuid::Uuid::new_v4().to_string(),
                project_id: s.project_id.clone(),
                version: 1,
                name: req.name.clone(),
                template: req.template.clone(),
                inputs: req.inputs.clone(),
                source_ids: req.source_ids.clone(),
                connection: req.connection.clone(),
                output_format: req.output_format.clone(),
                character_budget: req.character_budget,
                max_runs_per_day: req.max_runs_per_day,
                archived: false,
                created_at: now(),
                updated_at: now(),
            };
            let json = serde_json::to_string(&recipe)?;
            sqlx::query("INSERT INTO work_recipes(id,project_id,record) VALUES(?,?,?)")
                .bind(&recipe.id)
                .bind(&recipe.project_id)
                .bind(&json)
                .execute(&mut *tx)
                .await?;
            sqlx::query("INSERT INTO recipe_versions(recipe_id,version,record) VALUES(?,1,?)")
                .bind(&recipe.id)
                .bind(&json)
                .execute(&mut *tx)
                .await?;
            s.status = "installed".into();
            s.recipe_id = Some(recipe.id.clone());
            s.version += 1;
            s.updated_at = now();
            save(&mut tx, &s, "installed_recipe", &req.note).await?;
            event(&mut tx, "recipe", &recipe.id).await?;
            remember(&mut tx, &req.idempotency_key, &digest, &recipe.id).await?;
            recipe.id
        };
        let recipe: String = sqlx::query_scalar("SELECT record FROM work_recipes WHERE id=?")
            .bind(recipe_id)
            .fetch_one(&mut *tx)
            .await?;
        let recipe = serde_json::from_str(&recipe)?;
        let suggestion = raw(&mut tx, id).await?;
        tx.commit().await?;
        Ok(ProcedureInstallation { suggestion, recipe })
    }
}
