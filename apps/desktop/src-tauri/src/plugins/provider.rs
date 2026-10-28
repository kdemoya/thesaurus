use super::*;

#[derive(Clone, Copy)]
pub(super) enum Kind {
    Windows,
    Tray,
    Shortcuts,
    Notifications,
}
pub(super) const KINDS: [Kind; 4] = [
    Kind::Windows,
    Kind::Tray,
    Kind::Shortcuts,
    Kind::Notifications,
];
impl Kind {
    pub fn id(self) -> &'static str {
        match self {
            Self::Windows => "thesaurus.native.windows",
            Self::Tray => "thesaurus.native.tray",
            Self::Shortcuts => "thesaurus.native.shortcuts",
            Self::Notifications => "thesaurus.native.notifications",
        }
    }
    pub fn manifest(self) -> PluginManifest {
        let (name, cap, commands): (&str, &str, &[&str]) = match self {
            Self::Windows => (
                "Workspace and companion windows",
                "native.windows",
                &["workspace", "companion", "hide"],
            ),
            Self::Tray => (
                "Tray and meeting popover",
                "native.tray",
                &["show", "dismiss", "toggle", "height", "status"],
            ),
            Self::Shortcuts => ("Meeting menu shortcuts", "native.shortcuts", &["status"]),
            Self::Notifications => (
                "OS notifications",
                "native.notifications",
                &[
                    "status",
                    "request_permission",
                    "show",
                    "dismiss",
                    "setup_check",
                ],
            ),
        };
        let mut m = thesaurus_core::plugins::native::manifest(
            self.id(),
            name,
            cap,
            commands,
            PluginHost::Desktop,
        );
        m.dependencies = match self {
            Self::Windows | Self::Notifications => vec![],
            Self::Tray => vec![Kind::Windows.id().into()],
            Self::Shortcuts => vec![Kind::Tray.id().into()],
        };
        if matches!(self, Self::Notifications) {
            m.setup.push(PluginSetupStep {
                id: "notification-permission".into(),
                title: "Notification authorization".into(),
                description:
                    "Read OS authorization without requesting access or sending a notification."
                        .into(),
                applies_to: vec!["native.notifications".into()],
                prerequisites: vec![PluginPrerequisite::Capability {
                    capability: "native.notifications".into(),
                }],
                check: true,
                optional: false,
                repair: Some(PluginSetupRepair::Feature {
                    feature: PluginSetupFeature::Notifications,
                }),
            });
        }
        m
    }
    pub async fn set_installed(self, app: &tauri::AppHandle, enabled: bool) -> Result<()> {
        on_main(app.clone(), move |app| {
            match (self, enabled) {
                (Self::Windows, true) => windows::install(app)?,
                (Self::Windows, false) => windows::teardown(app)?,
                (Self::Tray, true) => tray::install(app)?,
                (Self::Tray, false) => tray::teardown(app)?,
                (Self::Shortcuts, true) => shortcuts::install(app)?,
                (Self::Shortcuts, false) => shortcuts::teardown(app)?,
                (Self::Notifications, true) => notifications::supported()?,
                (Self::Notifications, false) => {}
            }
            Ok(())
        })
        .await
    }
}
pub(super) struct Native {
    pub app: tauri::AppHandle,
    pub kind: Kind,
}
impl CapabilityProvider for Native {
    fn manifest(&self) -> PluginManifest {
        self.kind.manifest()
    }
    fn health(&self) -> Result<()> {
        if matches!(self.kind, Kind::Notifications) {
            notifications::supported().map_err(anyhow::Error::msg)?;
        }
        Ok(())
    }
    fn invoke<'a>(&'a self, command: &'a str, input: Value) -> ProviderFuture<'a> {
        let app = self.app.clone();
        let command = command.to_owned();
        let kind = self.kind;
        if matches!(kind, Kind::Notifications) {
            return Box::pin(async move { notifications::invoke(app, &command, input).await });
        }
        Box::pin(async move {
            on_main(app, move |app| {
                match kind {
                    Kind::Windows => match command.as_str() {
                        "workspace" => windows::show(app, "workspace")?,
                        "companion" => windows::show(
                            app,
                            input["mode"].as_str().ok_or("Companion mode required")?,
                        )?,
                        "hide" => windows::hide(app)?,
                        _ => return Err("Unknown window command".into()),
                    },
                    Kind::Tray => match command.as_str() {
                        "status" => return popover::status(app),
                        "show" => popover::show(app)?,
                        "dismiss" => popover::dismiss(app),
                        "toggle" => popover::toggle_current(app)?,
                        "height" => popover::set_height(
                            app,
                            input["height"].as_f64().ok_or("Popover height required")?,
                        )?,
                        _ => return Err("Unknown tray command".into()),
                    },
                    Kind::Shortcuts => {
                        if command != "status" {
                            return Err("Unknown shortcut command".into());
                        }
                    }
                    Kind::Notifications => unreachable!(),
                }
                Ok(json!({"ok":true}))
            })
            .await
        })
    }
}
