//! Cross-process checks: the real daemon and two independent MCP client processes.
use anyhow::{Context, Result};
use serde_json::{Value, json};
use std::{path::Path, process::Stdio, time::Duration};
use thesaurus_client::Client;
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    process::{Child, Command},
};

async fn start(dir: &Path) -> Result<Child> {
    let mut command = Command::new(env!("CARGO_BIN_EXE_thesaurus"));
    command
        .args(["--data-dir", dir.to_str().unwrap(), "serve", "--port", "0"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    let child = command.spawn()?;
    for _ in 0..100 {
        if let Ok(client) = Client::connect(dir, None) {
            if client
                .call::<Value>(reqwest::Method::GET, "/health", None)
                .await
                .is_ok()
            {
                return Ok(child);
            }
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    anyhow::bail!("daemon did not become ready")
}
async fn stop(child: &mut Child) -> Result<()> {
    if let Some(pid) = child.id() {
        unsafe {
            libc::kill(pid as i32, libc::SIGINT);
        }
    }
    tokio::time::timeout(Duration::from_secs(5), child.wait()).await??;
    Ok(())
}

struct Mcp {
    child: Child,
    input: tokio::process::ChildStdin,
    output: tokio::io::Lines<BufReader<tokio::process::ChildStdout>>,
    id: u64,
}
impl Mcp {
    async fn start(dir: &Path, grant: &Path) -> Result<Self> {
        let mut child = Command::new(env!("CARGO_BIN_EXE_thesaurus"))
            .args([
                "--data-dir",
                dir.to_str().unwrap(),
                "mcp",
                "serve",
                "--grant-file",
                grant.to_str().unwrap(),
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()?;
        let input = child.stdin.take().unwrap();
        let output = BufReader::new(child.stdout.take().unwrap()).lines();
        let mut mcp = Self {
            child,
            input,
            output,
            id: 0,
        };
        mcp.call("initialize",json!({"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"integration-test","version":"1"}})).await?;
        mcp.input
            .write_all(b"{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}\n")
            .await?;
        Ok(mcp)
    }
    async fn call(&mut self, method: &str, params: Value) -> Result<Value> {
        self.id += 1;
        let id = self.id;
        let request = json!({"jsonrpc":"2.0","id":id,"method":method,"params":params});
        self.input
            .write_all(format!("{request}\n").as_bytes())
            .await?;
        loop {
            let line = tokio::time::timeout(Duration::from_secs(10), self.output.next_line())
                .await??
                .context("MCP process closed")?;
            let v: Value = serde_json::from_str(&line)?;
            if v["id"] == id {
                if let Some(e) = v.get("error") {
                    anyhow::bail!("MCP error: {e}")
                }
                return Ok(v["result"].clone());
            }
        }
    }
    async fn tool(&mut self, name: &str, args: Value) -> Result<Value> {
        let v = self
            .call("tools/call", json!({"name":name,"arguments":args}))
            .await?;
        Ok(serde_json::from_str(
            v["content"][0]["text"]
                .as_str()
                .context("missing tool content")?,
        )?)
    }
}

#[tokio::test]
async fn work_survives_restart_and_two_agents_share_only_authorized_memory() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let mut daemon = start(dir.path()).await?;
    let client = Client::connect(dir.path(), None)?;
    client
        .call::<Value>(reqwest::Method::POST, "/api/v1/demo", Some(json!({})))
        .await?;
    let mut snapshot = Value::Null;
    for _ in 0..100 {
        snapshot = client
            .call(reqwest::Method::GET, "/api/v1/snapshot", None)
            .await?;
        if snapshot["jobs"]
            .as_array()
            .unwrap()
            .iter()
            .all(|j| j["status"] == "completed")
        {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert_eq!(snapshot["documents"].as_array().unwrap().len(), 3);
    assert!(snapshot["tasks"].as_array().unwrap().len() >= 4);
    let task = snapshot["tasks"][0].clone();
    let task_id = task["id"].as_str().unwrap().to_string();
    client
        .call::<Value>(
            reqwest::Method::PATCH,
            &format!("/api/v1/tasks/{task_id}"),
            Some(json!({"status":"accepted","expected_version":task["version"]})),
        )
        .await?;
    let source = snapshot["sources"][0]["id"].as_str().unwrap().to_string();
    let mut grant_files = vec![];
    let mut grants = vec![];
    for name in ["agent-one", "agent-two"] {
        let grant:Value=client.call(reqwest::Method::POST,"/api/v1/grants",Some(json!({"name":name,"source_ids":[source],"allow_write":true,"allow_external":false}))).await?;
        let path = dir.path().join(format!("{name}.json"));
        thesaurus_core::private_write(&path, &serde_json::to_vec(&grant)?)?;
        grant_files.push(path);
        grants.push(grant);
    }
    let mut one = Mcp::start(dir.path(), &grant_files[0]).await?;
    let mut two = Mcp::start(dir.path(), &grant_files[1]).await?;
    let tools = one.call("tools/list", json!({})).await?;
    assert_eq!(tools["tools"].as_array().unwrap().len(), 22);
    let visible = one.tool("memory_sources", json!({})).await?;
    assert_eq!(visible.as_array().unwrap().len(), 1);
    let appended=one.tool("memory_append_observation",json!({"body":"There is an unresolved launch dependency.","source_ids":[source],"idempotency_key":"cross-agent-1"})).await?;
    let repeated=one.tool("memory_append_observation",json!({"body":"There is an unresolved launch dependency.","source_ids":[source],"idempotency_key":"cross-agent-1"})).await?;
    assert_eq!(appended["id"], repeated["id"]);
    let context = two
        .tool(
            "context_build",
            json!({"query":"launch","character_budget":8000}),
        )
        .await?;
    assert!(
        context["observations"]
            .as_array()
            .unwrap()
            .iter()
            .any(|o| o["id"] == appended["id"])
    );
    let denied = snapshot["documents"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["source_id"] != source)
        .unwrap();
    let hidden = two.tool("memory_get", json!({"id":denied["id"]})).await?;
    assert!(hidden.get("error").is_some());
    client
        .call::<Value>(
            reqwest::Method::POST,
            &format!(
                "/api/v1/grants/{}/revoke",
                grants[1]["grant"]["id"].as_str().unwrap()
            ),
            Some(json!({})),
        )
        .await?;
    assert!(
        two.tool("memory_sources", json!({}))
            .await?
            .get("error")
            .is_some()
    );
    one.child.kill().await?;
    two.child.kill().await?;
    stop(&mut daemon).await?;
    let mut daemon = start(dir.path()).await?;
    let client = Client::connect(dir.path(), None)?;
    let persisted: Value = client
        .call(reqwest::Method::GET, "/api/v1/snapshot", None)
        .await?;
    assert!(
        persisted["tasks"]
            .as_array()
            .unwrap()
            .iter()
            .any(|t| t["id"] == task_id && t["status"] == "accepted")
    );
    assert_eq!(persisted["observations"].as_array().unwrap().len(), 1);
    stop(&mut daemon).await?;
    Ok(())
}
