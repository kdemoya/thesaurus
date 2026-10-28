mod ledger;
use crate::{
    Store,
    continuity::{event, operation, remember},
    store::{hash, validate_text},
};
use anyhow::{Context, Result, bail};
pub(crate) use ledger::{reserve_job_tx, used_tx};
use sqlx::{Sqlite, Transaction};
use thesaurus_contracts::*;
pub(crate) async fn policy_tx(
    tx: &mut Transaction<'_, Sqlite>,
    connection: &str,
    configured: i64,
) -> Result<UsagePolicy> {
    let record: Option<String> =
        sqlx::query_scalar("SELECT record FROM usage_control_policy WHERE connection=?")
            .bind(connection)
            .fetch_optional(&mut **tx)
            .await?;
    let mut p = match record {
        Some(r) => serde_json::from_str(&r)?,
        None => UsagePolicy {
            connection: connection.into(),
            version: 0,
            daily_limit: configured,
            interactive_reserve: 0,
            background_paused: false,
            fallback_connection: None,
        },
    };
    p.daily_limit = p.daily_limit.min(configured);
    p.interactive_reserve = p.interactive_reserve.min(p.daily_limit);
    Ok(p)
}
pub(crate) async fn effective_limit_tx(
    tx: &mut Transaction<'_, Sqlite>,
    connection: &str,
    configured: i64,
    background: bool,
) -> Result<i64> {
    let p = policy_tx(tx, connection, configured).await?;
    if background && p.background_paused {
        bail!("forbidden: background model work is paused for this connection");
    }
    Ok(p.daily_limit - if background { p.interactive_reserve } else { 0 })
}
impl Store {
    pub async fn migrate_usage_controls(&self) -> Result<()> {
        sqlx::raw_sql("CREATE TABLE IF NOT EXISTS usage_control_policy(connection TEXT PRIMARY KEY,record TEXT NOT NULL); CREATE TABLE IF NOT EXISTS usage_control_versions(connection TEXT NOT NULL,version INTEGER NOT NULL,record TEXT NOT NULL,PRIMARY KEY(connection,version)); CREATE TABLE IF NOT EXISTS usage_control_fallbacks(run_id TEXT PRIMARY KEY,primary_run_id TEXT NOT NULL,record TEXT NOT NULL);").execute(&self.pool).await?;
        ledger::migrate_ledger(self).await?;
        Ok(())
    }
    pub async fn connection_usage(
        &self,
        connection: &str,
        configured: i64,
        billing: String,
    ) -> Result<ConnectionUsage> {
        let mut tx = self.pool.begin().await?;
        let policy = policy_tx(&mut tx, connection, configured).await?;
        let used = used_tx(&mut tx, connection).await?;
        Ok(ConnectionUsage {
            configured_limit: configured,
            requests_reserved_today: used,
            interactive_remaining: (policy.daily_limit - used).max(0),
            background_remaining: if policy.background_paused {
                0
            } else {
                (policy.daily_limit - policy.interactive_reserve - used).max(0)
            },
            policy,
            billing,
        })
    }
    pub async fn set_usage_policy(
        &self,
        connection: &str,
        req: UsagePolicyRequest,
        configured: i64,
    ) -> Result<UsagePolicy> {
        validate_text(connection, 100, "connection")?;
        if req.daily_limit < 1
            || req.daily_limit > configured
            || req.interactive_reserve < 0
            || req.interactive_reserve > req.daily_limit
        {
            bail!(
                "invalid: daily limit cannot exceed configuration; reserve must fit within daily limit"
            );
        }
        if req.fallback_connection.as_deref() == Some(connection) {
            bail!("invalid: fallback must use a different selected connection");
        }
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let (digest, old) = operation(
            &mut tx,
            "usage-policy",
            connection,
            &req.idempotency_key,
            &req,
        )
        .await?;
        if old.is_some() {
            return policy_tx(&mut tx, connection, configured).await;
        }
        let previous = policy_tx(&mut tx, connection, configured).await?;
        if previous.version != req.expected_version {
            bail!("conflict: usage policy changed");
        }
        let p = UsagePolicy {
            connection: connection.into(),
            version: previous.version + 1,
            daily_limit: req.daily_limit,
            interactive_reserve: req.interactive_reserve,
            background_paused: req.background_paused,
            fallback_connection: req.fallback_connection,
        };
        let record = serde_json::to_string(&p)?;
        sqlx::query("INSERT INTO usage_control_policy(connection,record) VALUES(?,?) ON CONFLICT(connection) DO UPDATE SET record=excluded.record").bind(connection).bind(&record).execute(&mut *tx).await?;
        sqlx::query("INSERT INTO usage_control_versions(connection,version,record) VALUES(?,?,?)")
            .bind(connection)
            .bind(p.version)
            .bind(record)
            .execute(&mut *tx)
            .await?;
        remember(&mut tx, &req.idempotency_key, &digest, connection).await?;
        event(&mut tx, "usage_policy", connection).await?;
        tx.commit().await?;
        Ok(p)
    }
    pub async fn fallback_preview(&self, id: &str) -> Result<FallbackPreview> {
        let mut tx = self.pool.begin().await?;
        self.prepare_fallback(&mut tx, id).await
    }
    pub(crate) async fn prepare_fallback(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        id: &str,
    ) -> Result<FallbackPreview> {
        let record: String = sqlx::query_scalar("SELECT record FROM runtime_runs WHERE id=?")
            .bind(id)
            .fetch_optional(&mut **tx)
            .await?
            .context("record not found")?;
        let run: RuntimeRun = serde_json::from_str(&record)?;
        if !["failed", "interrupted", "cancelled"].contains(&run.status.as_str()) {
            bail!("conflict: fallback requires a failed, interrupted or cancelled primary run");
        }
        let record: String =
            sqlx::query_scalar("SELECT record FROM usage_control_policy WHERE connection=?")
                .bind(&run.connection)
                .fetch_optional(&mut **tx)
                .await?
                .context("forbidden: configure an explicit fallback rule first")?;
        let p: UsagePolicy = serde_json::from_str(&record)?;
        let target = p
            .fallback_connection
            .context("forbidden: this connection has no explicit fallback rule")?;
        let mut policies = vec![];
        for source in &run.source_ids {
            crate::source_controls::check_connection_tx(tx, source, &target, true).await?;
            let record: Option<String> =
                sqlx::query_scalar("SELECT record FROM source_control_policy WHERE source_id=?")
                    .bind(source)
                    .fetch_optional(&mut **tx)
                    .await?;
            policies.push(record);
        }
        let signature = hash(&serde_json::to_string(&(
            &run.id,
            &run.connection,
            &run.prompt,
            &run.source_ids,
            p.version,
            &target,
            policies,
        ))?);
        Ok(FallbackPreview{primary_run_id:id.into(),primary_connection:run.connection,fallback_connection:target,policy_version:p.version,approximate_input_tokens:run.prompt.len().div_ceil(4),prompt:run.prompt,source_ids:run.source_ids,signature,note:"A fallback is a separate explicit request and may incur usage on its selected account. The primary may already have incurred usage. No automatic chain or retry is performed.".into()})
    }
    pub(crate) async fn link_fallback(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        run_id: &str,
        p: &FallbackPreview,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO usage_control_fallbacks(run_id,primary_run_id,record) VALUES(?,?,?)",
        )
        .bind(run_id)
        .bind(&p.primary_run_id)
        .bind(serde_json::to_string(p)?)
        .execute(&mut **tx)
        .await?;
        Ok(())
    }
}
