use super::*;

pub(super) async fn allowed(
    tx: &mut Transaction<'_, Sqlite>,
    item: &Commitment,
    access: &Access,
) -> Result<bool> {
    if access.source_ids.is_none() {
        return Ok(true);
    }
    for link in &item.links {
        if let Some(e) = &link.repository {
            let record: Option<String> =
                sqlx::query_scalar("SELECT record FROM repo_connections WHERE id=?")
                    .bind(&e.repository_id)
                    .fetch_optional(&mut **tx)
                    .await?;
            let Some(record) = record else {
                return Ok(false);
            };
            let repo: RepositoryConnection = serde_json::from_str(&record)?;
            if !repo.allow_external || repo.project_id != item.project_id {
                return Ok(false);
            }
        }
    }
    Ok(true)
}
impl Store {
    /// Attach an immutable report/commit snapshot. This performs no Git, network or filesystem operation.
    pub async fn link_commitment_repository(
        &self,
        id: &str,
        mut req: DeliveryRepositoryRequest,
    ) -> Result<Commitment> {
        validate_text(&req.note, 2000, "repository link explanation")?;
        req.paths.sort();
        req.paths.dedup();
        if req.paths.len() > 20 {
            bail!("invalid: choose at most 20 report paths");
        }
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let (digest, old) = operation(
            &mut tx,
            "commitment-repository",
            id,
            &req.idempotency_key,
            &req,
        )
        .await?;
        if old.is_none() {
            let mut item = raw(&mut tx, id).await?;
            if item.version != req.expected_version {
                bail!("conflict: commitment changed; refresh");
            }
            if ["completed", "cancelled"].contains(&item.status.as_str()) {
                bail!("conflict: reopen commitment before attaching evidence");
            }
            if item.links.len() >= 100 {
                bail!("invalid: commitment already has 100 evidence links");
            }
            let report_json: String =
                sqlx::query_scalar("SELECT record FROM repo_reports WHERE id=?")
                    .bind(&req.report_id)
                    .fetch_optional(&mut *tx)
                    .await?
                    .context("record not found")?;
            let report: RepositoryReport = serde_json::from_str(&report_json)?;
            let scope = project_sources(&mut tx, &item.project_id).await?;
            if report.project_id != item.project_id
                || !report.source_ids.iter().all(|s| scope.contains(s))
            {
                bail!("invalid: choose a report from this project's current source scope");
            }
            permit(&mut tx, &report.source_ids, &Access::admin()).await?;
            let repo_json: String =
                sqlx::query_scalar("SELECT record FROM repo_connections WHERE id=?")
                    .bind(&report.repository_id)
                    .fetch_optional(&mut *tx)
                    .await?
                    .context("record not found")?;
            let repo: RepositoryConnection = serde_json::from_str(&repo_json)?;
            if repo.project_id != item.project_id {
                bail!("invalid: repository belongs to another project");
            }
            let commit = report
                .commits
                .iter()
                .find(|c| c.id == req.commit_id)
                .context("invalid: select a full commit ID present in the pinned report")?;
            let paths = if req.paths.is_empty() {
                commit.files.iter().take(20).cloned().collect::<Vec<_>>()
            } else {
                req.paths.clone()
            };
            if paths.is_empty() || !paths.iter().all(|p| commit.files.contains(p)) {
                bail!("invalid: choose file paths present in the selected report commit");
            }
            let evidence = DeliveryRepositoryEvidence {
                repository_id: report.repository_id,
                report_id: report.id,
                report_hash: crate::store::hash(&report_json),
                report_created_at: report.created_at,
                from_commit: report.from_commit,
                to_commit: report.to_commit,
                commit_id: commit.id.clone(),
                commit_subject: commit.subject.clone(),
                paths_omitted: commit.files.len().saturating_sub(paths.len()),
                paths,
                report_omissions: report.omissions,
            };
            let link_id = uuid::Uuid::new_v4().to_string();
            item.links.push(DeliveryLink {
                id: link_id.clone(),
                kind: "implementation".into(),
                note: req.note.clone(),
                evidence: vec![],
                repository: Some(evidence),
                contributor: "You".into(),
                created_at: now(),
            });
            item.source_ids.extend(report.source_ids);
            item.source_ids.sort();
            item.source_ids.dedup();
            item.completion_proposal = None;
            item.status = "open".into();
            item.version += 1;
            item.updated_at = now();
            item.history_count += 1;
            save(
                &mut tx,
                &item,
                "repository_link",
                &req.note,
                &[],
                &[link_id],
            )
            .await?;
            remember(&mut tx, &req.idempotency_key, &digest, id).await?;
        }
        tx.commit().await?;
        self.commitment(id, &Access::admin()).await
    }
}
