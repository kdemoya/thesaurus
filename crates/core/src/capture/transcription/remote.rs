use super::*;

/// Bound response bodies and discard provider error bodies (may echo audio or credentials).
async fn response_json(response: reqwest::Response) -> Result<Value> {
    if !response.status().is_success() {
        bail!(
            "Transcription provider rejected the request (HTTP {}); check credentials, account limits and model configuration",
            response.status().as_u16()
        );
    }
    let mut response = response;
    let mut bytes = vec![];
    while let Some(chunk) = response.chunk().await.map_err(|_| {
        anyhow::anyhow!("Transcription response interrupted; audio will not be resent")
    })? {
        if bytes.len() + chunk.len() > 1_000_000 {
            bail!("Transcription response exceeded size bound");
        }
        bytes.extend_from_slice(&chunk);
    }
    serde_json::from_slice(&bytes)
        .context("invalid: transcription provider returned malformed JSON")
}
async fn send(request: reqwest::RequestBuilder) -> Result<reqwest::Response> {
    request.send().await.map_err(|_| {
        anyhow::anyhow!(
            "Transcription delivery failed or is uncertain; audio will not be resent automatically"
        )
    })
}
fn remote_id(v: &Value) -> Result<String> {
    let id = v["id"]
        .as_str()
        .context("invalid: provider returned no resource identifier")?;
    uuid::Uuid::parse_str(id).context("invalid: provider resource identifier")?;
    Ok(id.into())
}
// The recorder emits PCM WAV. Reject malformed headers rather than undercounting audio.
fn audio_seconds(bytes: &[u8]) -> Result<u64> {
    if bytes.len() < 44 || &bytes[..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        bail!("invalid: expected PCM WAV recording chunk");
    }
    let mut at = 12usize;
    let mut rate = None;
    let mut length = 0u64;
    while at + 8 <= bytes.len() {
        let n = u32::from_le_bytes(bytes[at + 4..at + 8].try_into()?) as usize;
        let start = at + 8;
        let end = start.checked_add(n).context("invalid WAV size")?;
        if end > bytes.len() {
            bail!("invalid: incomplete WAV chunk");
        }
        if &bytes[at..at + 4] == b"fmt " {
            if n < 16 {
                bail!("invalid WAV format");
            }
            let format = u16::from_le_bytes(bytes[start..start + 2].try_into()?);
            let channels = u16::from_le_bytes(bytes[start + 2..start + 4].try_into()?);
            let hz = u32::from_le_bytes(bytes[start + 4..start + 8].try_into()?);
            let bits = u16::from_le_bytes(bytes[start + 14..start + 16].try_into()?);
            if format != 1 || channels != 1 || hz != 16000 || bits != 16 {
                bail!("invalid: expected 16 kHz mono PCM16 audio");
            }
            rate = Some(32000u64);
        }
        if &bytes[at..at + 4] == b"data" {
            length += n as u64;
        }
        at = end + (n % 2);
    }
    let seconds = length.div_ceil(rate.context("invalid: WAV has no format")?);
    if seconds == 0 || seconds > 61 {
        bail!("invalid: cloud audio chunk duration outside recording bounds");
    }
    Ok(seconds)
}
impl Store {
    async fn update_transcription_receipt(&self, r: &TranscriptionReceipt) -> Result<()> {
        sqlx::query("UPDATE transcription_receipts SET record=? WHERE chunk_id=?")
            .bind(serde_json::to_string(r)?)
            .bind(&r.chunk_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
    async fn reserve_audio_delivery(
        &self,
        session: &LiveMeeting,
        selection: &TranscriptionSelection,
        r: &TranscriptionReceipt,
        credential_fingerprint: &str,
    ) -> Result<()> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let active: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM transcription_audio_grants WHERE route=? AND active=1)",
        )
        .bind(route_key(selection)?)
        .fetch_one(&mut *tx)
        .await?;
        if !active {
            bail!("forbidden: audio upload permission revoked; queued audio was not sent");
        }
        let used:i64=sqlx::query_scalar("SELECT COALESCE(SUM(audio_seconds),0) FROM transcription_receipts WHERE substr(created_at,1,10)=?")
            .bind(Utc::now().format("%Y-%m-%d").to_string()).fetch_one(&mut *tx).await?;
        if used as u64 + r.audio_seconds > selection.daily_audio_minutes as u64 * 60 {
            bail!(
                "forbidden: daily cloud audio limit reached; recording paused. Both microphone and system tracks count toward this limit"
            );
        }
        // A duplicate identifier is never resubmitted, even after a crash or failed response.
        sqlx::query("INSERT INTO transcription_receipts(chunk_id,session_id,audio_seconds,created_at,record) VALUES(?,?,?,?,?)")
            .bind(&r.chunk_id).bind(&session.id).bind(r.audio_seconds as i64).bind(&r.created_at).bind(serde_json::to_string(r)?)
            .execute(&mut *tx).await.context("conflict: audio delivery already reserved; inspect its receipt instead of retrying")?;
        sqlx::query(
            "INSERT INTO transcription_delivery_credentials(chunk_id,fingerprint) VALUES(?,?)",
        )
        .bind(&r.chunk_id)
        .bind(credential_fingerprint)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(())
    }
    async fn cleanup_soniox(
        &self,
        client: &reqwest::Client,
        endpoint: &str,
        secret: &str,
        receipt: &mut TranscriptionReceipt,
    ) -> Result<()> {
        let mut ok = true;
        // Delete the job first; this also stops pending processing before removing the file.
        for (collection, id) in [
            ("transcriptions", receipt.provider_request_id.as_ref()),
            ("files", receipt.provider_file_id.as_ref()),
        ] {
            if let Some(id) = id {
                let response = client
                    .delete(format!("{endpoint}/{collection}/{id}"))
                    .bearer_auth(secret)
                    .send()
                    .await;
                ok &= response.is_ok_and(|r| {
                    r.status().is_success() || r.status() == reqwest::StatusCode::NOT_FOUND
                });
            }
        }
        receipt.cleanup_pending = !ok;
        self.update_transcription_receipt(receipt).await?;
        Ok(())
    }
    /// Deletion only: permitted after revocation, never submits or resubmits audio.
    pub async fn cleanup_transcription_delivery(
        &self,
        session_id: &str,
        chunk_id: &str,
    ) -> Result<()> {
        let session = self.live_meeting(session_id).await?;
        let selection = session
            .transcription
            .context("invalid: no provider selection")?;
        if selection.provider != TranscriptionProvider::Soniox {
            bail!("invalid: only Soniox has retained job/file resources");
        }
        let text: String = sqlx::query_scalar(
            "SELECT record FROM transcription_receipts WHERE session_id=? AND chunk_id=?",
        )
        .bind(session_id)
        .bind(chunk_id)
        .fetch_one(&self.pool)
        .await?;
        let mut receipt: TranscriptionReceipt = serde_json::from_str(&text)?;
        if !receipt.cleanup_pending {
            return Ok(());
        }
        if receipt.status == "sending" {
            bail!("conflict: wait for processing to finish before cleanup");
        }
        let secret = self.resolve_credential(&selection.credential_ref).await?;
        let fingerprint: Option<String> = sqlx::query_scalar(
            "SELECT fingerprint FROM transcription_delivery_credentials WHERE chunk_id=?",
        )
        .bind(chunk_id)
        .fetch_optional(&self.pool)
        .await?;
        if fingerprint.as_deref() != Some(crate::store::hash(&secret).as_str()) {
            bail!(
                "invalid: provider cleanup requires the original API key. Restore that credential or delete the file/job from the original provider account"
            );
        }
        let client = client()?;
        self.cleanup_soniox(&client, &selection.endpoint, &secret, &mut receipt)
            .await?;
        if receipt.cleanup_pending {
            bail!(
                "Provider deletion is still pending; inspect your Soniox account or retry cleanup"
            );
        }
        Ok(())
    }
    pub(super) async fn transcribe_remote(
        &self,
        session: &LiveMeeting,
        selection: &TranscriptionSelection,
        chunk_id: &str,
        track: &str,
        offset_ms: u64,
        path: &Path,
    ) -> Result<(String, Value, Option<String>)> {
        self.require_audio_grant(selection).await?;
        let secret = self.resolve_credential(&selection.credential_ref).await?;
        let bytes = read_bounded(path, 4_000_000).await?;
        let seconds = audio_seconds(&bytes)?;
        let mut receipt = TranscriptionReceipt {
            chunk_id: chunk_id.into(),
            track: track.into(),
            offset_ms,
            audio_seconds: seconds,
            provider: selection.provider,
            model: selection.model.clone(),
            destination: selection.endpoint.clone(),
            status: "sending".into(),
            provider_request_id: None,
            provider_file_id: None,
            cleanup_pending: false,
            error: None,
            created_at: now(),
        };
        self.reserve_audio_delivery(session, selection, &receipt, &crate::store::hash(&secret))
            .await?;
        let client = client()?;
        let result=tokio::time::timeout(Duration::from_secs(240),async {
            if selection.provider!=TranscriptionProvider::Soniox {
                let mut request=client.post(&selection.endpoint).header("Authorization",format!("Token {secret}"))
                    .header("Content-Type","audio/wav").query(&[("model",selection.model.as_str()),("smart_format","true")]);
                if selection.provider==TranscriptionProvider::Deepgram {request=request.query(&[("mip_opt_out","true")]);}
                if selection.language=="auto" {request=request.query(&[("detect_language","true")]);}
                else {request=request.query(&[("language",selection.language.as_str())]);}
                let v=response_json(send(request.body(bytes)).await?).await?;
                receipt.provider_request_id=v["metadata"]["request_id"].as_str().map(str::to_owned);
                let alternative=&v["results"]["channels"][0]["alternatives"][0];
                let text=alternative["transcript"].as_str().context("invalid: Deepgram-compatible response has no transcript")?.to_owned();
                return Ok((text,json!({"provider":selection.provider,"model":selection.model,"metadata":v["metadata"],"words":alternative["words"],"language":v["results"]["channels"][0]["detected_language"]}),None));
            }
            let form=reqwest::multipart::Form::new().part("file",reqwest::multipart::Part::bytes(bytes).file_name("audio.wav").mime_str("audio/wav")?);
            let v=response_json(send(client.post(format!("{}/files",selection.endpoint)).bearer_auth(&secret).multipart(form)).await?).await?;
            let file_id=remote_id(&v)?;
            receipt.provider_file_id=Some(file_id.clone()); receipt.cleanup_pending=true;
            self.update_transcription_receipt(&receipt).await?;
            // Revocation between the upload and job creation prevents another billable operation.
            self.require_audio_grant(selection).await?;
            let mut body=json!({"model":selection.model,"file_id":file_id,"enable_language_identification":true,"client_reference_id":chunk_id});
            if selection.language!="auto" {body["language_hints"]=json!([selection.language]);body["language_hints_strict"]=json!(true);}
            let v=response_json(send(client.post(format!("{}/transcriptions",selection.endpoint)).bearer_auth(&secret).json(&body)).await?).await?;
            let job=remote_id(&v)?;
            receipt.provider_request_id=Some(job.clone()); self.update_transcription_receipt(&receipt).await?;
            loop {
                self.require_audio_grant(selection).await?;
                let v=response_json(send(client.get(format!("{}/transcriptions/{job}",selection.endpoint)).bearer_auth(&secret)).await?).await?;
                match v["status"].as_str() {
                    Some("completed")=>break,
                    Some("error")=>bail!("Soniox could not transcribe this chunk; inspect its request identifier in your provider account"),
                    Some("queued"|"processing")=>tokio::time::sleep(Duration::from_secs(2)).await,
                    _=>bail!("invalid: Soniox returned an unknown transcription state"),
                }
            }
            let v=response_json(send(client.get(format!("{}/transcriptions/{job}/transcript",selection.endpoint)).bearer_auth(&secret)).await?).await?;
            let text=v["text"].as_str().context("invalid: Soniox response has no transcript")?.to_owned();
            Ok((text,json!({"provider":selection.provider,"model":selection.model,"tokens":v["tokens"]}),None))
        }).await.unwrap_or_else(|_|Err(anyhow::anyhow!("Transcription timed out; delivery may have been billed and audio will not be resent")));
        receipt.status = if result.is_ok() {
            "completed"
        } else {
            "uncertain"
        }
        .into();
        receipt.error = result.as_ref().err().map(|e| {
            if selection.provider==TranscriptionProvider::Soniox && receipt.provider_file_id.is_none() {
                format!("{e}. An upload may exist without a returned identifier; inspect and delete it in your Soniox account")
            }else{e.to_string()}
        });
        self.update_transcription_receipt(&receipt).await?;
        if selection.provider == TranscriptionProvider::Soniox {
            self.cleanup_soniox(&client, &selection.endpoint, &secret, &mut receipt)
                .await?;
        }
        result
    }
}
fn client() -> Result<reqwest::Client> {
    Ok(reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .retry(reqwest::retry::never())
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(60))
        .build()?)
}
