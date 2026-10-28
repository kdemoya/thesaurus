use super::*;
use rmcp::transport::streamable_http_client::{
    StreamableHttpClientTransport, StreamableHttpClientTransportConfig,
};
use rmcp::{
    RoleClient, ServiceExt,
    model::{CallToolRequestParams, ReadResourceRequestParams},
    service::RunningService,
};
use std::process::Stdio;
use tokio::io::AsyncReadExt;
struct Client {
    service: RunningService<RoleClient, ()>,
    _child: Option<tokio::process::Child>,
}
pub(super) fn arguments(value: &serde_json::Value, query: &str) -> serde_json::Value {
    match value {
        serde_json::Value::String(s) => serde_json::Value::String(s.replace("{query}", query)),
        serde_json::Value::Array(a) => {
            serde_json::Value::Array(a.iter().map(|v| arguments(v, query)).collect())
        }
        serde_json::Value::Object(o) => serde_json::Value::Object(
            o.iter()
                .map(|(k, v)| (k.clone(), arguments(v, query)))
                .collect(),
        ),
        v => v.clone(),
    }
}
async fn connect(store: &Store, c: &UpstreamConnection) -> Result<Client> {
    if c.transport == "stdio" {
        let command = c.command.as_ref().context("invalid upstream command")?;
        let mut child = tokio::process::Command::new(command)
            .args(&c.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .context("invalid: approved MCP command unavailable")?;
        let input = child.stdin.take().context("MCP stdin unavailable")?;
        let output = child
            .stdout
            .take()
            .context("MCP stdout unavailable")?
            .take(2_000_000);
        let service =
            ().serve((output, input))
                .await
                .context("invalid: upstream MCP handshake failed")?;
        Ok(Client {
            service,
            _child: Some(child),
        })
    } else {
        let mut config = StreamableHttpClientTransportConfig::with_uri(
            c.endpoint.clone().context("invalid endpoint")?,
        );
        config.max_sse_event_size = 2_000_000;
        config.max_concurrent_requests = 1;
        config.reinit_on_expired_session = false;
        if let Some(env) = &c.token_env {
            let token = store.resolve_credential(&format!("env:{env}")).await?;
            if token.len() > 16000 || token.contains(['\r', '\n']) {
                bail!("invalid upstream token format");
            }
            config = config.auth_header(token);
        }
        let http = mcp_http::Client::builder()
            .redirect(mcp_http::redirect::Policy::none())
            .timeout(std::time::Duration::from_secs(25))
            .build()?;
        let transport = StreamableHttpClientTransport::with_client(
            super::bounded_http::BoundedHttp(http),
            config,
        );
        let service =
            ().serve(transport)
                .await
                .context("invalid: upstream HTTP MCP handshake failed")?;
        Ok(Client {
            service,
            _child: None,
        })
    }
}
pub(super) async fn fetch(
    store: &Store,
    c: &UpstreamConnection,
    query: &str,
) -> Result<(String, Vec<String>)> {
    let client = connect(store, c).await?;
    let value =
        if let Some(uri) = &c.resource_uri {
            serde_json::to_value(
                client
                    .service
                    .read_resource(ReadResourceRequestParams::new(uri))
                    .await
                    .context("invalid: configured upstream resource read failed")?,
            )?
        } else {
            let name = c
                .read_tool
                .clone()
                .context("invalid: no approved upstream read tool")?;
            let listed = client
                .service
                .list_tools(None)
                .await
                .context("invalid: upstream tool discovery failed")?;
            let tool = listed
                .tools
                .iter()
                .find(|t| t.name.as_ref() == name)
                .context("invalid: configured read tool absent from first upstream tool page")?;
            if tool.annotations.as_ref().is_some_and(|a| {
                a.read_only_hint == Some(false) || a.destructive_hint == Some(true)
            }) {
                bail!("forbidden: configured upstream tool advertises writes/destructive behavior");
            }
            let args = arguments(&c.tool_arguments, query)
                .as_object()
                .cloned()
                .context("invalid: approved tool arguments must be JSON object")?;
            let result = client
                .service
                .call_tool(CallToolRequestParams::new(name).with_arguments(args))
                .await
                .context("invalid: approved upstream read tool failed")?;
            if result.is_error == Some(true) {
                bail!("invalid: upstream read tool returned an error");
            }
            serde_json::to_value(result)?
        };
    let _ = client.service.cancel().await;
    if serde_json::to_vec(&value)?.len() > 200000 {
        bail!("invalid: upstream result exceeds200KB");
    }
    let mut texts = vec![];
    let mut omitted = 0;
    for block in value["contents"]
        .as_array()
        .or_else(|| value["content"].as_array())
        .into_iter()
        .flatten()
    {
        if let Some(t) = block["text"].as_str() {
            texts.push(t.to_string());
        } else {
            omitted += 1;
        }
    }
    if texts.is_empty() {
        if let Some(v) = value.get("structuredContent") {
            texts.push(serde_json::to_string_pretty(v)?);
        }
    }
    let body = texts.join("\n\n");
    validate_text(&body, 200000, "upstream text")?;
    let mut notes = vec![];
    if omitted > 0 {
        notes.push(format!(
            "{omitted} non-text upstream content blocks omitted"
        ));
    }
    Ok((body, notes))
}
