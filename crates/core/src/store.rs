use anyhow::{Context, Result, bail};
use chrono::Utc;
use sha2::{Digest, Sha256};
use sqlx::{
    QueryBuilder, Row, Sqlite, SqlitePool,
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions},
};
use std::{path::Path, str::FromStr, time::Duration};
use thesaurus_contracts::*;
use uuid::Uuid;

#[derive(Clone)]
pub struct Store {
    pub pool: SqlitePool,
    pub(crate) data_dir: std::path::PathBuf,
    pub providers: crate::plugins::registry::ProviderRegistry,
}
pub fn now() -> String {
    Utc::now().to_rfc3339()
}
pub fn hash(value: &str) -> String {
    format!("{:x}", Sha256::digest(value.as_bytes()))
}
pub fn token() -> String {
    format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple())
}
pub fn validate_text(value: &str, max: usize, field: &str) -> Result<()> {
    if value.trim().is_empty() || value.len() > max || value.contains('\0') {
        bail!("invalid {field}");
    }
    Ok(())
}

impl Store {
    pub async fn open(path: &Path) -> Result<Self> {
        if path.exists() && std::fs::symlink_metadata(path)?.file_type().is_symlink() {
            bail!("database must not be a symlink");
        }
        let options = SqliteConnectOptions::from_str(&format!("sqlite://{}", path.display()))?
            .create_if_missing(true)
            .foreign_keys(true)
            .journal_mode(SqliteJournalMode::Wal)
            .busy_timeout(Duration::from_secs(5));
        let pool = SqlitePoolOptions::new()
            .max_connections(4)
            .connect_with(options)
            .await?;
        let version: i64 = sqlx::query_scalar("PRAGMA user_version")
            .fetch_one(&pool)
            .await?;
        if version > 6 {
            bail!("database schema is newer than this version of Thesaurus");
        }
        if version == 0 {
            sqlx::raw_sql(include_str!("schema.sql"))
                .execute(&pool)
                .await?;
        }
        let store = Self {
            pool: pool.clone(),
            data_dir: std::fs::canonicalize(
                path.parent()
                    .filter(|p| !p.as_os_str().is_empty())
                    .unwrap_or_else(|| Path::new(".")),
            )?,
            providers: crate::plugins::native::service_registry()?,
        };
        if version < 2 {
            store.migrate_runtime().await?;
        }
        if version < 3 {
            store.migrate_briefs().await?;
        }
        if version < 4 {
            store.migrate_continuity().await?;
        }
        if version < 5 {
            store.migrate_focus().await?;
        }
        store.migrate_delivery().await?;
        store.migrate_recipes().await?;
        store.migrate_evaluations().await?;
        store.migrate_memory_review().await?;
        store.migrate_annotations().await?;
        store.migrate_context_delivery().await?;
        store.migrate_actions().await?;
        store.migrate_retrieval().await?;
        store.migrate_procedures().await?;
        store.migrate_capture().await?;
        store.migrate_onboarding().await?;
        store
            .recover_capture(path.parent().unwrap_or_else(|| std::path::Path::new(".")))
            .await?;
        store.migrate_insights().await?;
        store.migrate_knowledge().await?;
        store.migrate_usage_controls().await?;
        store.migrate_repository().await?;
        store.migrate_source_controls().await?;
        store.migrate_discovery().await?;
        store.migrate_plugins().await?;
        store.migrate_bragbook().await?;
        store.seed_native_providers().await?;
        sqlx::query("PRAGMA user_version=6").execute(&pool).await?;
        store.recover_runtime().await?;
        store.recover_briefs().await?;
        store.recover_meeting_summaries().await?;
        sqlx::query("UPDATE jobs SET status=CASE WHEN connection='local' THEN 'queued' ELSE 'failed' END, summary='Interrupted during previous service run; explicit retry required for model usage' WHERE status='processing'").execute(&pool).await?;
        Ok(store)
    }

    pub async fn import(&self, req: ImportRequest) -> Result<ImportResult> {
        self.import_into_project(req, None).await
    }

