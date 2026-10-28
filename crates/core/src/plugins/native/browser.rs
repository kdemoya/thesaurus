//! OS browser launch belongs to the selected service provider. URLs are local
//! workspace pairing links; neither a shell nor an arbitrary URL scheme is used.
use super::*;

pub(super) struct Browser;
pub(crate) fn local_url(value: &str) -> Result<reqwest::Url> {
    if value.len() > 4096 || value.chars().any(char::is_control) {
        bail!("invalid: local workspace URL required");
    }
    let url = reqwest::Url::parse(value).context("invalid: local workspace URL required")?;
    if url.scheme() != "http"
        || url.host_str() != Some("127.0.0.1")
        || url.port().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || !matches!(url.path(), "/" | "/index.html")
        || url.query().is_some()
        || !url.fragment().is_some_and(|fragment| {
            fragment.strip_prefix("pair=").is_some_and(|code| {
                (32..=256).contains(&code.len()) && code.bytes().all(|b| b.is_ascii_hexdigit())
            })
        })
    {
        bail!("invalid: local workspace pairing URL required");
    }
    Ok(url)
}
impl CapabilityProvider for Browser {
    fn manifest(&self) -> PluginManifest {
        manifest(
            "thesaurus.native.browser",
            "Local browser launcher",
            "native.browser",
            &["open"],
            PluginHost::Service,
        )
    }
    fn health(&self) -> Result<()> {
        if cfg!(any(
            target_os = "macos",
            target_os = "linux",
            target_os = "windows"
        )) {
            Ok(())
        } else {
            bail!("unsupported: browser launch is unavailable on this platform")
        }
    }
    fn invoke<'a>(&'a self, command: &'a str, input: Value) -> ProviderFuture<'a> {
        Box::pin(async move {
            if command != "open" {
                bail!("unknown browser command");
            }
            let url = local_url(
                input["url"]
                    .as_str()
                    .context("invalid: local workspace URL required")?,
            )?;
            #[cfg(target_os = "macos")]
            let mut process = {
                let mut p = tokio::process::Command::new("/usr/bin/open");
                p.arg("--");
                p
            };
            #[cfg(target_os = "linux")]
            let mut process = tokio::process::Command::new("xdg-open");
            #[cfg(target_os = "windows")]
            let mut process = {
                let mut p = tokio::process::Command::new("rundll32.exe");
                p.arg("url.dll,FileProtocolHandler");
                p
            };
            #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
            {
                let _ = url;
                bail!("unsupported: browser launch is unavailable on this platform");
            }
            #[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
            {
                process
                    .arg(url.as_str())
                    .stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .kill_on_drop(true);
                let status =
                    tokio::time::timeout(std::time::Duration::from_secs(5), process.status())
                        .await
                        .context("invalid: browser launch timed out; outcome is unknown")?
                        .context("invalid: browser launcher unavailable")?;
                if !status.success() {
                    bail!("invalid: OS browser launch failed");
                }
                Ok(
                    json!({"requested":true,"detail":"The OS accepted the browser-open request; page rendering is not confirmed"}),
                )
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn browser_accepts_only_bounded_local_pairing_links() {
        let code = "a".repeat(64);
        assert!(local_url(&format!("http://127.0.0.1:4620/#pair={code}")).is_ok());
        for url in [
            "file:///tmp/work",
            "javascript:alert(1)",
            "https://example.com/",
            "http://127.0.0.1/",
            "http://127.0.0.1:4620/#pair=short",
            "http://127.0.0.1:4620/api/v1/",
            "--args",
            "http://user@127.0.0.1:4620/",
        ] {
            assert!(local_url(url).is_err(), "{url}");
        }
        assert!(local_url(&format!("http://127.0.0.1:4620/?token=x#pair={code}")).is_err());
        assert!(local_url(&format!("http://127.0.0.1:4620/#pair={}\n", code)).is_err());
    }
}
