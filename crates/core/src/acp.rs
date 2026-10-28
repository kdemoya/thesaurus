//! A bounded ACP v1 stdio client. No filesystem or terminal capabilities are exposed.
use crate::config::Connection;
use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use std::{collections::HashMap, path::Path, process::Stdio, time::Duration};
use thesaurus_contracts::{PermissionOption, RuntimePermission, RuntimeProbe, RuntimeProvenance};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    process::{Child, ChildStdin, ChildStdout, Command},
    sync::mpsc,
    time::{Instant, timeout},
};

const FRAME_LIMIT: u64 = 1_048_576;
const WIRE_LIMIT: usize = 16_777_216;
pub enum Control {
    Cancel,
    Permission {
        id: String,
        option_id: Option<String>,
    },
}
pub enum Update {
    Provenance(RuntimeProvenance),
    Session(String),
    Text(String),
    Progress(String),
    Permission(RuntimePermission),
}
pub struct Outcome {
    pub text: String,
    pub stop_reason: String,
}
struct Wire {
    child: Child,
    pid: i32,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    next_id: u64,
    session: Option<String>,
    pending: HashMap<String, (Value, Vec<PermissionOption>)>,
    controls: mpsc::Receiver<Control>,
    updates: mpsc::Sender<Update>,
    cancelled: bool,
    cancel_deadline: Option<Instant>,
    output: String,
    bytes_read: usize,
}
impl Drop for Wire {
    fn drop(&mut self) {
        unsafe {
            libc::kill(-self.pid, libc::SIGKILL);
        }
        let _ = self.child.start_kill();
    }
}
impl Wire {
    fn spawn(
        c: &Connection,
        cwd: &Path,
        controls: mpsc::Receiver<Control>,
        updates: mpsc::Sender<Update>,
    ) -> Result<Self> {
        let mut command = Command::new(c.command.as_deref().context("ACP command is missing")?);
        command
            .args(&c.args)
            .current_dir(cwd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .process_group(0);
        let mut child = command.spawn().context("ACP executable could not start")?;
        let pid = child.id().context("ACP process has no PID")? as i32;
        let stdin = child.stdin.take().context("ACP stdin unavailable")?;
        let stdout = BufReader::new(child.stdout.take().context("ACP stdout unavailable")?);
        Ok(Self {
            child,
            pid,
            stdin,
            stdout,
            next_id: 0,
            session: None,
            pending: HashMap::new(),
            controls,
            updates,
            cancelled: false,
            cancel_deadline: None,
            output: String::new(),
            bytes_read: 0,
        })
    }
    async fn send(&mut self, value: Value) -> Result<()> {
        let mut bytes = serde_json::to_vec(&value)?;
        bytes.push(b'\n');
        timeout(Duration::from_secs(5), self.stdin.write_all(&bytes))
            .await
            .context("ACP write timed out")??;
        Ok(())
    }
    async fn notify(&mut self, method: &str, params: Value) -> Result<()> {
        self.send(json!({"jsonrpc":"2.0","method":method,"params":params}))
            .await
    }
    async fn cancel(&mut self) -> Result<()> {
        if self.cancelled {
            return Ok(());
        }
        self.cancelled = true;
        self.cancel_deadline = Some(Instant::now() + Duration::from_secs(3));
        if let Some(id) = self.session.clone() {
            self.notify("session/cancel", json!({"sessionId":id}))
                .await?;
        }
        let pending = std::mem::take(&mut self.pending);
        for (_, (id, _)) in pending {
            self.send(
                json!({"jsonrpc":"2.0","id":id,"result":{"outcome":{"outcome":"cancelled"}}}),
            )
            .await?;
        }
        Ok(())
    }
    async fn request(&mut self, method: &str, params: Value, duration: Duration) -> Result<Value> {
        self.next_id += 1;
        let id = self.next_id;
        self.send(json!({"jsonrpc":"2.0","id":id,"method":method,"params":params}))
            .await?;
        let deadline = Instant::now() + duration;
        loop {
            let until = self.cancel_deadline.map_or(deadline, |d| d.min(deadline));
            tokio::select! {
                _ = tokio::time::sleep_until(until) => { if self.cancelled { bail!("cancelled"); } let _ = self.cancel().await; bail!("ACP operation timed out"); },
                control = self.controls.recv() => match control {
                    None => { let _ = self.cancel().await; bail!("cancelled"); },
                    Some(Control::Cancel) => { self.cancel().await?; if self.session.is_none() { bail!("cancelled"); } },
                    Some(Control::Permission{id,option_id}) => {
                        if let Some((rpc_id, options)) = self.pending.remove(&id) {
                            let outcome = match option_id.filter(|id| options.iter().any(|o| &o.id==id && matches!(o.kind.as_str(),"allow_once"|"reject_once"))) {
                                Some(option) if !self.cancelled => json!({"outcome":"selected","optionId":option}),
                                _ => json!({"outcome":"cancelled"}),
                            };
                            self.send(json!({"jsonrpc":"2.0","id":rpc_id,"result":{"outcome":outcome}})).await?;
                        }
                    }
                },
                message = read_frame(&mut self.stdout) => {
                    let bytes = message?; self.bytes_read += bytes.len();
                    if self.bytes_read > WIRE_LIMIT { bail!("ACP wire limit exceeded"); }
                    let msg: Value = serde_json::from_slice(&bytes).context("ACP sent invalid JSON")?;
                    if msg["jsonrpc"] != "2.0" { bail!("ACP sent an invalid envelope"); }
                    if let Some(method_name) = msg["method"].as_str() {
                        self.incoming(method_name, &msg).await?;
                    } else if msg["id"] == id {
                        if let Some(error) = msg.get("error") { bail!("ACP rejected {method} (code {}); check runtime authentication and configuration", error["code"].as_i64().unwrap_or(-32603)); }
                        return msg.get("result").cloned().context("ACP response has no result");
                    }
                }
            }
        }
    }
    async fn incoming(&mut self, method: &str, msg: &Value) -> Result<()> {
        let p = &msg["params"];
        if method == "session/update"
            && self
                .session
                .as_ref()
                .is_some_and(|id| p["sessionId"].as_str() == Some(id))
        {
            let u = &p["update"];
            match u["sessionUpdate"].as_str().unwrap_or_default() {
                "agent_message_chunk" if u["content"]["type"] == "text" => {
                    let text = u["content"]["text"].as_str().unwrap_or_default();
                    if self.output.len() + text.len() > 65_536 {
                        bail!("ACP output limit exceeded");
                    }
                    self.output.push_str(text);
                    self.updates
                        .send(Update::Text(text.into()))
                        .await
                        .context("runtime observer closed")?;
                }
                "tool_call" | "tool_call_update" => {
                    self.updates
                        .send(Update::Progress(format!(
                            "Tool {}",
                            match u["status"].as_str() {
                                Some("completed") => "completed",
                                Some("failed") => "failed",
                                Some("in_progress") => "running",
                                _ => "pending",
                            }
                        )))
                        .await
                        .context("runtime observer closed")?;
                }
                "plan" => {
                    self.updates
                        .send(Update::Progress("Agent updated its plan".into()))
                        .await
                        .context("runtime observer closed")?;
                }
                _ => {}
            }
        } else if method == "session/request_permission" && msg.get("id").is_some() {
            if self.cancelled
                || self
                    .session
                    .as_ref()
                    .is_none_or(|id| p["sessionId"].as_str() != Some(id))
                || self.pending.len() >= 8
            {
                self.send(json!({"jsonrpc":"2.0","id":msg["id"],"result":{"outcome":{"outcome":"cancelled"}}})).await?;
            } else {
                let options: Vec<PermissionOption> = p["options"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .take(20)
                    .filter_map(|o| {
                        let kind = o["kind"].as_str()?;
                        if !matches!(kind, "allow_once" | "reject_once") {
                            return None;
                        }
                        Some(PermissionOption {
                            id: o["optionId"].as_str()?.chars().take(200).collect(),
                            label: o["name"]
                                .as_str()
                                .unwrap_or(kind)
                                .chars()
                                .take(200)
                                .collect(),
                            kind: kind.into(),
                        })
                    })
                    .collect();
                let permission = RuntimePermission {
                    id: uuid::Uuid::new_v4().to_string(),
                    title: p["toolCall"]["title"]
                        .as_str()
                        .unwrap_or("Agent tool request")
                        .chars()
                        .take(300)
                        .collect(),
                    kind: p["toolCall"]["kind"]
                        .as_str()
                        .unwrap_or("other")
                        .chars()
                        .take(40)
                        .collect(),
                    input: serde_json::to_string(&p["toolCall"]["rawInput"])?
                        .chars()
                        .take(8000)
                        .collect(),
                    options: options.clone(),
                    decision: None,
                };
                self.pending
                    .insert(permission.id.clone(), (msg["id"].clone(), options));
                self.updates
                    .send(Update::Permission(permission))
                    .await
                    .context("runtime observer closed")?;
            }
        } else if let Some(id) = msg.get("id") {
            self.send(json!({"jsonrpc":"2.0","id":id,"error":{"code":-32601,"message":"This client does not expose filesystem, terminal, or extension methods"}})).await?;
        }
        Ok(())
    }
    async fn initialize(&mut self) -> Result<RuntimeProbe> {
        let r = self.request("initialize",json!({"protocolVersion":1,"clientCapabilities":{"fs":{"readTextFile":false,"writeTextFile":false},"terminal":false},"clientInfo":{"name":"thesaurus","version":"0.1.0"}}),Duration::from_secs(30)).await?;
        if r["protocolVersion"] != 1 {
            bail!("ACP agent does not support protocol v1");
        }
        Ok(RuntimeProbe {
            protocol_version: 1,
            agent_name: r["agentInfo"]["name"]
                .as_str()
                .filter(|s| !s.trim().is_empty())
                .unwrap_or("unknown")
                .chars()
                .take(100)
                .collect(),
            agent_version: r["agentInfo"]["version"]
                .as_str()
                .filter(|s| !s.trim().is_empty())
                .unwrap_or("unknown")
                .chars()
                .take(100)
                .collect(),
            auth_methods: r["authMethods"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|a| a["id"].as_str().map(|s| s.chars().take(100).collect()))
                .collect(),
            load_session: r["agentCapabilities"]["loadSession"] == true,
            close_session: r["agentCapabilities"]["sessionCapabilities"]
                .get("close")
                .is_some(),
            mcp_stdio: true,
            cancellation: true,
        })
    }
}
async fn read_frame(reader: &mut BufReader<ChildStdout>) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    let n = reader
        .take(FRAME_LIMIT + 1)
        .read_until(b'\n', &mut bytes)
        .await?;
    if n == 0 {
        bail!("ACP process disconnected");
    }
    if bytes.len() as u64 > FRAME_LIMIT || !bytes.ends_with(b"\n") {
        bail!("ACP frame exceeded its limit or was incomplete");
    }
    Ok(bytes)
}
pub async fn probe(c: &Connection, cwd: &Path) -> Result<RuntimeProbe> {
    let (_control, rx) = mpsc::channel(1);
    let (tx, _updates) = mpsc::channel(1);
    Wire::spawn(c, cwd, rx, tx)?.initialize().await
}
pub async fn run(
    c: &Connection,
    cwd: &Path,
    mcp: Option<Value>,
    prompt: &str,
    controls: mpsc::Receiver<Control>,
    updates: mpsc::Sender<Update>,
) -> Result<Outcome> {
    let mut wire = Wire::spawn(c, cwd, controls, updates)?;
    let capabilities = wire.initialize().await?;
    let r = wire
        .request(
            "session/new",
            json!({"cwd":cwd,"mcpServers":mcp.into_iter().collect::<Vec<_>>()}),
            Duration::from_secs(45),
        )
        .await?;
    let id = r["sessionId"]
        .as_str()
        .filter(|s| s.len() <= 1024)
        .context("ACP session ID unavailable")?
        .to_string();
    wire.session = Some(id.clone());
    wire.updates.send(Update::Session(id.clone())).await?;
    if !c.model.is_empty() {
        let options = r["configOptions"]
            .as_array()
            .context("ACP agent does not expose model configuration")?;
        let model = options
            .iter()
            .find(|o| o["category"] == "model")
            .context("ACP agent does not expose model configuration")?;
        wire.request(
            "session/set_config_option",
            json!({"sessionId":id,"configId":model["id"],"value":c.model}),
            Duration::from_secs(10),
        )
        .await?;
    }
    let reported_model = r["configOptions"]
        .as_array()
        .and_then(|options| options.iter().find(|o| o["category"] == "model"))
        .and_then(|o| o["currentValue"].as_str())
        .map(|s| s.chars().take(200).collect::<String>())
        .filter(|s| !s.trim().is_empty());
    let (model, model_source) = if !c.model.is_empty() {
        (
            c.model.clone(),
            "configured model acknowledged by ACP; identity unverified".to_string(),
        )
    } else if let Some(model) = reported_model {
        (model, "agent-reported default".to_string())
    } else {
        (
            "unknown (ACP model unreported)".to_string(),
            "unreported by ACP".to_string(),
        )
    };
    wire.updates
        .send(Update::Provenance(RuntimeProvenance {
            agent_name: capabilities.agent_name.clone(),
            agent_version: capabilities.agent_version.clone(),
            model,
            model_source,
            actual_prompt_hash: crate::store::hash(prompt),
        }))
        .await?;
    let r = wire
        .request(
            "session/prompt",
            json!({"sessionId":id,"prompt":[{"type":"text","text":prompt}]}),
            Duration::from_secs(180),
        )
        .await?;
    let reason = if wire.cancelled {
        "cancelled"
    } else {
        r["stopReason"]
            .as_str()
            .context("ACP stop reason missing")?
    }
    .to_string();
    if capabilities.close_session {
        let _ = wire
            .request(
                "session/close",
                json!({"sessionId":id}),
                Duration::from_secs(2),
            )
            .await;
    }
    Ok(Outcome {
        text: wire.output.clone(),
        stop_reason: reason,
    })
}
