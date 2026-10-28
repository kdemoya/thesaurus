use super::*;
impl Store {
    pub async fn repository_reports(
        &self,
        id: &str,
        access: &Access,
    ) -> Result<Vec<RepositoryReport>> {
        let r = self.repository(id, access).await?;
        let rows=sqlx::query_scalar::<_,String>("SELECT record FROM repo_reports WHERE repository_id=? ORDER BY created_at DESC LIMIT 30").bind(id).fetch_all(&self.pool).await?;
        let mut tx = self.pool.begin().await?;
        let mut out = vec![];
        for row in rows {
            let report: RepositoryReport = serde_json::from_str(&row)?;
            if report.project_id == r.project_id
                && permit(&mut tx, &report.source_ids, access).await.is_ok()
            {
                out.push(report);
            }
        }
        tx.commit().await?;
        Ok(out)
    }
    pub async fn repository_report(
        &self,
        id: &str,
        req: RepositoryReportRequest,
    ) -> Result<RepositoryReport> {
        let repo = self.repository(id, &Access::admin()).await?;
        let mut tx = self.pool.begin().await?;
        let (digest, old) =
            operation(&mut tx, "repository-report", id, &req.idempotency_key, &req).await?;
        if let Some(old) = old {
            let r: String = sqlx::query_scalar("SELECT record FROM repo_reports WHERE id=?")
                .bind(old)
                .fetch_one(&mut *tx)
                .await?;
            tx.commit().await?;
            return Ok(serde_json::from_str(&r)?);
        }
        let scope = project_sources(&mut tx, &repo.project_id).await?;
        tx.commit().await?;
        if repo.version != req.expected_version {
            bail!("conflict: repository settings/checkpoint changed");
        }
        let to = commit(&repo.path, &req.to_ref).await?;
        let from = commit(&repo.path, &repo.checkpoint).await?;
        let diff = git(
            &repo.path,
            &[
                "diff",
                "--no-ext-diff",
                "--no-textconv",
                "--no-renames",
                "--name-status",
                "-z",
                &from,
                &to,
                "--",
            ],
            500000,
        )
        .await?;
        let parts: Vec<_> = diff.split(|b| *b == 0).filter(|s| !s.is_empty()).collect();
        let mut files = vec![];
        let mut excluded = 0;
        let mut omissions = vec![];
        for pair in parts.chunks(2) {
            if pair.len() != 2 {
                bail!("invalid git diff record");
            }
            let status = String::from_utf8_lossy(pair[0]).into_owned();
            let path = String::from_utf8_lossy(pair[1]).into_owned();
            if allowed(&repo, &path) {
                files.push(RepositoryFile { status, path });
            } else {
                excluded += 1;
            }
        }
        if files.len() > 500 {
            omissions.push(format!(
                "{} selected changed files omitted after first 500",
                files.len() - 500
            ));
            files.truncate(500);
        }
        if excluded > 0 {
            omissions.push(format!(
                "{excluded} changed files excluded by repository scope/sensitive path policy"
            ));
        }
        let range = format!("{from}..{to}");
        let raw = git(
            &repo.path,
            &[
                "log",
                "--max-count=31",
                "--format=%H%x00%s%x00",
                "--no-patch",
                &range,
                "--",
            ],
            100000,
        )
        .await?;
        let pieces: Vec<_> = raw.split(|b| *b == 0).collect();
        let mut commits = vec![];
        let mut seen = 0usize;
        for pair in pieces.chunks(2) {
            if pair.len() != 2 {
                continue;
            }
            let sha = String::from_utf8_lossy(pair[0]).trim().to_string();
            if sha.is_empty() {
                continue;
            }
            if !sha.bytes().all(|b| b.is_ascii_hexdigit()) || sha.len() < 40 {
                bail!("invalid git log record");
            }
            seen += 1;
            if seen > 30 {
                omissions.push(
                    "Commit details limited to the most recent 30 commits in the selected range"
                        .into(),
                );
                break;
            }
            let names = git(
                &repo.path,
                &[
                    "diff-tree",
                    "--root",
                    "--no-commit-id",
                    "--name-only",
                    "-z",
                    "-r",
                    "--no-renames",
                    &sha,
                    "--",
                ],
                500000,
            )
            .await?;
            let selected: Vec<_> = names
                .split(|b| *b == 0)
                .filter_map(|p| std::str::from_utf8(p).ok())
                .filter(|p| allowed(&repo, p))
                .take(501)
                .map(str::to_string)
                .collect();
            if selected.len() > 500 {
                omissions.push(format!("Files for commit {sha} limited to 500"));
            }
            if !selected.is_empty() {
                commits.push(RepositoryCommit {
                    id: sha,
                    subject: String::from_utf8_lossy(pair[1])
                        .chars()
                        .take(1000)
                        .collect(),
                    files: selected.into_iter().take(500).collect(),
                });
            }
        }
        let access = Access {
            contributor: "repository report".into(),
            source_ids: Some(scope.clone()),
            allow_write: false,
            external: false,
        };
        let snap = self.snapshot(&access).await?;
        let mut task_matches = vec![];
        for t in snap.tasks.iter().filter(|t| t.status == "accepted") {
            let words: Vec<_> = t
                .title
                .split(|c: char| !c.is_alphanumeric())
                .filter(|s| s.len() >= 4)
                .map(str::to_lowercase)
                .collect();
            let paths: Vec<_> = files
                .iter()
                .filter(|f| {
                    words.iter().any(|w| f.path.to_lowercase().contains(w))
                        || commits.iter().any(|c| {
                            c.files.contains(&f.path)
                                && words.iter().any(|w| c.subject.to_lowercase().contains(w))
                        })
                })
                .take(20)
                .map(|f| f.path.clone())
                .collect();
            if !paths.is_empty() {
                task_matches.push(RepositoryTaskMatch{task_id:t.id.clone(),title:t.title.clone(),evidence:t.evidence.clone(),matched_paths:paths,reason:"Accepted task title shares a word of at least four characters with selected file paths or commit subjects. This is a relevance hint, not proof of delivery.".into()});
            }
            if task_matches.len() >= 50 {
                omissions.push("Task matches limited to 50 accepted tasks".into());
                break;
            }
        }
        let report = RepositoryReport {
            id: uuid::Uuid::new_v4().to_string(),
            repository_id: id.into(),
            project_id: repo.project_id.clone(),
            source_ids: scope,
            from_commit: from,
            to_commit: to.clone(),
            commits,
            files,
            task_matches,
            omissions,
            created_at: now(),
            checkpoint_advanced: req.advance_checkpoint,
        };
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let (_, old) =
            operation(&mut tx, "repository-report", id, &req.idempotency_key, &req).await?;
        if let Some(old) = old {
            let r: String = sqlx::query_scalar("SELECT record FROM repo_reports WHERE id=?")
                .bind(old)
                .fetch_one(&mut *tx)
                .await?;
            tx.commit().await?;
            return Ok(serde_json::from_str(&r)?);
        }
        let mut current = load(&mut tx, id, &Access::admin()).await?;
        if current.version != req.expected_version
            || project_sources(&mut tx, &repo.project_id).await? != report.source_ids
        {
            bail!("conflict: repository/project changed during report");
        }
        if req.advance_checkpoint {
            current.checkpoint = to;
            current.version += 1;
            current.updated_at = now();
            sqlx::query("UPDATE repo_connections SET version=?,record=? WHERE id=?")
                .bind(current.version)
                .bind(serde_json::to_string(&current)?)
                .bind(id)
                .execute(&mut *tx)
                .await?;
        }
        if serde_json::to_vec(&report)?.len() > 500000 {
            bail!("invalid: report exceeds 500KB; narrow repository selection");
        }
        sqlx::query("INSERT INTO repo_reports(id,repository_id,record,created_at) VALUES(?,?,?,?)")
            .bind(&report.id)
            .bind(id)
            .bind(serde_json::to_string(&report)?)
            .bind(&report.created_at)
            .execute(&mut *tx)
            .await?;
        remember(&mut tx, &req.idempotency_key, &digest, &report.id).await?;
        event(&mut tx, "repository", id).await?;
        tx.commit().await?;
        Ok(report)
    }
}
