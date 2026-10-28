//! First-party platform providers. Domain state lives in Store; OS capture APIs
//! and native helper construction live behind these provider boundaries.
use super::registry::{CapabilityProvider, ProviderFuture, ProviderOpenFuture, ProviderRegistry};
use super::*;
use std::{process::Stdio, sync::Arc};
mod audio;
pub(crate) mod browser;
pub(crate) mod credentials;
mod screen;
mod transcription;

pub fn manifest(
    id: &str,
    name: &str,
    capability: &str,
    commands: &[&str],
    host: PluginHost,
) -> PluginManifest {
    PluginManifest {
        id: id.into(),
        name: name.into(),
        version: "1.0.0".into(),
        api_min: PLUGIN_API_VERSION,
        api_max: PLUGIN_API_VERSION,
        host,
        description: name.into(),
        dependencies: vec![],
        capabilities: vec![PluginCapability {
            id: capability.into(),
            commands: commands.iter().map(|s| (*s).into()).collect(),
            description: name.into(),
        }],
        hooks: vec![],
        configuration: vec![],
        contributions: vec![],
        requests: vec![],
        setup: vec![PluginSetupStep {
            id:"provider".into(),title:"Provider availability".into(),
            description:"Check that this provider is enabled and available on the connected host. Input permissions and credentials are checked separately.".into(),
            applies_to:vec![capability.into()],prerequisites:vec![PluginPrerequisite::Capability{capability:capability.into()}],check:false,optional:false,
            repair:Some(PluginSetupRepair::Provider{capability:capability.into()}),
        }],
    }
}
struct Audio;
impl CapabilityProvider for Audio {
    fn manifest(&self) -> PluginManifest {
        let mut m = manifest(
            "thesaurus.native.audio",
            "macOS audio inputs",
            "native.audio",
            &["inputs", "record", "setup_check"],
            PluginHost::Service,
        );
        for (id, title) in [
            ("microphone", "Microphone input and permission"),
            ("system-audio", "System audio support and permission"),
        ] {
            m.setup.push(PluginSetupStep{id:id.into(),title:title.into(),description:"An explicit passive check; does not start recording or request permission. Transcription setup is separate.".into(),applies_to:vec!["native.audio".into()],prerequisites:vec![PluginPrerequisite::Capability{capability:"native.audio".into()}],check:true,optional:true,repair:Some(PluginSetupRepair::Feature{feature:PluginSetupFeature::Audio})});
        }
        m
    }
    fn health(&self) -> Result<()> {
        audio::recorder_path().map(|_| ())
    }
    fn invoke<'a>(&'a self, command: &'a str, input: Value) -> ProviderFuture<'a> {
        Box::pin(async move {
            match command {
                "inputs" => Ok(serde_json::to_value(audio::inputs().await?)?),
                "setup_check" => {
                    let step = input["step_id"]
                        .as_str()
                        .context("invalid: audio setup step required")?;
                    let inputs = audio::inputs().await?;
                    let (ready, detail) = match step {
                        "microphone" => (
                            inputs.available
                                && inputs.microphone_permission == "granted"
                                && !inputs.microphones.is_empty(),
                            format!(
                                "Microphone permission: {}; {} inputs available. Choose an input in meeting audio settings; capture has not been tested",
                                inputs.microphone_permission,
                                inputs.microphones.len()
                            ),
                        ),
                        "system-audio" => {
                            let granted = screen::capture_permission_status()?;
                            (
                                inputs.available && inputs.system_audio_available && granted,
                                format!(
                                    "System audio supported: {}; screen/audio capture permission: {}. Recording and transcription have not been tested",
                                    inputs.system_audio_available,
                                    if granted { "granted" } else { "not granted" }
                                ),
                            )
                        }
                        _ => bail!("invalid: unknown audio setup step"),
                    };
                    Ok(serde_json::to_value(PluginReadinessResult {
                        step_id: step.into(),
                        state: if ready {
                            PluginSetupState::Ready
                        } else {
                            PluginSetupState::NeedsSetup
                        },
                        detail,
                    })?)
                }
                _ => bail!("unknown audio command"),
            }
        })
    }
    fn open<'a>(&'a self, command: &'a str, input: Value) -> ProviderOpenFuture<'a> {
        Box::pin(async move {
            if command != "record" {
                bail!("unknown audio resource command");
            }
            let s: LiveMeeting = serde_json::from_value(input["session"].clone())?;
            let folder = input["folder"].as_str().context("invalid audio folder")?;
            if !Path::new(folder).is_absolute() || !(10..=60).contains(&s.chunk_seconds) {
                bail!("invalid audio resource parameters");
            }
            Ok(tokio::process::Command::new(audio::recorder_path()?)
                .arg("record")
                .arg(folder)
                .arg(s.chunk_seconds.to_string())
                .arg(s.microphone_id.as_deref().unwrap_or("-"))
                .arg(if s.system_audio { "system" } else { "-" })
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .kill_on_drop(true)
                .spawn()?)
        })
    }
}
struct Screen;
impl CapabilityProvider for Screen {
    fn manifest(&self) -> PluginManifest {
        let mut m = manifest(
            "thesaurus.native.screen",
            "Selected-window capture",
            "native.screen_capture",
            &["windows", "permission", "frame", "setup_check"],
            PluginHost::Service,
        );
        m.setup.push(PluginSetupStep {
            id: "screen-permission".into(),
            title: "Selected-window capture permission".into(),
            description:
                "Read current OS authorization without capturing a window or requesting permission."
                    .into(),
            applies_to: vec!["native.screen_capture".into()],
            prerequisites: vec![PluginPrerequisite::Capability {
                capability: "native.screen_capture".into(),
            }],
            check: true,
            optional: false,
            repair: Some(PluginSetupRepair::Feature {
                feature: PluginSetupFeature::Capture,
            }),
        });
        m
    }
    fn health(&self) -> Result<()> {
        if cfg!(target_os = "macos") {
            Ok(())
        } else {
            bail!("unsupported: macOS screen provider is unavailable on this platform")
        }
    }
    fn invoke<'a>(&'a self, command: &'a str, input: Value) -> ProviderFuture<'a> {
        Box::pin(async move {
            match command {
                "windows" => Ok(serde_json::to_value(screen::capture_windows()?)?),
                "permission" => Ok(json!({"granted":screen::request_capture_permission()?})),
                "setup_check" => {
                    if input["step_id"] != "screen-permission" {
                        bail!("invalid: unknown screen setup step");
                    }
                    let granted = screen::capture_permission_status()?;
                    Ok(serde_json::to_value(PluginReadinessResult{step_id:"screen-permission".into(),state:if granted {PluginSetupState::Ready}else{PluginSetupState::NeedsSetup},detail:if granted {"Screen capture authorization is granted. No window was captured; select a window in Activity to use capture"}else{"Screen capture authorization is not granted. Review the permission setup in Activity"}.into()})?)
                }
                "frame" => {
                    let window: CaptureWindow = serde_json::from_value(input["window"].clone())?;
                    let path = Path::new(input["path"].as_str().context("invalid frame path")?);
                    let bytes = screen::capture_png(&window, path).await?;
                    Ok(json!({"bytes":bytes.len()}))
                }
                _ => bail!("unknown screen command"),
            }
        })
    }
}
struct Ocr;
impl CapabilityProvider for Ocr {
    fn manifest(&self) -> PluginManifest {
        manifest(
            "thesaurus.native.ocr",
            "Local Vision OCR",
            "native.ocr",
            &["recognize"],
            PluginHost::Service,
        )
    }
    fn health(&self) -> Result<()> {
        screen::ocr_helper_path().map(|_| ())
    }
    fn invoke<'a>(&'a self, command: &'a str, input: Value) -> ProviderFuture<'a> {
        Box::pin(async move {
            if command != "recognize" {
                bail!("unknown OCR command");
            }
            let path = Path::new(input["path"].as_str().context("invalid OCR path")?);
            let dir = Path::new(
                input["data_dir"]
                    .as_str()
                    .context("invalid OCR workspace")?,
            );
            Ok(json!({"text":screen::ocr(path,dir).await?}))
        })
    }
}
pub fn service_registry() -> Result<ProviderRegistry> {
    let registry = ProviderRegistry::default();
    registry.register(Arc::new(Audio))?;
    registry.register(Arc::new(transcription::Transcription))?;
    registry.register(Arc::new(Screen))?;
    registry.register(Arc::new(Ocr))?;
    registry.register(Arc::new(credentials::Credentials))?;
    registry.register(Arc::new(browser::Browser))?;
    registry.register(Arc::new(super::bragbook::Bragbook))?;
    Ok(registry)
}

