use anyhow::{Context, Result, bail};
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::{path::Path, time::Duration};
use thesaurus_contracts::{Instance, PROTOCOL_VERSION};

#[derive(Clone, Debug)]
pub struct Client {
    pub http: reqwest::Client,
    instance: Instance,
    token: String,
}
impl Client {
    pub fn connect(dir: &Path, agent_token: Option<String>) -> Result<Self> {
        let instance: Instance = serde_json::from_slice(
            &std::fs::read(dir.join("instance.json"))
                .context("daemon is not running; run thesaurus serve")?,
        )?;
        if instance.protocol_version != PROTOCOL_VERSION {
            bail!("daemon protocol differs; restart the daemon")
        }
        let token = agent_token.unwrap_or_else(|| instance.token.clone());
        let http = reqwest::Client::builder()
            .unix_socket(instance.socket.clone())
            .timeout(Duration::from_secs(30))
            .build()?;
        Ok(Self {
            http,
            instance,
            token,
        })
    }
    pub fn request(&self, method: reqwest::Method, path: &str) -> reqwest::RequestBuilder {
        self.http
            .request(method, format!("http://localhost{path}"))
            .bearer_auth(&self.token)
    }
    pub fn api_url(&self) -> &str {
        &self.instance.api_url
    }
    pub async fn call<T: DeserializeOwned>(
        &self,
        method: reqwest::Method,
        path: &str,
        body: Option<Value>,
    ) -> Result<T> {
        let mut request = self.request(method, path);
        if let Some(body) = body {
            request = request.json(&body);
        }
        let response = request
            .send()
            .await
            .context("daemon is unavailable; restart thesaurus serve")?;
        let status = response.status();
        let value: Value = response.json().await.context("invalid daemon response")?;
        if !status.is_success() {
            bail!("{}", value["error"].as_str().unwrap_or("request failed"))
        }
        Ok(serde_json::from_value(value)?)
    }
}
