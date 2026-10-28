use crate::server::{Api, App};
use anyhow::{Context, Result, bail};
use axum::{
    Json,
    extract::{Path, State},
    http::HeaderMap,
};
use serde::Deserialize;
use serde_json::{Value, json};
use thesaurus_contracts::*;
use thesaurus_core::{
    acp::{Control, Outcome},
    config::Connection,
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreviewRequest {
    connection: String,
}
pub async fn get(
    State(app): State<App>,
    h: HeaderMap,
    Path((plugin, id)): Path<(String, String)>,
) -> Api<PluginModelProposal> {
    app.admin(&h).await?;
    Ok(Json(app.store.plugin_model_proposal(&plugin, &id).await?))
}
pub async fn preview(
    State(app): State<App>,
    h: HeaderMap,
    Path((plugin, id)): Path<(String, String)>,
    Json(req): Json<PreviewRequest>,
) -> Api<PluginModelPreview> {
    let config = app.model_config().await?;
    app.admin(&h).await?;
    let c = config
        .connections
        .iter()
        .find(|c| c.id == req.connection)
        .context("invalid: model connection not configured")?;
    Ok(Json(app.store.plugin_model_preview(&plugin, &id, c).await?))
}
pub async fn approve(
    State(app): State<App>,
    h: HeaderMap,
    Path((plugin, id)): Path<(String, String)>,
    Json(req): Json<PluginModelApproveRequest>,
) -> Api<RuntimeRun> {
    let config = app.model_config().await?;
    app.admin(&h).await?;
    app.store.plugin_model_proposal(&plugin, &id).await?;
    let c = config
        .connections
        .iter()
        .find(|c| c.id == req.connection)
        .context("invalid: model connection not configured")?
        .clone();
    if c.kind == "acp"
        && !thesaurus_core::models::executable(c.command.as_deref().unwrap_or_default())
    {
        return Err(anyhow::anyhow!("invalid: configured ACP executable is unavailable").into());
    }
    let reserved = app.store.create_plugin_model_runtime(&id, req, &c).await?;
    Ok(Json(crate::runtime::launch(&app, c, reserved).await))
}
pub async fn reject(
    State(app): State<App>,
    h: HeaderMap,
    Path((plugin, id)): Path<(String, String)>,
) -> Api<PluginModelProposal> {
    app.admin(&h).await?;
    Ok(Json(app.store.reject_plugin_model(&plugin, &id).await?))
}

pub(crate) async fn drive_api(
    app: &App,
    run: &RuntimeRun,
    c: Connection,
    mut controls: tokio::sync::mpsc::Receiver<Control>,
) -> Result<Outcome> {
    let (max_tokens, max_bytes) = model_authority(app, run, &c).await?;
    let work = async {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(120))
            .redirect(reqwest::redirect::Policy::none())
            .retry(reqwest::retry::never())
            .build()?;
        let mut request=client.post(format!("{}/chat/completions",c.endpoint.as_deref().context("invalid: API endpoint missing")?.trim_end_matches('/'))).json(&json!({"model":c.model,"messages":[{"role":"user","content":run.prompt}],"max_tokens":max_tokens}));
        if let Some(reference) = c
            .credential_ref
            .clone()
            .or_else(|| c.token_env.as_ref().map(|name| format!("env:{name}")))
        {
            request = request.bearer_auth(app.store.resolve_credential(&reference).await?);
        }
        app.store
            .runtime_patch(
                &run.id,
                json!({"status":"running","progress":"Requesting selected model API"}),
            )
            .await?;
        let mut response = request.send().await?.error_for_status()?;
        let mut bytes = vec![];
        while let Some(chunk) = response.chunk().await? {
            if bytes.len() + chunk.len() > 256000 {
                bail!("invalid: model response exceeds host byte bounds");
            }
            bytes.extend_from_slice(&chunk);
        }
        let value: Value = serde_json::from_slice(&bytes)?;
        let text = value["choices"][0]["message"]["content"]
            .as_str()
            .context("invalid: provider returned no text")?
            .to_owned();
        thesaurus_core::store::validate_text(&text, max_bytes, "model output")?;
        model_authority(app, run, &c).await?;
        let model = value["model"]
            .as_str()
            .unwrap_or("unknown (provider did not report model)")
            .to_owned();
        app.store.runtime_patch(&run.id,json!({"provenance":RuntimeProvenance{agent_name:"Selected model API".into(),agent_version:"unreported".into(),model,model_source:"provider response; unverified".into(),actual_prompt_hash:thesaurus_core::store::hash(&run.prompt)}})).await?;
        let stop_reason = if value["choices"][0]["finish_reason"].as_str() == Some("stop") {
            "end_turn"
        } else {
            "provider_stopped_without_complete_output"
        };
        Ok::<_, anyhow::Error>(Outcome {
            text,
            stop_reason: stop_reason.into(),
        })
    };
    tokio::pin!(work);
    let mut interval = tokio::time::interval(std::time::Duration::from_millis(250));
    loop {
        tokio::select! {
            result=&mut work=>return result,
            control=controls.recv()=>if matches!(control,None|Some(Control::Cancel)){bail!("cancelled");},
            _=interval.tick()=>{model_authority(app,run,&c).await?;}
        }
    }
}

/// Shared bounded execution for explicitly reserved plugin and meeting requests.
pub(crate) async fn model_authority(
    app: &App,
    run: &RuntimeRun,
    c: &Connection,
) -> Result<(u32, usize)> {
    let config = app.model_config().await?;
    if app.store.onboarding_check_authority(run, c).await? {
        return Ok((64, 4096));
    }
    if let Some(p) = app.store.plugin_model_runtime_preview(&run.id, c).await? {
        return Ok((
            p.proposal.request.max_output_tokens,
            p.proposal.request.max_output_bytes,
        ));
    }
    let external = thesaurus_core::models::external(&config, &c.id)?;
    app.store
        .meeting_summary_authority(&run.id, &c.id, external)
        .await?
        .context("forbidden: model request lost authority")?;
    Ok((8192, 64000))
}
