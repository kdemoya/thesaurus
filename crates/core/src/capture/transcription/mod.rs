//! Host-owned audio routing, immutable session choices and private model assets.
use super::*;
use chrono::Utc;
use serde_json::{Value, json};
use std::time::Duration;
mod catalog;
mod remote;

pub(super) fn route_key(selection: &TranscriptionSelection) -> Result<String> {
    Ok(crate::store::hash(&serde_json::to_string(selection)?))
}
pub(super) fn canonicalize(s: &mut TranscriptionSelection) -> Result<()> {
    if s.language != "auto"
        && (s.language.len() < 2
            || s.language.len() > 12
            || !s
                .language
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b == b'-'))
    {
        bail!("invalid: choose auto or a supported language code, such as en or es");
    }
    match s.provider {
        TranscriptionProvider::Deepgram => {
            s.endpoint = "https://api.deepgram.com/v1/listen".into();
            s.model = "nova-3".into();
        }
        TranscriptionProvider::Soniox => {
            s.endpoint = "https://api.soniox.com/v1".into();
            s.model = "stt-async-v5".into();
        }
        TranscriptionProvider::Company => {
            let u = reqwest::Url::parse(&s.endpoint)
                .context("invalid: enter the company Deepgram-compatible listen URL")?;
            let loopback = matches!(u.host_str(), Some("127.0.0.1" | "localhost" | "[::1]"));
            if !(u.scheme() == "https" || (u.scheme() == "http" && loopback))
                || u.host_str().is_none()
                || !u.username().is_empty()
                || u.password().is_some()
                || u.query().is_some()
                || u.fragment().is_some()
                || s.endpoint.len() > 2000
            {
                bail!(
                    "invalid: company endpoint requires HTTPS (or loopback HTTP), without credentials, query or fragment"
                );
            }
            validate_text(&s.model, 100, "company transcription model")?;
        }
        _ => {
            s.endpoint.clear();
            s.model.clear();
            s.credential_ref.clear();
            s.approve_audio_upload = false;
        }
    }
    if s.provider.is_remote() {
        if !s.approve_audio_upload {
            bail!("forbidden: authorize audio uploads to this transcription destination");
        }
        if !(1..=2880).contains(&s.daily_audio_minutes) {
            bail!("invalid: choose a daily audio-minute limit from 1 to 2880");
        }
        crate::plugins::native::credentials::reference(&s.credential_ref)?;
    }
    Ok(())
}
impl Store {
    pub(crate) async fn resolve_transcription_settings(
        &self,
        settings: &mut LiveRecordingSettings,
    ) -> Result<()> {
        if let Some(s) = &mut settings.transcription {
            canonicalize(s)?;
            match s.provider {
                TranscriptionProvider::LocalNormal | TranscriptionProvider::LocalLarge => {
                    settings.use_bundled_transcription = false;
                    settings.whisper_command = super::recording_defaults::bundled_paths()?.0;
                    settings.model_path = self
                        .ready_model(if s.provider == TranscriptionProvider::LocalNormal {
                            "normal"
                        } else {
                            "large"
                        })
                        .await?
                        .to_string_lossy()
                        .into_owned();
                }
                TranscriptionProvider::Bundled => {
                    settings.use_bundled_transcription = true;
                    super::recording_defaults::resolve_bundled(settings)?;
                }
                TranscriptionProvider::Custom => settings.use_bundled_transcription = false,
                _ => {
                    settings.use_bundled_transcription = false;
                    settings.whisper_command.clear();
                    settings.model_path.clear();
                }
            }
            let s = settings.transcription.as_ref().unwrap();
            if !s.provider.is_remote() && s.use_vad {
                self.ready_model("vad").await?;
            }
        } else {
            super::recording_defaults::resolve_bundled(settings)?;
        }
        Ok(())
    }
    pub(super) async fn transcription_assets(
        &self,
        settings: &LiveRecordingSettings,
    ) -> Result<Option<TranscriptionAssets>> {
        let Some(s) = &settings.transcription else {
            return Ok(None);
        };
        if s.provider.is_remote() {
            return Ok(None);
        }
        let model_sha256 = match s.provider {
            TranscriptionProvider::LocalNormal => Some(catalog::artifact("normal")?.sha.into()),
            TranscriptionProvider::LocalLarge => Some(catalog::artifact("large")?.sha.into()),
            TranscriptionProvider::Bundled => {
                Some("60ed5bc3dd14eea856493d334349b405782ddcaf0028d4b5df4088345fba2efe".into())
            }
            _ => None,
        };
        Ok(Some(TranscriptionAssets {
            model_sha256,
            vad_path: if s.use_vad {
                Some(
                    self.ready_model("vad")
                        .await?
                        .to_string_lossy()
                        .into_owned(),
                )
            } else {
                None
            },
            vad_sha256: if s.use_vad {
                Some(catalog::artifact("vad")?.sha.into())
            } else {
                None
            },
            engine_version: if s.provider == TranscriptionProvider::Custom {
                "custom / unknown"
            } else {
                "1.9.5"
            }
            .into(),
        }))
    }
    pub(super) async fn save_audio_grant(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
        settings: &LiveRecordingSettings,
    ) -> Result<()> {
        // A setup change applies to new recordings. An existing session keeps
        // its previously authorized route until the explicit revoke operation.
        if let Some(s) = &settings.transcription {
            if s.provider.is_remote() && s.approve_audio_upload {
                sqlx::query("INSERT INTO transcription_audio_grants(route,active) VALUES(?,1) ON CONFLICT(route) DO UPDATE SET active=1")
                    .bind(route_key(s)?).execute(&mut **tx).await?;
            }
        }
        Ok(())
    }
    pub async fn revoke_transcription_audio(&self) -> Result<()> {
        sqlx::query("UPDATE transcription_audio_grants SET active=0")
            .execute(&self.pool)
            .await?;
        Ok(())
    }
    pub(crate) async fn require_audio_grant(&self, s: &TranscriptionSelection) -> Result<()> {
        let active: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM transcription_audio_grants WHERE route=? AND active=1)",
        )
        .bind(route_key(s)?)
        .fetch_one(&self.pool)
        .await?;
        if !active {
            bail!(
                "forbidden: audio upload permission was revoked; save Audio setup to authorize this route again"
            );
        }
        Ok(())
    }
    pub async fn transcription_receipts(&self, id: &str) -> Result<Vec<TranscriptionReceipt>> {
        self.live_meeting(id).await?;
        let rows:Vec<String>=sqlx::query_scalar("SELECT record FROM transcription_receipts WHERE session_id=? ORDER BY created_at,chunk_id")
            .bind(id).fetch_all(&self.pool).await?;
        rows.iter().map(|r| Ok(serde_json::from_str(r)?)).collect()
    }
    pub async fn save_transcription_credential(
        &self,
        req: TranscriptionCredentialRequest,
    ) -> Result<String> {
        let name = match req.provider {
            TranscriptionProvider::Deepgram => "transcription.deepgram",
            TranscriptionProvider::Soniox => "transcription.soniox",
            TranscriptionProvider::Company => "transcription.company",
            _ => bail!("invalid: credentials are only used for remote transcription"),
        };
        let reference = if req.provider == TranscriptionProvider::Company {
            let mut selection = TranscriptionSelection {
                provider: req.provider,
                endpoint: req.endpoint.clone(),
                model: "credential-setup".into(),
                credential_ref: "keychain:pending".into(),
                approve_audio_upload: true,
                ..Default::default()
            };
            canonicalize(&mut selection)?;
            format!(
                "keychain:{name}.{}",
                &crate::store::hash(&selection.endpoint)[..16]
            )
        } else {
            format!("keychain:{name}")
        };
        self.native_invoke(
            "native.credentials",
            "write",
            json!({"reference":reference,"secret":req.secret}),
        )
        .await?;
        Ok(reference)
    }
    pub(super) async fn transcribe_selected(
        &self,
        s: &LiveMeeting,
        chunk_id: &str,
        track: &str,
        offset_ms: u64,
        path: &Path,
        output: &Path,
    ) -> Result<(String, Value, Option<String>)> {
        let Some(selection) = &s.transcription else {
            // Original sessions retain their old execution semantics.
            let (whisper, model) = if s.use_bundled_transcription {
                super::recording_defaults::bundled_paths()?
            } else {
                (s.whisper_command.clone(), s.model_path.clone())
            };
            let mut c = Command::new(whisper);
            if s.use_bundled_transcription {
                c.args(["--language", "auto"]);
            }
            c.args(["--model", &model, "--file"])
                .arg(path)
                .args(["--output-txt", "--output-file"])
                .arg(output)
                .args(["--no-timestamps", "--no-prints"]);
            self.run_local_transcriber(&c).await?;
            return Ok((
                String::from_utf8(read_bounded(&output.with_extension("txt"), 100000).await?)?,
                json!({"provider":"legacy_local","model":model}),
                None,
            ));
        };
        if selection.provider.is_remote() {
            return self
                .transcribe_remote(s, selection, chunk_id, track, offset_ms, path)
                .await;
        }
        let whisper = if selection.provider == TranscriptionProvider::Custom {
            s.whisper_command.clone()
        } else {
            super::recording_defaults::bundled_paths()?.0
        };
        // Model path is pinned into the session. Settings changes never replace it.
        if !Path::new(&s.model_path).is_file() {
            bail!("invalid: this session's pinned model is missing; restore it before resuming");
        }
        let mut c = Command::new(whisper);
        let language = if selection.language == "auto" {
            s.detected_language.as_deref().unwrap_or("auto")
        } else if selection.language == "multi" {
            "auto"
        } else {
            &selection.language
        };
        c.args(["--language", language, "--model", &s.model_path, "--file"])
            .arg(path)
            .args(["--output-json", "--output-file"])
            .arg(output)
            .args(["--no-prints"]);
        if selection.use_vad {
            c.arg("--vad").arg("--vad-model").arg(
                s.transcription_assets
                    .as_ref()
                    .and_then(|a| a.vad_path.clone())
                    .context("invalid: this session has no pinned speech detection model")?,
            );
        }
        let previous:Option<String>=sqlx::query_scalar("SELECT json_extract(record,'$.text') FROM live_meeting_chunks WHERE session_id=? AND json_extract(record,'$.track')=? ORDER BY rowid DESC LIMIT 1")
            .bind(&s.id).bind(track).fetch_optional(&self.pool).await?;
        if let Some(previous) = previous {
            let prompt = previous
                .chars()
                .rev()
                .take(500)
                .collect::<String>()
                .chars()
                .rev()
                .collect::<String>();
            c.args(["--prompt", &prompt]);
        }
        self.run_local_transcriber(&c).await?;
        let v: Value =
            serde_json::from_slice(&read_bounded(&output.with_extension("json"), 300000).await?)?;
        let segments = v["transcription"]
            .as_array()
            .context("invalid: local transcriber returned no segment list")?;
        let text = segments
            .iter()
            .filter_map(|x| x["text"].as_str())
            .collect::<Vec<_>>()
            .join(" ");
        let detected = v["result"]["language"]
            .as_str()
            .filter(|l| {
                l.len() >= 2
                    && l.len() <= 12
                    && l.bytes().all(|b| b.is_ascii_lowercase() || b == b'-')
            })
            .map(str::to_owned);
        let provenance = json!({"provider":selection.provider,"engine":"whisper.cpp","assets":s.transcription_assets,"model_path":s.model_path,"language":detected,"segments":segments,"vad":selection.use_vad});
        Ok((text, provenance, detected))
    }
}
