use super::*;
impl Store {
    pub async fn activity_sessions(&self) -> Result<Vec<ActivitySession>> {
        self.purge_activity().await?;
        let rows = sqlx::query_scalar::<_, String>(
            "SELECT record FROM cap_sessions ORDER BY rowid DESC LIMIT 100",
        )
        .fetch_all(&self.pool)
        .await?;
        let mut result = vec![];
        for s in rows {
            let mut s: ActivitySession = serde_json::from_str(&s)?;
            let counts: (i64, i64) = sqlx::query_as(
                "SELECT COUNT(*),COALESCE(SUM(length(png)),0) FROM cap_frames WHERE session_id=?",
            )
            .bind(&s.id)
            .fetch_one(&self.pool)
            .await?;
            s.retained_frames = counts.0;
            s.retained_bytes = counts.1;
            result.push(s);
        }
        Ok(result)
    }
    pub async fn activity_session(&self, id: &str) -> Result<ActivitySession> {
        let s: String = sqlx::query_scalar("SELECT record FROM cap_sessions WHERE id=?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?
            .context("record not found")?;
        Ok(serde_json::from_str(&s)?)
    }
    pub async fn start_activity(
        &self,
        req: ActivityStartRequest,
    ) -> Result<(ActivitySession, bool)> {
        if !req.approve_capture {
            bail!("forbidden: explicitly approve selected-window activity capture");
        }
        if !(15..=3600).contains(&req.interval_seconds)
            || !(1..=168).contains(&req.retention_hours)
            || !(1..=100).contains(&req.max_frames)
        {
            bail!("invalid: interval15–3600seconds, retention1–168hours, max1–100frames");
        }
        let mut tx = self.pool.begin().await?;
        let (_, old) = operation(
            &mut tx,
            "activity-start",
            "capture",
            &req.idempotency_key,
            &req,
        )
        .await?;
        if let Some(old) = old {
            tx.commit().await?;
            return Ok((self.activity_session(&old).await?, false));
        }
        // A platform provider may block or need another pool connection. Release
        // the read snapshot before invoking it, then recheck replay and scope
        // under the writer lock before publishing the capture session.
        tx.commit().await?;
        let selected = serde_json::from_value::<Vec<CaptureWindow>>(
            self.native_invoke("native.screen_capture", "windows", serde_json::json!({}))
                .await?,
        )?
        .into_iter()
        .find(|w| {
            w.window_id == req.window.window_id
                && w.process_id == req.window.process_id
                && w.app == req.window.app
                && w.title == req.window.title
        })
        .context("invalid: refresh and select an available named window")?;
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let (digest, old) = operation(
            &mut tx,
            "activity-start",
            "capture",
            &req.idempotency_key,
            &req,
        )
        .await?;
        if let Some(old) = old {
            tx.commit().await?;
            return Ok((self.activity_session(&old).await?, false));
        }
        let scope = project_sources(&mut tx, &req.project_id).await?;
        permit(&mut tx, &scope, &Access::admin()).await?;
        let active: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM cap_sessions WHERE state='active'")
                .fetch_one(&mut *tx)
                .await?;
        if active >= 1 {
            bail!("conflict: pause the existing active capture before starting another");
        }
        let s = ActivitySession {
            id: uuid::Uuid::new_v4().to_string(),
            version: 1,
            project_id: req.project_id.clone(),
            window: selected,
            state: "active".into(),
            interval_seconds: req.interval_seconds,
            retention_hours: req.retention_hours,
            max_frames: req.max_frames,
            use_local_ocr: req.use_local_ocr,
            retained_frames: 0,
            retained_bytes: 0,
            error: None,
            created_at: now(),
            updated_at: now(),
        };
        sqlx::query(
            "INSERT INTO cap_sessions(id,version,state,project_id,record) VALUES(?,?,?,?,?)",
        )
        .bind(&s.id)
        .bind(s.version)
        .bind(&s.state)
        .bind(&s.project_id)
        .bind(serde_json::to_string(&s)?)
        .execute(&mut *tx)
        .await?;
        remember(&mut tx, &req.idempotency_key, &digest, &s.id).await?;
        event(&mut tx, "capture", &s.id).await?;
        tx.commit().await?;
        Ok((s, true))
    }
    pub async fn control_activity(
        &self,
        id: &str,
        req: ActivityControlRequest,
    ) -> Result<(ActivitySession, bool)> {
        if !["pause", "resume", "stop"].contains(&req.action.as_str()) {
            bail!("invalid capture control");
        }
        if req.action == "resume" && !req.approve_capture {
            bail!("forbidden: explicitly approve capture resume");
        }
        // Resolve the selected window outside a transaction. The final writer
        // rechecks both the idempotency key and expected session version.
        let available = if req.action == "resume" {
            let mut tx = self.pool.begin().await?;
            let (_, old) =
                operation(&mut tx, "activity-control", id, &req.idempotency_key, &req).await?;
            tx.commit().await?;
            if old.is_some() {
                return Ok((self.activity_session(id).await?, false));
            }
            Some(serde_json::from_value::<Vec<CaptureWindow>>(
                self.native_invoke("native.screen_capture", "windows", serde_json::json!({}))
                    .await?,
            )?)
        } else {
            None
        };
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let (digest, old) =
            operation(&mut tx, "activity-control", id, &req.idempotency_key, &req).await?;
        if old.is_some() {
            tx.commit().await?;
            return Ok((self.activity_session(id).await?, false));
        }
        let record: String = sqlx::query_scalar("SELECT record FROM cap_sessions WHERE id=?")
            .bind(id)
            .fetch_optional(&mut *tx)
            .await?
            .context("record not found")?;
        let mut s: ActivitySession = serde_json::from_str(&record)?;
        if s.version != req.expected_version {
            bail!("conflict: capture session changed");
        }
        if s.state == "stopped" {
            bail!("invalid: stopped sessions cannot resume");
        }
        if req.action == "resume" {
            if s.state != "paused" {
                bail!("invalid: capture is already active");
            }
            let active: i64 =
                sqlx::query_scalar("SELECT COUNT(*) FROM cap_sessions WHERE state='active'")
                    .fetch_one(&mut *tx)
                    .await?;
            if active > 0 {
                bail!("conflict: another capture is active");
            }
            if !available
                .as_ref()
                .context("selected windows unavailable")?
                .iter()
                .any(|w| {
                    w.window_id == s.window.window_id
                        && w.process_id == s.window.process_id
                        && w.title == s.window.title
                        && w.app == s.window.app
                })
            {
                bail!("invalid: selected window changed; create a new selection");
            }
        }
        s.state = match req.action.as_str() {
            "pause" => "paused",
            "resume" => "active",
            _ => "stopped",
        }
        .into();
        s.version += 1;
        s.updated_at = now();
        s.error = None;
        sqlx::query("UPDATE cap_sessions SET version=?,state=?,record=? WHERE id=?")
            .bind(s.version)
            .bind(&s.state)
            .bind(serde_json::to_string(&s)?)
            .bind(id)
            .execute(&mut *tx)
            .await?;
        remember(&mut tx, &req.idempotency_key, &digest, id).await?;
        event(&mut tx, "capture", id).await?;
        tx.commit().await?;
        let launch = s.state == "active";
        Ok((s, launch))
    }
    pub async fn activity_frames(&self, id: &str, query: &str) -> Result<Vec<ActivityFrame>> {
        self.activity_session(id).await?;
        self.purge_activity().await?;
        if query.len() > 1024 {
            bail!("invalid capture query");
        }
        let records: Vec<String> = if query.trim().is_empty() {
            sqlx::query_scalar("SELECT record FROM cap_frames WHERE session_id=? ORDER BY created_at DESC LIMIT 100").bind(id).fetch_all(&self.pool).await?
        } else {
            let terms = query
                .split_whitespace()
                .take(20)
                .map(|s| format!("\"{}\"", s.replace('"', "\"\"")))
                .collect::<Vec<_>>()
                .join(" OR ");
            sqlx::query_scalar("SELECT f.record FROM cap_frames_fts JOIN cap_frames f ON f.id=cap_frames_fts.frame_id WHERE cap_frames_fts MATCH ? AND f.session_id=? ORDER BY f.created_at DESC LIMIT 100").bind(terms).bind(id).fetch_all(&self.pool).await?
        };
        records
            .iter()
            .map(|s| Ok(serde_json::from_str(s)?))
            .collect()
    }
    pub async fn activity_image(&self, id: &str) -> Result<Vec<u8>> {
        self.purge_activity().await?;
        sqlx::query_scalar("SELECT png FROM cap_frames WHERE id=?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?
            .context("record not found")
    }
    pub async fn delete_activity_frame(&self, id: &str) -> Result<()> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        sqlx::query("DELETE FROM cap_frames_fts WHERE frame_id=?")
            .bind(id)
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM cap_frames WHERE id=?")
            .bind(id)
            .execute(&mut *tx)
            .await?;
        event(&mut tx, "capture", id).await?;
        tx.commit().await?;
        Ok(())
    }
    pub async fn purge_activity(&self) -> Result<()> {
        let rows = sqlx::query("SELECT s.id,s.record FROM cap_sessions s")
            .fetch_all(&self.pool)
            .await?;
        for row in rows {
            let s: ActivitySession = serde_json::from_str(row.get("record"))?;
            let cutoff = (chrono::Utc::now() - chrono::Duration::hours(s.retention_hours as i64))
                .to_rfc3339();
            let ids:Vec<String>=sqlx::query_scalar("SELECT id FROM cap_frames WHERE session_id=? AND (created_at<? OR id NOT IN (SELECT id FROM cap_frames WHERE session_id=? ORDER BY created_at DESC LIMIT ?))").bind(&s.id).bind(cutoff).bind(&s.id).bind(s.max_frames as i64).fetch_all(&self.pool).await?;
            for id in ids {
                self.delete_activity_frame(&id).await?;
            }
        }
        Ok(())
    }
    pub async fn pause_activity_error(&self, id: &str, generation: i64, error: &str) -> Result<()> {
        sqlx::query("UPDATE cap_sessions SET state='paused',version=version+1,record=json_set(record,'$.state','paused','$.version',version+1,'$.error',?,'$.updated_at',?) WHERE id=? AND version=? AND state='active'").bind(error.chars().take(2000).collect::<String>()).bind(now()).bind(id).bind(generation).execute(&self.pool).await?;
        sqlx::query("INSERT INTO events(kind,record_id) VALUES('capture',?)")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
    pub async fn activity_tick(&self, id: &str, generation: i64, dir: &Path) -> Result<bool> {
        self.purge_activity().await?;
        let s = self.activity_session(id).await?;
        if s.state != "active" || s.version != generation {
            return Ok(false);
        }
        let folder = dir.join("activity-tmp");
        private_dir(&folder)?;
        let path = folder.join(format!("{}.png", uuid::Uuid::new_v4()));
        let result = async {
            self.native_invoke(
                "native.screen_capture",
                "frame",
                serde_json::json!({"window":s.window,"path":path}),
            )
            .await?;
            let png = read_bounded(&path, 4_000_000).await?;
            let text = if s.use_local_ocr {
                self.native_invoke(
                    "native.ocr",
                    "recognize",
                    serde_json::json!({"path":path,"data_dir":dir}),
                )
                .await?["text"]
                    .as_str()
                    .context("invalid: OCR provider returned no text")?
                    .to_string()
            } else {
                String::new()
            };
            Ok::<_, anyhow::Error>((png, text))
        }
        .await;
        let _ = tokio::fs::remove_file(&path).await;
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let current: (String, i64) =
            sqlx::query_as("SELECT state,version FROM cap_sessions WHERE id=?")
                .bind(id)
                .fetch_one(&mut *tx)
                .await?;
        if current.0 != "active" || current.1 != generation {
            tx.commit().await?;
            return Ok(false);
        }
        match result {
            Ok((png, text)) => {
                let bytes: i64 =
                    sqlx::query_scalar("SELECT COALESCE(SUM(length(png)),0) FROM cap_frames")
                        .fetch_one(&mut *tx)
                        .await?;
                if bytes + png.len() as i64 > 200_000_000 {
                    bail!(
                        "invalid: retained capture storage reached200MB; delete frames before resuming"
                    );
                }
                let f = ActivityFrame {
                    id: uuid::Uuid::new_v4().to_string(),
                    session_id: id.into(),
                    project_id: s.project_id.clone(),
                    app: s.window.app.clone(),
                    window_title: s.window.title.clone(),
                    text,
                    png_bytes: png.len(),
                    created_at: now(),
                };
                sqlx::query(
                    "INSERT INTO cap_frames(id,session_id,record,png,created_at) VALUES(?,?,?,?,?)",
                )
                .bind(&f.id)
                .bind(id)
                .bind(serde_json::to_string(&f)?)
                .bind(png)
                .bind(&f.created_at)
                .execute(&mut *tx)
                .await?;
                sqlx::query("INSERT INTO cap_frames_fts(frame_id,text) VALUES(?,?)")
                    .bind(&f.id)
                    .bind(&f.text)
                    .execute(&mut *tx)
                    .await?;
                event(&mut tx, "capture", id).await?;
            }
            Err(e) => {
                let mut s = s;
                s.state = "paused".into();
                s.version += 1;
                s.error = Some(e.to_string());
                s.updated_at = now();
                sqlx::query("UPDATE cap_sessions SET state='paused',version=?,record=? WHERE id=?")
                    .bind(s.version)
                    .bind(serde_json::to_string(&s)?)
                    .bind(id)
                    .execute(&mut *tx)
                    .await?;
                event(&mut tx, "capture", id).await?;
                tx.commit().await?;
                return Ok(false);
            }
        }
        tx.commit().await?;
        self.purge_activity().await?;
        Ok(true)
    }
}
