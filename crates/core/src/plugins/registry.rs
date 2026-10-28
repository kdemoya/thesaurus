//! Compiled capability providers use the same manifests as subprocess plugins.
//! This registry owns active-resource leases and never unloads a live provider.
use super::*;
use std::{
    collections::HashMap,
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex},
};

pub type ProviderFuture<'a> = Pin<Box<dyn Future<Output = Result<Value>> + Send + 'a>>;
pub type ProviderOpenFuture<'a> =
    Pin<Box<dyn Future<Output = Result<tokio::process::Child>> + Send + 'a>>;
pub trait CapabilityProvider: Send + Sync {
    fn manifest(&self) -> PluginManifest;
    fn invoke<'a>(&'a self, command: &'a str, input: Value) -> ProviderFuture<'a>;
    fn health(&self) -> Result<()>;
    fn open<'a>(&'a self, _command: &'a str, _input: Value) -> ProviderOpenFuture<'a> {
        Box::pin(async { bail!("unsupported: provider does not open streaming resources") })
    }
}
pub struct ProviderProcess {
    pub child: tokio::process::Child,
    _lease: ProviderLease,
}
struct Entry {
    provider: Arc<dyn CapabilityProvider>,
    manifest: PluginManifest,
    enabled: bool,
    leases: usize,
}
#[derive(Default)]
struct State {
    providers: HashMap<String, Entry>,
    selected: HashMap<String, String>,
}
#[derive(Clone, Default)]
pub struct ProviderRegistry {
    state: Arc<Mutex<State>>,
}
pub struct ProviderLease {
    id: String,
    provider: Arc<dyn CapabilityProvider>,
    state: Arc<Mutex<State>>,
}
impl ProviderLease {
    pub fn provider(&self) -> &dyn CapabilityProvider {
        self.provider.as_ref()
    }
    pub fn id(&self) -> &str {
        &self.id
    }
}
impl Drop for ProviderLease {
    fn drop(&mut self) {
        if let Ok(mut s) = self.state.lock() {
            if let Some(entry) = s.providers.get_mut(&self.id) {
                entry.leases = entry.leases.saturating_sub(1);
            }
        }
    }
}
impl ProviderRegistry {
    pub fn register(&self, provider: Arc<dyn CapabilityProvider>) -> Result<()> {
        let m = provider.manifest();
        validate_manifest(&m)?;
        let mut state = self
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("provider registry unavailable"))?;
        if state.providers.contains_key(&m.id) {
            bail!("conflict: provider already registered");
        }
        for c in &m.capabilities {
            state
                .selected
                .entry(c.id.clone())
                .or_insert_with(|| m.id.clone());
        }
        state.providers.insert(
            m.id.clone(),
            Entry {
                provider,
                manifest: m,
                enabled: true,
                leases: 0,
            },
        );
        Ok(())
    }
    pub fn set_enabled(&self, id: &str, enabled: bool) -> Result<()> {
        let mut s = self
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("provider registry unavailable"))?;
        let entry = s.providers.get(id).context("unknown provider")?;
        if entry.enabled == enabled {
            return Ok(());
        }
        if entry.leases > 0 {
            bail!("conflict: provider owns active resources");
        }
        if !enabled {
            if s.providers
                .values()
                .any(|e| e.enabled && e.manifest.dependencies.iter().any(|d| d == id))
            {
                bail!("conflict: enabled provider depends on this provider");
            }
        } else {
            for d in &entry.manifest.dependencies {
                if !s.providers.get(d).is_some_and(|p| p.enabled) {
                    bail!("invalid: provider dependency is unavailable");
                }
            }
        }
        s.providers.get_mut(id).unwrap().enabled = enabled;
        Ok(())
    }
    pub fn select(&self, capability: &str, id: &str) -> Result<()> {
        let mut s = self
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("provider registry unavailable"))?;
        let next = s.providers.get(id).context("unknown provider")?;
        if !next.enabled
            || !next
                .manifest
                .capabilities
                .iter()
                .any(|c| c.id == capability)
        {
            bail!("invalid: provider does not offer this capability");
        }
        if s.selected
            .get(capability)
            .is_some_and(|selected| selected == id)
        {
            return Ok(());
        }
        if s.selected
            .get(capability)
            .and_then(|id| s.providers.get(id))
            .is_some_and(|p| p.leases > 0)
        {
            bail!("conflict: selected provider owns active resources");
        }
        s.selected.insert(capability.into(), id.into());
        Ok(())
    }
    pub fn acquire(&self, capability: &str) -> Result<ProviderLease> {
        let mut s = self
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("provider registry unavailable"))?;
        let id = s
            .selected
            .get(capability)
            .context("unsupported: no provider for this capability")?
            .clone();
        let e = s.providers.get_mut(&id).context("unknown provider")?;
        if !e.enabled {
            bail!("unsupported: selected provider is disabled");
        }
        e.leases += 1;
        let lease = ProviderLease {
            id,
            provider: e.provider.clone(),
            state: self.state.clone(),
        };
        drop(s);
        // Provider code can inspect dependency health without holding our lock.
        // The acquired lease still prevents concurrent disable/replacement.
        lease.provider.health()?;
        Ok(lease)
    }
    pub async fn invoke(&self, capability: &str, command: &str, input: Value) -> Result<Value> {
        let lease = self.acquire(capability)?;
        let m = lease.provider.manifest();
        if !m
            .capabilities
            .iter()
            .any(|c| c.id == capability && c.commands.iter().any(|s| s == command))
        {
            bail!("unknown provider command");
        }
        lease.provider.invoke(command, input).await
    }
    pub async fn open(
        &self,
        capability: &str,
        command: &str,
        input: Value,
    ) -> Result<ProviderProcess> {
        let lease = self.acquire(capability)?;
        if !lease
            .provider
            .manifest()
            .capabilities
            .iter()
            .any(|c| c.id == capability && c.commands.iter().any(|s| s == command))
        {
            bail!("unknown provider resource command");
        }
        let child = lease.provider.open(command, input).await?;
        Ok(ProviderProcess {
            child,
            _lease: lease,
        })
    }
    pub fn capabilities(&self) -> Result<Vec<PluginCapabilityStatus>> {
        let snapshots = {
            let s = self
                .state
                .lock()
                .map_err(|_| anyhow::anyhow!("provider registry unavailable"))?;
            s.selected
                .iter()
                .filter_map(|(cap, id)| {
                    s.providers
                        .get(id)
                        .map(|p| (cap.clone(), id.clone(), p.provider.clone(), p.enabled))
                })
                .collect::<Vec<_>>()
        };
        Ok(snapshots
            .into_iter()
            .map(|(cap, id, provider, enabled)| {
                let health = if enabled {
                    provider.health()
                } else {
                    Err(anyhow::anyhow!("Provider is disabled"))
                };
                PluginCapabilityStatus {
                    capability: cap,
                    provider_id: id,
                    host: provider.manifest().host,
                    available: health.is_ok(),
                    detail: health
                        .err()
                        .map(|e| e.to_string())
                        .unwrap_or_else(|| "Provider ready".into()),
                    selection_version: 0,
                }
            })
            .collect())
    }
    pub fn manifests(&self) -> Result<Vec<PluginManifest>> {
        let s = self
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("provider registry unavailable"))?;
        let mut manifests = s
            .providers
            .values()
            .map(|p| p.manifest.clone())
            .collect::<Vec<_>>();
        manifests.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(manifests)
    }
    pub fn provider_health(&self, id: &str) -> Result<()> {
        let provider = self
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("provider registry unavailable"))?
            .providers
            .get(id)
            .context("unsupported: provider host is not connected")?
            .provider
            .clone();
        provider.health()
    }
}
