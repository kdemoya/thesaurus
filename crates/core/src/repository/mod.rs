use crate::{
    Store,
    continuity::{event, operation, permit, project_sources, remember},
    store::{hash, now, validate_text},
};
use anyhow::{Context, Result, bail};
use sqlx::{Sqlite, Transaction};
use std::{
    path::{Path, PathBuf},
    process::Stdio,
};
use thesaurus_contracts::*;
use tokio::{io::AsyncReadExt, process::Command};
fn prefix_valid(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 500
        && !s.chars().any(|c| c.is_control() || c == '\\')
        && !Path::new(s).is_absolute()
        && Path::new(s)
            .components()
            .all(|c| matches!(c, std::path::Component::Normal(_)))
}
fn allowed(repo: &RepositoryConnection, path: &str) -> bool {
    if !prefix_valid(path) {
        return false;
    }
    if Path::new(path).components().any(|c| {
        let s = c.as_os_str().to_string_lossy();
        s == ".git"
            || s == "node_modules"
            || s == "target"
            || s.starts_with(".env")
            || [".ssh", ".aws", ".codex", ".agents"].contains(&s.as_ref())
    }) {
        return false;
    }
    let matches = |p: &str| path == p || path.starts_with(&format!("{p}/"));
    (repo.include.is_empty() || repo.include.iter().any(|p| matches(p)))
        && !repo.exclude.iter().any(|p| matches(p))
}
async fn git(path: &str, args: &[&str], max: usize) -> Result<Vec<u8>> {
    let mut c = Command::new("/usr/bin/git");
    c.args([
        "-c",
        "core.fsmonitor=false",
        "-c",
        "core.hooksPath=/dev/null",
        "-c",
        "core.pager=cat",
        "-c",
        "diff.external=",
    ]);
    c.arg("-C")
        .arg(path)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE")
        .env_remove("GIT_CONFIG_COUNT")
        .env_remove("GIT_CONFIG_PARAMETERS");
    let mut child = c.spawn().context("invalid: git unavailable")?;
    let out = child.stdout.take().context("git stdout unavailable")?;
    let result=tokio::time::timeout(std::time::Duration::from_secs(20),async {let mut bytes=vec![];out.take(max as u64+1).read_to_end(&mut bytes).await?;if bytes.len()>max{bail!("invalid: repository command output exceeds bound");}if !child.wait().await?.success(){bail!("invalid: git operation failed; choose a local repository and valid commit reference");}Ok::<_,anyhow::Error>(bytes)}).await.context("invalid: git command timed out")?;
    let _ = child.kill().await;
    result
}
async fn commit(path: &str, reference: &str) -> Result<String> {
    if reference.is_empty()
        || reference.len() > 200
        || reference.starts_with('-')
        || !reference
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_./-".contains(&b))
    {
        bail!("invalid commit reference");
    }
    let r = String::from_utf8(
        git(
            path,
            &["rev-parse", "--verify", &format!("{reference}^{{commit}}")],
            100,
        )
        .await?,
    )?
    .trim()
    .to_string();
    if r.len() < 40 || r.len() > 64 || !r.bytes().all(|b| b.is_ascii_hexdigit()) {
        bail!("invalid commit hash");
    }
    Ok(r)
}
async fn load(
    tx: &mut Transaction<'_, Sqlite>,
    id: &str,
    access: &Access,
) -> Result<RepositoryConnection> {
    let record: String = sqlx::query_scalar("SELECT record FROM repo_connections WHERE id=?")
        .bind(id)
        .fetch_optional(&mut **tx)
        .await?
        .context("record not found")?;
    let r: RepositoryConnection = serde_json::from_str(&record)?;
    let scope = project_sources(tx, &r.project_id).await?;
    permit(tx, &scope, access).await?;
    if access.source_ids.is_some() && !r.allow_external {
        bail!("record not found");
    }
    Ok(r)
}
fn visible(mut repo: RepositoryConnection, access: &Access) -> RepositoryConnection {
    if access.source_ids.is_some() {
        repo.path = "[local repository]".into();
        repo.symbol_service = None;
    }
    repo
}
async fn safe_file(repo: &RepositoryConnection, file: &str) -> Result<PathBuf> {
    if !allowed(repo, file) {
        bail!("invalid: file is outside approved repository selection");
    }
    let root = tokio::fs::canonicalize(&repo.path).await?;
    let path = tokio::fs::canonicalize(root.join(file)).await?;
    if !path.starts_with(&root) || !tokio::fs::metadata(&path).await?.is_file() {
        bail!("invalid: file escapes repository or is not a regular file");
    }
    Ok(path)
}

async fn read_text(path: &Path) -> Result<String> {
    let file = tokio::fs::File::open(path).await?;
    let mut bytes = Vec::new();
    file.take(100001).read_to_end(&mut bytes).await?;
    if bytes.len() > 100000 {
        bail!("invalid: symbol file exceeds 100KB");
    }
    let text = String::from_utf8(bytes).context("invalid: symbol file must be UTF-8")?;
    if text.contains('\0') {
        bail!("invalid: symbol file contains binary data");
    }
    Ok(text)
}

