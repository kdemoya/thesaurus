use super::*;
use sha2::{Digest, Sha256};
use std::{os::unix::fs::OpenOptionsExt, path::PathBuf};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

pub(super) struct Artifact {
    pub id: &'static str,
    pub name: &'static str,
    pub file: &'static str,
    pub repo: &'static str,
    pub revision: &'static str,
    pub bytes: u64,
    pub sha: &'static str,
    pub guidance: &'static str,
}
const REV: &str = "5359861c739e955e79d9a303bcbc70fb988958b1";
pub(super) const ARTIFACTS: &[Artifact] = &[
    Artifact {
        id: "normal",
        name: "Local · Normal / Large v3 Turbo Q5_0",
        file: "ggml-large-v3-turbo-q5_0.bin",
        repo: "ggerganov/whisper.cpp",
        revision: REV,
        bytes: 574041195,
        sha: "394221709cd5ad1f40c46e6031ca61bce88931e6e088c188294c6d5a55ffa7e2",
        guidance: "Recommended local preset · multilingual · 547 MiB download. More RAM is needed than the weight size; CPU latency depends on your machine.",
    },
    Artifact {
        id: "large",
        name: "Local · Large / Whisper Large v3",
        file: "ggml-large-v3.bin",
        repo: "ggerganov/whisper.cpp",
        revision: REV,
        bytes: 3095033483,
        sha: "64d182b440b98d5203c4f9bd541544d84c605196c4f7b845dfa11fb23594d1e2",
        guidance: "2.9 GiB download · substantially more memory and compute. May fall behind live capture; model size does not guarantee better accuracy.",
    },
    Artifact {
        id: "vad",
        name: "Silero speech detection v6.2.0",
        file: "ggml-silero-v6.2.0.bin",
        repo: "ggml-org/whisper-vad",
        revision: "9ffd54a1e1ee413ddf265af9913beaf518d1639b",
        bytes: 885098,
        sha: "2aa269b785eeb53a82983a20501ddf7c1d9c48e33ab63a41391ac6c9f7fb6987",
        guidance: "865 KiB · speech detection for the local presets; helps suppress silence and noise.",
    },
];
pub(super) fn artifact(id: &str) -> Result<&'static Artifact> {
    ARTIFACTS
        .iter()
        .find(|a| a.id == id)
        .context("invalid: unknown transcription model")
}
impl Store {
    pub(super) fn model_folder(&self) -> Result<PathBuf> {
        let folder = self.data_dir.join("transcription-models");
        private_dir(&folder)?;
        Ok(folder)
    }
    pub(super) fn model_file(&self, a: &Artifact) -> Result<PathBuf> {
        Ok(self
            .model_folder()?
            .join(format!("{}-{}", &a.sha[..16], a.file)))
    }
    pub(in crate::capture) async fn ready_model(&self, id: &str) -> Result<PathBuf> {
        let a = artifact(id)?;
        let state: Option<String> =
            sqlx::query_scalar("SELECT state FROM transcription_models WHERE id=?")
                .bind(id)
                .fetch_optional(&self.pool)
                .await?;
        let path = self.model_file(a)?;
        let valid = tokio::fs::symlink_metadata(&path)
            .await
            .is_ok_and(|m| m.is_file() && !m.file_type().is_symlink() && m.len() == a.bytes);
        if state.as_deref() != Some("ready") || !valid {
            bail!("invalid: download or repair the {id} transcription model in Audio setup");
        }
        Ok(path)
    }
    pub async fn transcription_catalog(&self) -> Result<TranscriptionCatalog> {
        let mut models = vec![];
        for a in ARTIFACTS {
            let row = sqlx::query(
                "SELECT state,downloaded_bytes,detail FROM transcription_models WHERE id=?",
            )
            .bind(a.id)
            .fetch_optional(&self.pool)
            .await?;
            let (mut state, downloaded_bytes, mut detail) = match row {
                Some(r) => (
                    r.get::<String, _>(0),
                    r.get::<i64, _>(1).max(0) as u64,
                    r.get::<String, _>(2),
                ),
                None => ("not_downloaded".into(), 0, String::new()),
            };
            if state == "ready" && self.ready_model(a.id).await.is_err() {
                state = "repair_needed".into();
                detail = "Model is missing or has changed; download it again".into();
            }
            models.push(TranscriptionModel {
                id: a.id.into(),
                name: a.name.into(),
                bytes: a.bytes,
                sha256: a.sha.into(),
                state,
                downloaded_bytes,
                detail: if detail.is_empty() {
                    a.guidance.into()
                } else {
                    format!("{} · {detail}", a.guidance)
                },
            });
        }
        let engine_available = super::super::recording_defaults::bundled_paths()
            .is_ok_and(|(p, _)| Path::new(&p).is_file());
        let used: i64 = sqlx::query_scalar("SELECT COALESCE(SUM(audio_seconds),0) FROM transcription_receipts WHERE substr(created_at,1,10)=?")
            .bind(Utc::now().format("%Y-%m-%d").to_string()).fetch_one(&self.pool).await?;
        Ok(TranscriptionCatalog {
            models,
            engine_available,
            hardware: format!(
                "{} · {} · {} logical CPUs · packaged engine uses CPU/Apple Accelerate; real-time capacity is not measured",
                std::env::consts::OS,
                std::env::consts::ARCH,
                std::thread::available_parallelism()
                    .map(|v| v.get())
                    .unwrap_or(1)
            ),
            remote_audio_seconds_today: used.max(0) as u64,
        })
    }
    /// Reserve before spawning so two browser tabs cannot write the same partial.
    pub async fn reserve_transcription_download(&self, id: &str) -> Result<bool> {
        let a = artifact(id)?;
        if self.ready_model(id).await.is_ok() {
            return Ok(false);
        }
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let state: Option<String> =
            sqlx::query_scalar("SELECT state FROM transcription_models WHERE id=?")
                .bind(id)
                .fetch_optional(&mut *tx)
                .await?;
        if state
            .as_deref()
            .is_some_and(|s| matches!(s, "downloading" | "cancelling"))
        {
            bail!("conflict: this model already has a download in progress");
        }
        let partial = self.model_file(a)?.with_extension("partial");
        let existing = tokio::fs::symlink_metadata(&partial)
            .await
            .ok()
            .filter(|m| m.is_file() && !m.file_type().is_symlink())
            .map(|m| m.len())
            .unwrap_or(0);
        if fs2::available_space(self.model_folder()?)?
            < a.bytes.saturating_sub(existing) + 64 * 1024 * 1024
        {
            bail!("invalid: insufficient free space for this model download");
        }
        sqlx::query("INSERT INTO transcription_models(id,state,downloaded_bytes,detail) VALUES(?,'downloading',?,'') ON CONFLICT(id) DO UPDATE SET state='downloading',downloaded_bytes=excluded.downloaded_bytes,detail=''")
            .bind(id).bind(existing as i64).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(true)
    }
    pub async fn cancel_transcription_download(&self, id: &str) -> Result<()> {
        artifact(id)?;
        sqlx::query(
            "UPDATE transcription_models SET state='cancelling' WHERE id=? AND state='downloading'",
        )
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
    async fn download_active(&self, id: &str) -> Result<bool> {
        Ok(
            sqlx::query_scalar::<_, String>("SELECT state FROM transcription_models WHERE id=?")
                .bind(id)
                .fetch_one(&self.pool)
                .await?
                == "downloading",
        )
    }
    pub async fn download_transcription_model(&self, id: &str) {
        let result = self.fetch_transcription_model(id).await;
        if let Err(e) = result {
            // Errors are locally generated; never include a signed redirect URL.
            let _ = sqlx::query("UPDATE transcription_models SET state=CASE WHEN state='cancelling' THEN 'cancelled' ELSE 'interrupted' END,detail=? WHERE id=? AND state!='ready'")
                .bind(e.to_string()).bind(id).execute(&self.pool).await;
        }
    }
    async fn fetch_transcription_model(&self, id: &str) -> Result<()> {
        let a = artifact(id)?;
        let dest = self.model_file(a)?;
        let partial = dest.with_extension("partial");
        let std_file = std::fs::OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(&partial)?;
        let mut file = tokio::fs::File::from_std(std_file);
        let mut start = file.metadata().await?.len();
        if start > a.bytes {
            file.set_len(0).await?;
            start = 0;
        }
        // Hash the existing prefix, then the new data, without loading model weights into RAM.
        let mut sha = Sha256::new();
        let mut buf = vec![0; 1024 * 1024];
        loop {
            let n = file.read(&mut buf).await?;
            if n == 0 {
                break;
            }
            sha.update(&buf[..n]);
        }
        let mut downloaded = start;
        let mut last_progress = tokio::time::Instant::now() - Duration::from_secs(1);
        if start < a.bytes {
            let url = format!(
                "https://huggingface.co/{}/resolve/{}/{}",
                a.repo, a.revision, a.file
            );
            let client = reqwest::Client::builder()
                .connect_timeout(Duration::from_secs(20))
                .timeout(Duration::from_secs(7200))
                .https_only(true)
                .build()?;
            let mut request = client.get(url);
            if start > 0 {
                request = request.header("Range", format!("bytes={start}-"));
            }
            let mut response = tokio::select! {
                r=request.send()=>r.map_err(|_|anyhow::anyhow!("Model download connection failed; explicitly resume to retry"))?,
                _=async { loop { tokio::time::sleep(Duration::from_millis(500)).await; if !self.download_active(id).await.unwrap_or(false) { break; } } }=>bail!("Download cancelled; the partial file will be checked on resume"),
            };
            if start > 0 && response.status() == reqwest::StatusCode::OK {
                file.set_len(0).await?;
                use tokio::io::AsyncSeekExt;
                file.rewind().await?;
                sha = Sha256::new();
                downloaded = 0;
            } else if start > 0 {
                let expected = format!("bytes {start}-{}/{}", a.bytes - 1, a.bytes);
                if response.status() != reqwest::StatusCode::PARTIAL_CONTENT
                    || response
                        .headers()
                        .get("content-range")
                        .and_then(|v| v.to_str().ok())
                        != Some(expected.as_str())
                {
                    bail!("Model server returned an invalid resume range");
                }
            } else if response.status() != reqwest::StatusCode::OK {
                bail!(
                    "Model download rejected (HTTP {})",
                    response.status().as_u16()
                );
            }
            loop {
                if last_progress.elapsed() >= Duration::from_millis(500) {
                    if !self.download_active(id).await? {
                        bail!("Download cancelled; partial file kept for resume");
                    }
                    sqlx::query("UPDATE transcription_models SET downloaded_bytes=? WHERE id=?")
                        .bind(downloaded as i64)
                        .bind(id)
                        .execute(&self.pool)
                        .await?;
                    last_progress = tokio::time::Instant::now();
                }
                let chunk = tokio::select! {
                    r=response.chunk()=>r.map_err(|_|anyhow::anyhow!("Model download interrupted; explicitly resume to retry"))?,
                    _=tokio::time::sleep(Duration::from_millis(500))=>continue,
                };
                let Some(chunk) = chunk else {
                    break;
                };
                downloaded += chunk.len() as u64;
                if downloaded > a.bytes {
                    bail!("Model download exceeded pinned size");
                }
                file.write_all(&chunk).await?;
                sha.update(&chunk);
            }
        }
        file.sync_all().await?;
        drop(file);
        if downloaded != a.bytes || format!("{:x}", sha.finalize()) != a.sha {
            let _ = tokio::fs::remove_file(&partial).await;
            bail!("Model integrity check failed; partial file removed. Download again to repair");
        }
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let state: String = sqlx::query_scalar("SELECT state FROM transcription_models WHERE id=?")
            .bind(id)
            .fetch_one(&mut *tx)
            .await?;
        if state != "downloading" {
            bail!("Download cancelled before publication; partial file kept");
        }
        tokio::fs::rename(&partial, &dest).await?;
        use std::os::unix::fs::PermissionsExt;
        tokio::fs::set_permissions(&dest, std::fs::Permissions::from_mode(0o400)).await?;
        sqlx::query(
            "UPDATE transcription_models SET state='ready',downloaded_bytes=?,detail='' WHERE id=?",
        )
        .bind(a.bytes as i64)
        .bind(id)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(())
    }
}
