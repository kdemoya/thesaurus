use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

pub fn data_dir() -> PathBuf {
    std::env::var_os("THESAURUS_DATA_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            directories::ProjectDirs::from("dev", "thesaurus", "Thesaurus")
                .expect("user data directory")
                .data_dir()
                .to_path_buf()
        })
}

pub fn private_dir(path: &Path) -> Result<()> {
    if path.exists() && fs::symlink_metadata(path)?.file_type().is_symlink() {
        bail!("application data directory must not be a symlink");
    }
    fs::create_dir_all(path)?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    Ok(())
}

pub fn private_write(path: &Path, body: &[u8]) -> Result<()> {
    let tmp = path.with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW)
        .open(&tmp)?;
    file.write_all(body)?;
    file.sync_all()?;
    fs::rename(tmp, path)?;
    Ok(())
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    #[serde(default)]
    pub connections: Vec<Connection>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Connection {
    pub id: String,
    pub kind: String,
    #[serde(default)]
    pub model: String,
    pub endpoint: Option<String>,
    pub command: Option<String>,
    #[serde(default)]
    pub args: Vec<String>,
    pub token_env: Option<String>,
    #[serde(default)]
    pub credential_ref: Option<String>,
    #[serde(default = "default_daily")]
    pub daily_limit: i64,
}
fn default_daily() -> i64 {
    20
}

impl Config {
    pub fn load(dir: &Path) -> Result<Self> {
        let file = dir.join("config.toml");
        if !file.exists() {
            return Ok(Self::default());
        }
        let config: Self =
            toml::from_str(&fs::read_to_string(file)?).context("invalid config.toml")?;
        config.validate()?;
        Ok(config)
    }
    pub fn validate(&self) -> Result<()> {
        let config = self;
        let mut ids = std::collections::HashSet::new();
        for c in &config.connections {
            if c.id == "local"
                || !c
                    .id
                    .chars()
                    .all(|x| x.is_ascii_alphanumeric() || x == '-' || x == '_')
                || c.id.is_empty()
                || !ids.insert(&c.id)
            {
                bail!("connection IDs must be unique; local is reserved");
            }
            if !["claude_cli", "codex_cli", "openai_compatible", "acp"].contains(&c.kind.as_str()) {
                bail!("unsupported connection kind");
            }
            if c.kind == "acp" {
                let command = c
                    .command
                    .as_deref()
                    .context("ACP connection requires command")?;
                if command.is_empty()
                    || !Path::new(command).is_absolute()
                    || command.len() > 4096
                    || c.args.len() > 32
                    || c.args.iter().any(|a| a.len() > 4096 || a.contains('\0'))
                {
                    bail!("ACP requires an absolute executable path and bounded arguments");
                }
                if c.token_env.is_some() {
                    bail!(
                        "ACP authentication belongs to the runtime; token_env is for API connections"
                    );
                }
            }
            if !(1..=1000).contains(&c.daily_limit) {
                bail!("daily_limit must be between 1 and 1000");
            }
            if c.kind == "openai_compatible" {
                let endpoint = reqwest::Url::parse(
                    c.endpoint
                        .as_deref()
                        .context("API connection requires endpoint")?,
                )?;
                if !(endpoint.scheme() == "https"
                    || (endpoint.scheme() == "http"
                        && [Some("localhost"), Some("127.0.0.1"), Some("[::1]")]
                            .contains(&endpoint.host_str())))
                    || endpoint.username() != ""
                    || endpoint.password().is_some()
                    || endpoint.query().is_some()
                    || endpoint.fragment().is_some()
                {
                    bail!("endpoint must use HTTPS or loopback HTTP without credentials or query");
                }
                if c.model.trim().is_empty() {
                    bail!("API connection requires model");
                }
            }
            if let Some(env) = &c.token_env {
                if env.is_empty() || !env.chars().all(|x| x.is_ascii_alphanumeric() || x == '_') {
                    bail!("token_env must name an environment variable");
                }
            }
            if let Some(reference) = &c.credential_ref {
                if c.kind != "openai_compatible" || c.token_env.is_some() {
                    bail!(
                        "credential_ref is for API connections and cannot be combined with token_env"
                    );
                }
                crate::plugins::native::credentials::reference(reference)?;
            }
        }
        Ok(())
    }
}
