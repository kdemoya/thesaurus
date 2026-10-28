//! Credentials stay in the host. Plugin configuration and event payloads carry
//! references only; the public desktop broker cannot invoke this service provider.
use super::*;

const KEYCHAIN_SERVICE: &str = "dev.thesaurus.credentials";
pub(crate) fn reference(value: &str) -> Result<(&str, &str)> {
    let (scheme, name) = value
        .split_once(':')
        .context("invalid: credential reference must use env:NAME or keychain:NAME")?;
    if !matches!(scheme, "env" | "keychain")
        || name.is_empty()
        || name.len() > 120
        || !name.bytes().all(|b| {
            b.is_ascii_alphanumeric()
                || b == b'_'
                || (scheme == "keychain" && matches!(b, b'-' | b'.'))
        })
    {
        bail!("invalid: credential reference name");
    }
    Ok((scheme, name))
}
pub(super) struct Credentials;
impl CapabilityProvider for Credentials {
    fn manifest(&self) -> PluginManifest {
        manifest(
            "thesaurus.native.credentials",
            "Host credentials",
            "native.credentials",
            &["status", "read", "write", "delete"],
            PluginHost::Service,
        )
    }
    fn health(&self) -> Result<()> {
        Ok(())
    }
    fn invoke<'a>(&'a self, command: &'a str, input: Value) -> ProviderFuture<'a> {
        Box::pin(async move {
            if command == "status" {
                return Ok(
                    json!({"environment":true,"keychain":cfg!(target_os="macos"),"references_only":true,"keychain_service":KEYCHAIN_SERVICE}),
                );
            }
            let value = input["reference"]
                .as_str()
                .context("invalid: credential reference required")?;
            let (scheme, name) = reference(value)?;
            let name = name.to_owned();
            match (command, scheme) {
                ("read", "env") => {
                    let value = std::env::var(&name).map_err(|_| {
                        anyhow::anyhow!("invalid: configured environment credential unavailable")
                    })?;
                    valid_secret(&value)?;
                    Ok(json!({"secret":value}))
                }
                ("read" | "write" | "delete", "keychain") => {
                    #[cfg(target_os = "macos")]
                    {
                        let action = command.to_owned();
                        let secret = if command == "write" {
                            let s = input["secret"]
                                .as_str()
                                .context("invalid: secret required")?;
                            valid_secret(s)?;
                            Some(s.to_owned())
                        } else {
                            None
                        };
                        tokio::task::spawn_blocking(move || keychain(&action, &name, secret))
                            .await
                            .context("credential provider was interrupted")?
                    }
                    #[cfg(not(target_os = "macos"))]
                    {
                        bail!("unsupported: OS Keychain provider requires macOS");
                    }
                }
                ("write" | "delete", "env") => {
                    bail!("unsupported: manage environment credentials outside Thesaurus")
                }
                _ => bail!("unknown credential provider command"),
            }
        })
    }
}
fn valid_secret(value: &str) -> Result<()> {
    if value.is_empty() || value.len() > 16000 || value.contains(['\0', '\r', '\n']) {
        bail!("invalid: credential value must be a nonempty bounded single line");
    }
    Ok(())
}
#[cfg(target_os = "macos")]
fn keychain(command: &str, name: &str, secret: Option<String>) -> Result<Value> {
    use security_framework::passwords;
    match command {
        "read" => {
            let bytes = passwords::get_generic_password(KEYCHAIN_SERVICE, name).map_err(|_| {
                anyhow::anyhow!(
                    "invalid: configured Keychain credential unavailable or access denied"
                )
            })?;
            let secret = String::from_utf8(bytes)
                .map_err(|_| anyhow::anyhow!("invalid: Keychain credential is not UTF-8"))?;
            valid_secret(&secret)?;
            Ok(json!({"secret":secret}))
        }
        "write" => {
            passwords::set_generic_password(
                KEYCHAIN_SERVICE,
                name,
                secret
                    .as_deref()
                    .context("invalid: secret required")?
                    .as_bytes(),
            )
            .map_err(|_| anyhow::anyhow!("invalid: Keychain credential could not be saved"))?;
            Ok(json!({"saved":true}))
        }
        "delete" => {
            passwords::delete_generic_password(KEYCHAIN_SERVICE, name).map_err(|_| {
                anyhow::anyhow!("invalid: Keychain credential could not be removed")
            })?;
            Ok(json!({"removed":true}))
        }
        _ => bail!("unknown Keychain operation"),
    }
}

impl Store {
    /// Called by host adapters after their ordinary usage/source approval checks.
    /// Never serialize this resolved value in a plugin invocation or receipt.
    pub async fn resolve_credential(&self, reference: &str) -> Result<String> {
        let value = self
            .native_invoke("native.credentials", "read", json!({"reference":reference}))
            .await?;
        let secret = value["secret"]
            .as_str()
            .context("invalid: credential provider returned no value")?;
        valid_secret(secret)?;
        Ok(secret.to_owned())
    }
}