    /// Publish a recording source and its project membership atomically. Outbox
    /// consumers can never observe the new revision before its approved scope.
    pub(crate) async fn import_into_project(
        &self,
        req: ImportRequest,
        project: Option<&str>,
    ) -> Result<ImportResult> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let (document_id, changed) = Self::import_tx(&mut tx, &req, project).await?;
        tx.commit().await?;
        self.refresh_discovery_document(&document_id).await?;
        Ok(ImportResult {
            document: self.document(&document_id, None, &Access::admin()).await?,
            changed,
        })
    }

    /// Shared canonical import; callers own the writer transaction and may
    /// atomically enforce additional grants, provenance and destination scope.
    pub(crate) async fn import_tx(
        tx: &mut sqlx::Transaction<'_, Sqlite>,
        req: &ImportRequest,
        project: Option<&str>,
    ) -> Result<(String, bool)> {
        validate_text(&req.source_name, 200, "source name")?;
        validate_text(&req.title, 300, "title")?;
        validate_text(&req.uri, 2000, "source URI")?;
        validate_text(&req.body, 200_000, "document body")?;
        if !["transcript", "markdown", "knowledge", "repository"].contains(&req.kind.as_str()) {
            bail!("invalid source kind");
        }
        let source_id = format!("src-{}", &hash(&req.uri)[..24]);
        let document_id = format!("doc-{}", &hash(&req.uri)[..24]);
        let digest = hash(&format!("{}\0{}", req.title, req.body));
        let timestamp = now();
        sqlx::query("INSERT INTO sources(id,name,kind,uri,allow_external,updated_at) VALUES(?,?,?,?,?,?) ON CONFLICT(uri) DO UPDATE SET name=excluded.name,kind=excluded.kind,allow_external=excluded.allow_external,updated_at=excluded.updated_at")
            .bind(&source_id).bind(&req.source_name).bind(&req.kind).bind(&req.uri).bind(req.allow_external).bind(&timestamp).execute(&mut **tx).await?;
        if let Some(project) = project.filter(|p| !p.is_empty()) {
            let mut scope = crate::continuity::project_sources(tx, project).await?;
            if !scope.contains(&source_id) {
                if scope.len() >= 50 {
                    bail!(
                        "conflict: project source limit reached; transcript remains in the recording"
                    );
                }
                scope.push(source_id.clone());
                scope.sort();
                sqlx::query("UPDATE project_briefs SET source_ids=?,version=version+1,updated_at=? WHERE id=?").bind(serde_json::to_string(&scope)?).bind(&timestamp).bind(project).execute(&mut **tx).await?;
                crate::continuity::event(tx, "brief", project).await?;
            }
        }
        let previous = sqlx::query("SELECT content_hash,revision FROM documents WHERE id=?")
            .bind(&document_id)
            .fetch_optional(&mut **tx)
            .await?;
        let changed = previous
            .as_ref()
            .is_none_or(|r| r.get::<String, _>("content_hash") != digest);
        if changed {
            let revision = previous.map_or(1, |r| r.get::<i64, _>("revision") + 1);
            sqlx::query("INSERT INTO documents(id,source_id,title,body,revision,content_hash,updated_at) VALUES(?,?,?,?,?,?,?) ON CONFLICT(id) DO UPDATE SET title=excluded.title,body=excluded.body,revision=excluded.revision,content_hash=excluded.content_hash,updated_at=excluded.updated_at")
                .bind(&document_id).bind(&source_id).bind(&req.title).bind(&req.body).bind(revision).bind(&digest).bind(&timestamp).execute(&mut **tx).await?;
            sqlx::query("INSERT INTO revisions(document_id,revision,title,body,content_hash,updated_at) VALUES(?,?,?,?,?,?)").bind(&document_id).bind(revision).bind(&req.title).bind(&req.body).bind(&digest).bind(&timestamp).execute(&mut **tx).await?;
            sqlx::query("DELETE FROM documents_fts WHERE document_id=?")
                .bind(&document_id)
                .execute(&mut **tx)
                .await?;
            sqlx::query("INSERT INTO documents_fts(document_id,title,body) VALUES(?,?,?)")
                .bind(&document_id)
                .bind(&req.title)
                .bind(&req.body)
                .execute(&mut **tx)
                .await?;
            let previous_tasks =
                sqlx::query("SELECT * FROM tasks WHERE document_id=? AND status='candidate'")
                    .bind(&document_id)
                    .fetch_all(&mut **tx)
                    .await?
                    .iter()
                    .map(task_row)
                    .collect::<Vec<_>>();
            sqlx::query("UPDATE tasks SET status='superseded',version=version+1,updated_at=? WHERE document_id=? AND status='candidate'").bind(&timestamp).bind(&document_id).execute(&mut **tx).await?;
            for before in previous_tasks {
                let row = sqlx::query("SELECT * FROM tasks WHERE id=?")
                    .bind(&before.id)
                    .fetch_one(&mut **tx)
                    .await?;
                crate::insights::record_task_transition_tx(
                    tx,
                    Some(&before),
                    &task_row(&row),
                    "source_revision_superseded_candidate",
                )
                .await?;
            }
            sqlx::query("INSERT INTO jobs(id,document_id,revision,connection,status,updated_at) VALUES(?,?,?,'local','queued',?)").bind(Uuid::new_v4().to_string()).bind(&document_id).bind(revision).bind(&timestamp).execute(&mut **tx).await?;
        }
        sqlx::query("INSERT INTO events(kind,record_id) VALUES('document',?)")
            .bind(&document_id)
            .execute(&mut **tx)
            .await?;
        Ok((document_id, changed))
    }

    pub async fn sources(&self, access: &Access) -> Result<Vec<Source>> {
        let rows = sqlx::query("SELECT s.*,COUNT(d.id) AS document_count FROM sources s LEFT JOIN documents d ON s.id=d.source_id GROUP BY s.id ORDER BY s.updated_at DESC").fetch_all(&self.pool).await?;
        Ok(rows
            .into_iter()
            .map(|r| Source {
                id: r.get("id"),
                name: r.get("name"),
                kind: r.get("kind"),
                uri: r.get("uri"),
                allow_external: r.get("allow_external"),
                updated_at: r.get("updated_at"),
                document_count: r.get("document_count"),
            })
            .filter(|s| access.permits(&s.id, s.allow_external))
            .collect())
    }

    pub async fn document(
        &self,
        id: &str,
        revision: Option<i64>,
        access: &Access,
    ) -> Result<Document> {
        let row = sqlx::query("SELECT d.*,s.allow_external FROM documents d JOIN sources s ON s.id=d.source_id WHERE d.id=?").bind(id).fetch_optional(&self.pool).await?.context("record not found")?;
        if !access.permits(row.get("source_id"), row.get("allow_external")) {
            bail!("record not found");
        }
        let mut document = document_row(&row);
        if let Some(rev) = revision {
            let r = sqlx::query("SELECT * FROM revisions WHERE document_id=? AND revision=?")
                .bind(id)
                .bind(rev)
                .fetch_optional(&self.pool)
                .await?
                .context("record not found")?;
            document.title = r.get("title");
            document.body = r.get("body");
            document.revision = rev;
            document.content_hash = r.get("content_hash");
            document.updated_at = r.get("updated_at");
        }
        Ok(document)
    }

    pub async fn snapshot(&self, access: &Access) -> Result<Snapshot> {
        let sources = self.sources(access).await?;
        let ids: Vec<_> = sources.iter().map(|s| s.id.clone()).collect();
        let rows = sqlx::query("SELECT * FROM documents ORDER BY updated_at DESC")
            .fetch_all(&self.pool)
            .await?;
        let documents: Vec<_> = rows
            .iter()
            .map(document_row)
            .filter(|d| ids.contains(&d.source_id))
            .collect();
        let doc_ids: Vec<_> = documents.iter().map(|d| d.id.clone()).collect();
        let tasks = sqlx::query("SELECT * FROM tasks ORDER BY CASE status WHEN 'accepted' THEN 0 WHEN 'candidate' THEN 1 ELSE 2 END,due IS NULL,due,updated_at DESC").fetch_all(&self.pool).await?.iter().map(task_row).filter(|t| doc_ids.contains(&t.evidence.document_id)).collect();
        let jobs = sqlx::query("SELECT * FROM jobs ORDER BY updated_at DESC LIMIT 200")
            .fetch_all(&self.pool)
            .await?
            .iter()
            .map(job_row)
            .filter(|j| doc_ids.contains(&j.document_id))
            .collect();
        Ok(Snapshot {
            sources,
            documents,
            tasks,
            observations: self.observations(access).await?,
            jobs,
            connections: vec![],
            runtime_runs: if access.source_ids.is_none() {
                self.runtimes().await?
            } else {
                vec![]
            },
            briefs: self.briefs(access).await?,
            threads: if access.source_ids.is_none() {
                self.work_threads(access).await?
            } else {
                vec![]
            },
            meeting_series: if access.source_ids.is_none() {
                self.meeting_series(access).await?
            } else {
                vec![]
            },
            watermark: self.watermark().await?,
        })
    }

    pub async fn search(&self, req: &SearchRequest, access: &Access) -> Result<Vec<SearchHit>> {
        if req.query.len() > 1024 {
            bail!("query too long");
        }
        let terms: Vec<_> = req
            .query
            .split_whitespace()
            .take(20)
            .map(|w| format!("\"{}\"", w.replace('"', "\"\"")))
            .collect();
        if terms.is_empty() {
            return Ok(vec![]);
        }
        let live_only = self.live_only_sources().await?;
        let allowed: Vec<_> = self
            .sources(access)
            .await?
            .into_iter()
            .filter(|s| !live_only.contains(&s.id))
            .filter(|s| req.source_id.as_ref().is_none_or(|id| id == &s.id))
            .map(|s| s.id)
            .collect();
        if allowed.is_empty() {
            return Ok(vec![]);
        }
        let mut q: QueryBuilder<Sqlite> = QueryBuilder::new(
            "SELECT d.id,d.source_id,d.title,d.revision,snippet(documents_fts,2,'','',' … ',40) AS excerpt FROM documents_fts JOIN documents d ON documents_fts.document_id=d.id WHERE documents_fts MATCH ",
        );
        q.push_bind(terms.join(" OR "))
            .push(" AND d.source_id IN (");
        let mut separated = q.separated(",");
        for id in allowed {
            separated.push_bind(id);
        }
        separated.push_unseparated(")");
        q.push(" ORDER BY bm25(documents_fts) LIMIT ")
            .push_bind(req.limit.unwrap_or(20).clamp(1, 50) as i64);
        Ok(q.build()
            .fetch_all(&self.pool)
            .await?
            .iter()
            .map(|r| SearchHit {
                document_id: r.get("id"),
                source_id: r.get("source_id"),
                title: r.get("title"),
                revision: r.get("revision"),
                excerpt: r.get("excerpt"),
            })
            .collect())
    }

    pub async fn context(
        &self,
        query: &str,
        budget: usize,
        access: &Access,
    ) -> Result<ContextBundle> {
        let character_budget = budget.clamp(500, 16000);
        self.context_budgeted(
            ContextBuildRequest {
                query: query.into(),
                character_budget,
                token_budget: 16000,
                source_ids: None,
            },
            access,
        )
        .await
    }

    pub async fn update_task(&self, id: &str, update: TaskUpdate) -> Result<Task> {
        if !["accepted", "dismissed", "completed", "candidate"].contains(&update.status.as_str()) {
            bail!("invalid task status");
        }
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let row = sqlx::query("SELECT * FROM tasks WHERE id=?")
            .bind(id)
            .fetch_optional(&mut *tx)
            .await?
            .context("record not found")?;
        let old = task_row(&row);
        if old.version != update.expected_version || old.status == "superseded" {
            bail!("conflict: task changed; refresh before editing");
        }
        if update.status == "accepted" && old.status == "candidate" {
            let current: i64 = sqlx::query_scalar("SELECT revision FROM documents WHERE id=?")
                .bind(&old.evidence.document_id)
                .fetch_one(&mut *tx)
                .await?;
            if current != old.evidence.revision {
                bail!("conflict: evidence changed; review the current document");
            }
        }
        sqlx::query("UPDATE tasks SET status=?,version=version+1,updated_at=? WHERE id=?")
            .bind(update.status)
            .bind(now())
            .bind(id)
            .execute(&mut *tx)
            .await?;
        sqlx::query("INSERT INTO events(kind,record_id) VALUES('task',?)")
            .bind(id)
            .execute(&mut *tx)
            .await?;
        let task = task_row(
            &sqlx::query("SELECT * FROM tasks WHERE id=?")
                .bind(id)
                .fetch_one(&mut *tx)
                .await?,
        );
        crate::insights::record_task_transition_tx(
            &mut tx,
            Some(&old),
            &task,
            "user_status_change",
        )
        .await?;
        tx.commit().await?;
        Ok(task)
    }

    pub async fn observations(&self, access: &Access) -> Result<Vec<Observation>> {
        let sources = self.sources(access).await?;
        let ids: Vec<_> = sources.iter().map(|s| s.id.clone()).collect();
        let rows = sqlx::query("SELECT * FROM observations ORDER BY created_at DESC LIMIT 500")
            .fetch_all(&self.pool)
            .await?;
        let mut result = vec![];
        for r in rows {
            let obs = observation_row(&r)?;
            if obs.source_ids.iter().all(|id| ids.contains(id))
                && (!obs.source_ids.is_empty()
                    || access.source_ids.is_none()
                    || obs.contributor == access.contributor)
            {
                result.push(obs);
            }
        }
        self.effective_observations(result).await
    }

    pub async fn append_observation(
        &self,
        req: ObservationRequest,
        access: &Access,
    ) -> Result<Observation> {
        if !access.allow_write {
            bail!("forbidden: contribution access required");
        }
        validate_text(&req.body, 16000, "observation")?;
        validate_text(&req.idempotency_key, 200, "idempotency key")?;
        let allowed: Vec<_> = self
            .sources(access)
            .await?
            .into_iter()
            .map(|s| s.id)
            .collect();
        if req.source_ids.iter().any(|id| !allowed.contains(id))
            || req.source_ids.len() > 100
            || req.evidence.len() > 50
        {
            bail!("forbidden: source outside grant");
        }
        if access.source_ids.is_some() && req.source_ids.is_empty() {
            bail!("agent observations must select a permitted source");
        }
        for e in &req.evidence {
            let doc = self
                .document(&e.document_id, Some(e.revision), access)
                .await?;
            if !req.source_ids.contains(&doc.source_id)
                || e.line < 1
                || doc.body.lines().nth(e.line as usize - 1) != Some(e.quote.as_str())
            {
                bail!("invalid evidence locator");
            }
        }
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        if let Some(row) =
            sqlx::query("SELECT * FROM observations WHERE contributor=? AND idempotency_key=?")
                .bind(&access.contributor)
                .bind(&req.idempotency_key)
                .fetch_optional(&mut *tx)
                .await?
        {
            let old = observation_row(&row)?;
            if old.body != req.body
                || old.source_ids != req.source_ids
                || old.evidence != req.evidence
            {
                bail!("conflict: idempotency key has different content");
            }
            return Ok(old);
        }
        let obs = Observation {
            id: Uuid::new_v4().to_string(),
            body: req.body,
            contributor: access.contributor.clone(),
            source_ids: req.source_ids,
            evidence: req.evidence,
            created_at: now(),
        };
        sqlx::query("INSERT INTO observations(id,body,contributor,source_ids,evidence,idempotency_key,created_at) VALUES(?,?,?,?,?,?,?)").bind(&obs.id).bind(&obs.body).bind(&obs.contributor).bind(serde_json::to_string(&obs.source_ids)?).bind(serde_json::to_string(&obs.evidence)?).bind(req.idempotency_key).bind(&obs.created_at).execute(&mut *tx).await?;
        sqlx::query("INSERT INTO events(kind,record_id) VALUES('observation',?)")
            .bind(&obs.id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(obs)
    }

    pub async fn watermark(&self) -> Result<i64> {
        Ok(
            sqlx::query_scalar("SELECT COALESCE(MAX(sequence),0) FROM events")
                .fetch_one(&self.pool)
                .await?,
        )
    }
    pub async fn events(&self, since: i64) -> Result<Vec<ChangeEvent>> {
        Ok(
            sqlx::query("SELECT * FROM events WHERE sequence>? ORDER BY sequence LIMIT 100")
                .bind(since)
                .fetch_all(&self.pool)
                .await?
                .iter()
                .map(|r| ChangeEvent {
                    sequence: r.get("sequence"),
                    kind: r.get("kind"),
                    record_id: r.get("record_id"),
                })
                .collect(),
        )
    }

    pub async fn create_grant(
        &self,
        name: String,
        source_ids: Vec<String>,
        write: bool,
        external: bool,
    ) -> Result<(GrantInfo, String)> {
        validate_text(&name, 100, "agent name")?;
        let sources = self.sources(&Access::admin()).await?;
        if source_ids.is_empty()
            || source_ids
                .iter()
                .any(|id| !sources.iter().any(|s| &s.id == id))
        {
            bail!("grant must select existing sources");
        }
        let info = GrantInfo {
            id: Uuid::new_v4().to_string(),
            name,
            source_ids,
            allow_write: write,
            allow_external: external,
            revoked: false,
        };
        let secret = token();
        sqlx::query("INSERT INTO grants(id,name,token_hash,source_ids,allow_write,allow_external) VALUES(?,?,?,?,?,?)").bind(&info.id).bind(&info.name).bind(hash(&secret)).bind(serde_json::to_string(&info.source_ids)?).bind(write).bind(external).execute(&self.pool).await?;
        Ok((info, secret))
    }
    pub async fn authenticate(&self, secret: &str) -> Result<Access> {
        let row = sqlx::query("SELECT * FROM grants WHERE token_hash=? AND revoked=0")
            .bind(hash(secret))
            .fetch_optional(&self.pool)
            .await?
            .context("unauthorized")?;
        let grant_id: String = row.get("id");
        let filtered_sources = self
            .filter_grant_destinations(
                &grant_id,
                serde_json::from_str(row.get("source_ids"))?,
                row.get("allow_external"),
            )
            .await?;
        Ok(Access {
            contributor: format!(
                "{} ({})",
                row.get::<String, _>("name"),
                row.get::<String, _>("id")
            ),
            source_ids: Some(filtered_sources),
            allow_write: row.get("allow_write"),
            external: row.get("allow_external"),
        })
    }
    pub async fn grants(&self) -> Result<Vec<GrantInfo>> {
        let rows = sqlx::query("SELECT * FROM grants")
            .fetch_all(&self.pool)
            .await?;
        rows.iter()
            .map(|r| {
                Ok(GrantInfo {
                    id: r.get("id"),
                    name: r.get("name"),
                    source_ids: serde_json::from_str(r.get("source_ids"))?,
                    allow_write: r.get("allow_write"),
                    allow_external: r.get("allow_external"),
                    revoked: r.get("revoked"),
                })
            })
            .collect()
    }
    pub async fn revoke_grant(&self, id: &str) -> Result<()> {
        sqlx::query("UPDATE grants SET revoked=1 WHERE id=?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn queue_analysis(
        &self,
        doc_id: &str,
        connection: &str,
        limit: Option<i64>,
    ) -> Result<Job> {
        let doc = self.document(doc_id, None, &Access::admin()).await?;
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let existing =
            sqlx::query("SELECT * FROM jobs WHERE document_id=? AND revision=? AND connection=?")
                .bind(doc_id)
                .bind(doc.revision)
                .bind(connection)
                .fetch_optional(&mut *tx)
                .await?;
        if let Some(row) = existing.as_ref() {
            let job = job_row(row);
            if job.status != "failed" {
                return Ok(job);
            }
        }
        if let Some(limit) = limit {
            let limit =
                crate::usage_controls::effective_limit_tx(&mut tx, connection, limit, true).await?;
            let used = crate::usage_controls::used_tx(&mut tx, connection).await?;
            if used >= limit {
                bail!("forbidden: connection daily limit reached; no fallback was used")
            }
        }
        sqlx::query("INSERT INTO jobs(id,document_id,revision,connection,status,updated_at) VALUES(?,?,?,?,'queued',?) ON CONFLICT(document_id,revision,connection) DO UPDATE SET status=CASE WHEN status='failed' THEN 'queued' ELSE status END,updated_at=excluded.updated_at")
            .bind(Uuid::new_v4().to_string()).bind(doc_id).bind(doc.revision).bind(connection).bind(now()).execute(&mut *tx).await?;
        let job = job_row(
            &sqlx::query("SELECT * FROM jobs WHERE document_id=? AND revision=? AND connection=?")
                .bind(doc_id)
                .bind(doc.revision)
                .bind(connection)
                .fetch_one(&mut *tx)
                .await?,
        );
        crate::usage_controls::reserve_job_tx(&mut tx, &job).await?;
        sqlx::query("INSERT INTO events(kind,record_id) VALUES('job',?)")
            .bind(&job.id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(job)
    }

    pub async fn process_next(&self, config: &crate::Config, work_dir: &Path) -> Result<bool> {
        let effective = self.model_config(config).await?;
        let config = &effective;
        let row=sqlx::query("UPDATE jobs SET status='processing',attempt=attempt+1,updated_at=? WHERE id=(SELECT j.id FROM jobs j WHERE j.status='queued' AND (j.connection='local' OR NOT EXISTS(SELECT 1 FROM usage_control_policy p WHERE p.connection=j.connection AND json_extract(p.record,'$.background_paused')=1)) ORDER BY j.updated_at LIMIT 1) RETURNING *").bind(now()).fetch_optional(&self.pool).await?;
        let Some(row) = row else { return Ok(false) };
        let job = job_row(&row);
        let result=async {
            if job.connection!="local"{
                let configured=config.connections.iter().find(|c|c.id==job.connection).context("unknown connection")?.daily_limit;
                self.start_model_job(&job,configured).await?;
            }
            let doc=self.document(&job.document_id,Some(job.revision),&Access::admin()).await?;
            self.check_source_destination(&doc.source_id,&job.connection,crate::models::external(config,&job.connection)?).await?;
            if crate::models::external(config,&job.connection)? {
                let allowed:bool=sqlx::query_scalar("SELECT allow_external FROM sources WHERE id=?").bind(&doc.source_id).fetch_one(&self.pool).await?;
                if !allowed {bail!("source disclosure permission was withdrawn")}
            }
            let extracted=crate::models::extract(self,&doc,&job.connection,config,work_dir).await?;
            let extracted=self.normalize_candidates(&doc,extracted).await?;
            let mut tx=self.pool.begin_with("BEGIN IMMEDIATE").await?;
            let current:i64=sqlx::query_scalar("SELECT revision FROM documents WHERE id=?").bind(&doc.id).fetch_one(&mut *tx).await?;
            if current!=job.revision { bail!("evidence changed while processing"); }
            for candidate in extracted {
                validate_text(&candidate.title,500,"candidate title")?;
                if candidate.line<1 || doc.body.lines().nth(candidate.line as usize-1)!=Some(candidate.quote.as_str()) { bail!("model returned an invalid evidence citation"); }
                sqlx::query("INSERT OR IGNORE INTO tasks(id,title,owner,due,status,document_id,revision,line,quote,method,updated_at) VALUES(?,?,?,?,'candidate',?,?,?,?,?,?)")
                    .bind(Uuid::new_v4().to_string()).bind(candidate.title).bind(candidate.owner).bind(candidate.due).bind(&doc.id).bind(doc.revision).bind(candidate.line).bind(candidate.quote).bind(&job.connection).bind(now()).execute(&mut *tx).await?;
            }
            for row in sqlx::query("SELECT * FROM tasks WHERE document_id=? AND revision=? AND version=1").bind(&doc.id).bind(doc.revision).fetch_all(&mut *tx).await?{crate::insights::record_task_transition_tx(&mut tx,None,&task_row(&row),"extracted_candidate").await?;}
            sqlx::query("UPDATE jobs SET status='completed',summary='Action candidates ready for review',updated_at=? WHERE id=?").bind(now()).bind(&job.id).execute(&mut *tx).await?;
            sqlx::query("INSERT INTO events(kind,record_id) VALUES('analysis',?)").bind(&doc.id).execute(&mut *tx).await?;
            tx.commit().await?; Ok::<_,anyhow::Error>(())
        }.await;
        if let Err(error) = result {
            let reason = error.to_string();
            let summary = if reason
                == "forbidden: background work exceeds current budget; no provider request made"
            {
                "Background work exceeds the current budget; no provider request was made. Review usage limits before retrying."
            } else if reason == "forbidden: background model work is paused for this connection" {
                "Background model work is paused; no provider request was made. Resume this connection before retrying."
            } else if job.connection == "local" {
                "Local extraction failed; retry after checking the document"
            } else {
                "Model run failed or returned invalid evidence; check configuration and explicitly retry"
            };
            sqlx::query("UPDATE jobs SET status='failed',summary=?,updated_at=? WHERE id=?")
                .bind(summary)
                .bind(now())
                .bind(&job.id)
                .execute(&self.pool)
                .await?;
            sqlx::query("INSERT INTO events(kind,record_id) VALUES('job',?)")
                .bind(&job.id)
                .execute(&self.pool)
                .await?;
            eprintln!(
                "analysis failed ({})",
                if error.to_string().contains("citation") {
                    "invalid_citation"
                } else {
                    "processing_error"
                }
            );
        }
        Ok(true)
    }
}

pub(crate) fn document_row(r: &sqlx::sqlite::SqliteRow) -> Document {
    Document {
        id: r.get("id"),
        source_id: r.get("source_id"),
        title: r.get("title"),
        body: r.get("body"),
        revision: r.get("revision"),
        content_hash: r.get("content_hash"),
        updated_at: r.get("updated_at"),
    }
}
pub(crate) fn task_row(r: &sqlx::sqlite::SqliteRow) -> Task {
    Task {
        id: r.get("id"),
        title: r.get("title"),
        owner: r.get("owner"),
        due: r.get("due"),
        status: r.get("status"),
        evidence: Evidence {
            document_id: r.get("document_id"),
            revision: r.get("revision"),
            line: r.get("line"),
            quote: r.get("quote"),
        },
        method: r.get("method"),
        version: r.get("version"),
        updated_at: r.get("updated_at"),
    }
}
pub(crate) fn job_row(r: &sqlx::sqlite::SqliteRow) -> Job {
    Job {
        id: r.get("id"),
        document_id: r.get("document_id"),
        revision: r.get("revision"),
        connection: r.get("connection"),
        status: r.get("status"),
        attempt: r.get("attempt"),
        summary: r.get("summary"),
        updated_at: r.get("updated_at"),
    }
}
pub(crate) fn observation_row(r: &sqlx::sqlite::SqliteRow) -> Result<Observation> {
    Ok(Observation {
        id: r.get("id"),
        body: r.get("body"),
        contributor: r.get("contributor"),
        source_ids: serde_json::from_str(r.get("source_ids"))?,
        evidence: serde_json::from_str(r.get("evidence"))?,
        created_at: r.get("created_at"),
    })
}
