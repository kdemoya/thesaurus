//! First-party local transcription execution. Audio routing, grants, models and
//! transcript revisions stay host-owned; extensions cannot invoke this capability.
use super::*;
use crate::capture::command;

pub(super) struct Transcription;
impl CapabilityProvider for Transcription {
    fn manifest(&self) -> PluginManifest {
        manifest(
            "thesaurus.native.transcription",
            "Local transcription",
            "native.transcription",
            &["run"],
            PluginHost::Service,
        )
    }
    fn health(&self) -> Result<()> {
        Ok(())
    }
    fn invoke<'a>(&'a self, operation: &'a str, input: Value) -> ProviderFuture<'a> {
        Box::pin(async move {
            if operation != "run" {
                bail!("unknown local transcription command");
            }
            let executable = input["executable"]
                .as_str()
                .context("invalid: transcription executable required")?;
            let args: Vec<String> = serde_json::from_value(input["arguments"].clone())?;
            if !Path::new(executable).is_absolute()
                || args.len() > 40
                || args.iter().any(|a| a.len() > 4096 || a.contains('\0'))
            {
                bail!("invalid: bounded local transcription command required");
            }
            let mut c = tokio::process::Command::new(executable);
            c.args(args);
            command(&mut c, 300, 200000).await?;
            Ok(json!({"completed":true}))
        })
    }
}
impl Store {
    pub(crate) async fn run_local_transcriber(&self, c: &tokio::process::Command) -> Result<()> {
        let c = c.as_std();
        let executable = c
            .get_program()
            .to_str()
            .context("invalid transcriber executable")?;
        let args = c
            .get_args()
            .map(|v| {
                v.to_str()
                    .map(str::to_owned)
                    .context("invalid transcriber argument")
            })
            .collect::<Result<Vec<_>>>()?;
        self.native_invoke(
            "native.transcription",
            "run",
            json!({"executable":executable,"arguments":args}),
        )
        .await?;
        Ok(())
    }
}
