use crate::{
    Store,
    continuity::{event, operation, permit, project_sources, remember},
    store::{hash, now, validate_text},
};
use anyhow::{Context, Result, bail};
use serde_json::json;
use sqlx::{Row, Sqlite, Transaction};
use thesaurus_contracts::*;
fn visible(mut c: UpstreamConnection, access: &Access) -> UpstreamConnection {
    if access.source_ids.is_some() {
        c.command = None;
        c.args.clear();
        c.token_env = None;
        c.endpoint = None;
        c.tool_arguments = json!({});
    }
    c
}
async fn load(
    tx: &mut Transaction<'_, Sqlite>,
    id: &str,
    access: &Access,
) -> Result<UpstreamConnection> {
    let s: String = sqlx::query_scalar("SELECT record FROM retrieval_upstreams WHERE id=?")
        .bind(id)
        .fetch_optional(&mut **tx)
        .await?
        .context("record not found")?;
    let c: UpstreamConnection = serde_json::from_str(&s)?;
    permit(tx, std::slice::from_ref(&c.source_id), access).await?;
    let scope = project_sources(tx, &c.project_id).await?;
    permit(tx, &scope, access).await?;
    Ok(c)
}
impl Store {
    pub async fn migrate_retrieval(&self) -> Result<()> {
        sqlx::raw_sql(include_str!("schema.sql"))
            .execute(&self.pool)
            .await?;
        let columns = sqlx::query("PRAGMA table_info(retrieval_fetches)")
            .fetch_all(&self.pool)
            .await?;
        if !columns
            .iter()
            .any(|r| r.get::<String, _>("name") == "revision")
        {
            sqlx::query("ALTER TABLE retrieval_fetches ADD COLUMN revision INTEGER")
                .execute(&self.pool)
                .await?;
        }
        if !columns
            .iter()
            .any(|r| r.get::<String, _>("name") == "request_json")
        {
            sqlx::query(
                "ALTER TABLE retrieval_fetches ADD COLUMN request_json TEXT NOT NULL DEFAULT '{}'",
            )
            .execute(&self.pool)
            .await?;
        }
        sqlx::query("UPDATE retrieval_fetches SET status='interrupted',error='Service restarted; explicit retry requires a new request' WHERE status='fetching'").execute(&self.pool).await?;
        Ok(())
    }
    pub async fn live_only_sources(&self) -> Result<Vec<String>> {
        Ok(
            sqlx::query_scalar("SELECT source_id FROM retrieval_policies WHERE policy='live'")
                .fetch_all(&self.pool)
                .await?,
        )
    }
    pub async fn upstream(&self, id: &str, access: &Access) -> Result<UpstreamConnection> {
        let mut tx = self.pool.begin().await?;
        let c = load(&mut tx, id, access).await?;
        tx.commit().await?;
        Ok(visible(c, access))
    }
    pub async fn upstreams(&self, access: &Access) -> Result<Vec<UpstreamConnection>> {
        let mut tx = self.pool.begin().await?;
        let ids = sqlx::query_scalar::<_, String>(
            "SELECT id FROM retrieval_upstreams ORDER BY rowid DESC LIMIT 100",
        )
        .fetch_all(&mut *tx)
        .await?;
        let mut out = vec![];
        for id in ids {
            if let Ok(c) = load(&mut tx, &id, access).await {
                out.push(visible(c, access));
            }
        }
        tx.commit().await?;
        Ok(out)
    }
    pub async fn save_upstream(
        &self,
        id: Option<&str>,
        req: UpstreamRequest,
    ) -> Result<UpstreamConnection> {
        validate_text(&req.name, 200, "upstream name")?;
        if !req.approve_upstream {
            bail!(
                "forbidden: explicitly approve configured upstream read destination and operation"
            );
        }
        if !["index", "live", "hybrid"].contains(&req.policy.as_str())
            || !(1..=1000).contains(&req.daily_limit)
        {
            bail!("invalid retrieval policy or daily read limit");
        }
        if req.resource_uri.is_some() == req.read_tool.is_some() {
            bail!("invalid: choose exactly one fixed resource URI or approved read tool");
        }
        if let Some(s) = &req.resource_uri {
            validate_text(s, 2000, "resource URI")?;
        }
        if let Some(s) = &req.read_tool {
            validate_text(s, 200, "read tool")?;
            if !req.tool_arguments.is_object() {
                bail!("invalid: tool arguments must be a JSON object");
            }
        }
        if serde_json::to_vec(&req.tool_arguments)?.len() > 16000
            || (!req.allow_query_transmission
                && serde_json::to_string(&req.tool_arguments)?.contains("{query}"))
        {
            bail!(
                "invalid: explicitly approve query transmission before using {{query}} in arguments"
            );
        }
        if req.transport == "stdio" {
            let c = req
                .command
                .as_deref()
                .context("invalid: absolute stdio command required")?;
            if !std::path::Path::new(c).is_absolute()
                || c.len() > 4096
                || c.contains('\0')
                || req.args.len() > 30
                || req.args.iter().any(|s| s.len() > 2000 || s.contains('\0'))
            {
                bail!("invalid upstream command/args");
            }
            if req.endpoint.is_some() || req.token_env.is_some() {
                bail!("invalid: endpoint/token_env apply to HTTP transport only");
            }
        } else if req.transport == "http" {
            let u = reqwest::Url::parse(req.endpoint.as_deref().context("invalid HTTP endpoint")?)?;
            let loopback = matches!(
                u.host_str(),
                Some("localhost" | "127.0.0.1" | "[::1]" | "::1")
            );
            if (u.scheme() != "https" && !(u.scheme() == "http" && loopback))
                || u.username() != ""
                || u.password().is_some()
                || u.fragment().is_some()
                || u.query().is_some()
            {
                bail!(
                    "invalid: choose HTTPS MCP endpoint (HTTP only on loopback), without embedded credentials/query/fragment"
                );
            }
            if req.command.is_some() || !req.args.is_empty() {
                bail!("invalid: stdio command applies to stdio transport only");
            }
            if let Some(e) = &req.token_env {
                if e.is_empty()
                    || e.len() > 200
                    || !e.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
                {
                    bail!("invalid token environment variable name");
                }
            }
        } else {
            bail!("invalid upstream transport");
        }
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let (digest, old) = operation(
            &mut tx,
            "upstream-save",
            id.unwrap_or("new"),
            &req.idempotency_key,
            &req,
        )
        .await?;
        if let Some(old) = old {
            let c = load(&mut tx, &old, &Access::admin()).await?;
            tx.commit().await?;
            return Ok(c);
        }
        let old = if let Some(id) = id {
            let c = load(&mut tx, id, &Access::admin()).await?;
            if c.version != req.expected_version || c.project_id != req.project_id {
                bail!("conflict: upstream configuration changed");
            }
            Some(c)
        } else {
            if req.expected_version != 0 {
                bail!("conflict: new upstream expects version0");
            }
            None
        };
        let mut scope = project_sources(&mut tx, &req.project_id).await?;
        permit(&mut tx, &scope, &Access::admin()).await?;
        let id = old
            .as_ref()
            .map(|c| c.id.clone())
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        let source = old
            .as_ref()
            .map(|c| c.source_id.clone())
            .unwrap_or_else(|| format!("src-mcp-{}", uuid::Uuid::new_v4().simple()));
        let timestamp = now();
        if old.is_none() {
            if scope.len() >= 50 {
                bail!("invalid: project source limit reached");
            }
            sqlx::query("INSERT INTO sources(id,name,kind,uri,allow_external,updated_at) VALUES(?,?,'knowledge',?,0,?)").bind(&source).bind(&req.name).bind(format!("mcp://upstream/{id}")).bind(&timestamp).execute(&mut *tx).await?;
            scope.push(source.clone());
            scope.sort();
            scope.dedup();
            sqlx::query(
                "UPDATE project_briefs SET source_ids=?,version=version+1,updated_at=? WHERE id=?",
            )
            .bind(serde_json::to_string(&scope)?)
            .bind(&timestamp)
            .bind(&req.project_id)
            .execute(&mut *tx)
            .await?;
            event(&mut tx, "brief", &req.project_id).await?;
        }
        let c = UpstreamConnection {
            id: id.clone(),
            version: old.as_ref().map(|c| c.version + 1).unwrap_or(1),
            source_id: source.clone(),
            project_id: req.project_id.clone(),
            name: req.name.clone(),
            transport: req.transport.clone(),
            command: req.command.clone(),
            args: req.args.clone(),
            endpoint: req.endpoint.clone(),
            token_env: req.token_env.clone(),
            resource_uri: req.resource_uri.clone(),
            read_tool: req.read_tool.clone(),
            tool_arguments: req.tool_arguments.clone(),
            policy: req.policy.clone(),
            allow_query_transmission: req.allow_query_transmission,
            allow_agent_live: req.allow_agent_live,
            daily_limit: req.daily_limit,
            last_fetched_at: old.as_ref().and_then(|c| c.last_fetched_at.clone()),
            last_document_id: old.as_ref().and_then(|c| c.last_document_id.clone()),
            last_revision: old.as_ref().and_then(|c| c.last_revision),
            last_error: None,
            created_at: old.map(|c| c.created_at).unwrap_or(timestamp.clone()),
            updated_at: timestamp,
        };
        sqlx::query("INSERT INTO retrieval_upstreams(id,source_id,project_id,version,record) VALUES(?,?,?,?,?) ON CONFLICT(id) DO UPDATE SET version=excluded.version,record=excluded.record").bind(&id).bind(&source).bind(&c.project_id).bind(c.version).bind(serde_json::to_string(&c)?).execute(&mut *tx).await?;
        sqlx::query("INSERT INTO retrieval_policies(source_id,policy) VALUES(?,?) ON CONFLICT(source_id) DO UPDATE SET policy=excluded.policy").bind(&source).bind(&c.policy).execute(&mut *tx).await?;
        remember(&mut tx, &req.idempotency_key, &digest, &id).await?;
        event(&mut tx, "upstream", &id).await?;
        tx.commit().await?;
        Ok(c)
    }
}
mod bounded_http;
mod client;
mod query;