impl Store {
    pub async fn seed_native_providers(&self) -> Result<()> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        for m in self.providers.manifests()? {
            let existing: Option<String> =
                sqlx::query_scalar("SELECT record FROM plugins WHERE id=?")
                    .bind(&m.id)
                    .fetch_optional(&mut *tx)
                    .await?;
            let p = if let Some(text) = existing {
                let mut p: PluginRecord = serde_json::from_str(&text)?;
                if !p.builtin {
                    bail!("invalid: a user plugin occupies a first-party provider identity");
                }
                if p.manifest != m {
                    p.version += 1;
                    p.updated_at = now();
                }
                p.manifest = m.clone();
                p
            } else {
                PluginRecord {
                    id: m.id.clone(),
                    version: 1,
                    manifest: m.clone(),
                    executable: None,
                    builtin: true,
                    enabled: m.id != super::bragbook::ID,
                    grant: Default::default(),
                    configuration: json!({}),
                    installed_at: now(),
                    updated_at: now(),
                }
            };
            save(&mut tx, &p).await?;
            self.providers.set_enabled(&p.id, p.enabled)?;
            for c in &m.capabilities {
                sqlx::query(
                    "INSERT OR IGNORE INTO plugin_selections(capability,plugin_id) VALUES(?,?)",
                )
                .bind(&c.id)
                .bind(&m.id)
                .execute(&mut *tx)
                .await?;
            }
        }
        tx.commit().await?;
        Ok(())
    }
    pub async fn native_provider(&self, capability: &str) -> Result<PluginRecord> {
        let text:Option<String>=sqlx::query_scalar("SELECT p.record FROM plugin_selections s JOIN plugins p ON p.id=s.plugin_id WHERE s.capability=? AND p.enabled=1")
            .bind(capability).fetch_optional(&self.pool).await?;
        let p: PluginRecord = serde_json::from_str(
            &text.context("unsupported: selected native capability is unavailable or disabled")?,
        )?;
        self.providers.set_enabled(&p.id, true)?;
        self.providers.select(capability, &p.id)?;
        Ok(p)
    }
    pub async fn native_invoke(
        &self,
        capability: &str,
        command: &str,
        input: Value,
    ) -> Result<Value> {
        self.native_provider(capability).await?;
        self.providers.invoke(capability, command, input).await
    }
    pub async fn native_open(
        &self,
        capability: &str,
        command: &str,
        input: Value,
    ) -> Result<super::registry::ProviderProcess> {
        self.native_provider(capability).await?;
        self.providers.open(capability, command, input).await
    }
}