impl Store {
    pub async fn migrate_repository(&self) -> Result<()> {
        sqlx::raw_sql(include_str!("schema.sql"))
            .execute(&self.pool)
            .await?;
        Ok(())
    }
    pub async fn repositories(&self, access: &Access) -> Result<Vec<RepositoryConnection>> {
        let mut tx = self.pool.begin().await?;
        let ids = sqlx::query_scalar::<_, String>(
            "SELECT id FROM repo_connections ORDER BY id LIMIT 100",
        )
        .fetch_all(&mut *tx)
        .await?;
        let mut result = vec![];
        for id in ids {
            if let Ok(r) = load(&mut tx, &id, access).await {
                result.push(visible(r, access));
            }
        }
        tx.commit().await?;
        Ok(result)
    }
    pub async fn repository(&self, id: &str, access: &Access) -> Result<RepositoryConnection> {
        let mut tx = self.pool.begin().await?;
        let r = load(&mut tx, id, access).await?;
        tx.commit().await?;
        Ok(visible(r, access))
    }
    pub async fn save_repository(
        &self,
        id: Option<&str>,
        req: RepositoryRequest,
    ) -> Result<RepositoryConnection> {
        validate_text(&req.name, 200, "repository name")?;
        validate_text(&req.path, 4096, "repository path")?;
        if req.include.len() + req.exclude.len() > 100
            || req
                .include
                .iter()
                .chain(&req.exclude)
                .any(|p| !prefix_valid(p))
        {
            bail!("invalid repository path prefixes");
        }
        if let Some(s) = &req.symbol_service {
            if !req.approve_service
                || !Path::new(&s.command).is_absolute()
                || s.command.len() > 4096
                || s.command.contains('\0')
                || s.args.len() > 30
                || s.args.iter().any(|a| a.len() > 2000 || a.contains('\0'))
                || s.language_id.is_empty()
                || s.language_id.len() > 100
            {
                bail!("invalid: explicitly approve a bounded absolute symbol-service command");
            }
        }
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let (digest, old) = operation(
            &mut tx,
            "repository-save",
            id.unwrap_or("new"),
            &req.idempotency_key,
            &req,
        )
        .await?;
        if let Some(old) = old {
            let r = load(&mut tx, &old, &Access::admin()).await?;
            tx.commit().await?;
            return Ok(r);
        }
        let scope = project_sources(&mut tx, &req.project_id).await?;
        permit(&mut tx, &scope, &Access::admin()).await?;
        let old = if let Some(id) = id {
            let old = load(&mut tx, id, &Access::admin()).await?;
            if old.version != req.expected_version || old.project_id != req.project_id {
                bail!("conflict: repository version/scope changed");
            }
            Some(old)
        } else {
            if req.expected_version != 0 {
                bail!("conflict: new repository expects version 0");
            }
            None
        };
        let root = tokio::fs::canonicalize(&req.path)
            .await
            .context("invalid local repository path")?;
        let root = root
            .to_str()
            .context("invalid: path must be UTF-8")?
            .to_string();
        let reported =
            String::from_utf8(git(&root, &["rev-parse", "--show-toplevel"], 5000).await?)?;
        let reported = tokio::fs::canonicalize(reported.trim()).await?;
        if reported != Path::new(&root) {
            bail!("invalid: choose the repository root directory");
        }
        if old.as_ref().is_some_and(|r| r.path != root) {
            bail!("invalid: repository path is immutable; register a new repository");
        }
        let checkpoint = if let Some(r) = &old {
            r.checkpoint.clone()
        } else {
            commit(&root, &req.initial_ref).await?
        };
        let timestamp = now();
        let r = RepositoryConnection {
            id: old
                .as_ref()
                .map(|r| r.id.clone())
                .unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
            version: old.as_ref().map(|r| r.version + 1).unwrap_or(1),
            project_id: req.project_id.clone(),
            name: req.name.clone(),
            path: root,
            include: req.include.clone(),
            exclude: req.exclude.clone(),
            allow_external: req.allow_external,
            symbol_service: req.symbol_service.clone(),
            checkpoint,
            created_at: old.map(|r| r.created_at).unwrap_or(timestamp.clone()),
            updated_at: timestamp,
        };
        sqlx::query("INSERT INTO repo_connections(id,project_id,version,record) VALUES(?,?,?,?) ON CONFLICT(id) DO UPDATE SET version=excluded.version,record=excluded.record").bind(&r.id).bind(&r.project_id).bind(r.version).bind(serde_json::to_string(&r)?).execute(&mut *tx).await?;
        remember(&mut tx, &req.idempotency_key, &digest, &r.id).await?;
        event(&mut tx, "repository", &r.id).await?;
        tx.commit().await?;
        Ok(r)
    }
}
mod reports;
mod symbols;
