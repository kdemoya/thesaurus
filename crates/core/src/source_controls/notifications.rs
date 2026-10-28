use super::*;
use chrono::{DateTime, Utc};
const KINDS: &[&str] = &[
    "*",
    "action_candidate",
    "source_changed",
    "processing_failed",
    "waiting_due",
];
async fn mutes(tx: &mut Transaction<'_, Sqlite>) -> Result<Vec<NotificationMute>> {
    let mut result = Vec::new();
    for row in sqlx::query("SELECT record FROM source_control_mutes ORDER BY id")
        .fetch_all(&mut **tx)
        .await?
    {
        result.push(serde_json::from_str(row.get("record"))?);
    }
    Ok(result)
}
fn muted(item: &WorkNotification, mutes: &[NotificationMute]) -> bool {
    mutes.iter().any(|m| {
        m.active
            && (m.kind == "*" || m.kind == item.kind)
            && m.source_id
                .as_ref()
                .is_none_or(|id| item.source_ids.contains(id))
            && m.until.as_ref().is_none_or(|until| {
                DateTime::parse_from_rfc3339(until).is_ok_and(|time| time > Utc::now())
            })
    })
}
async fn emit(
    tx: &mut Transaction<'_, Sqlite>,
    mutes: &[NotificationMute],
    mut item: WorkNotification,
) -> Result<()> {
    if muted(&item, mutes) {
        return Ok(());
    }
    item.id = hash(&item.id);
    let id = item.id.clone();
    let inserted =
        sqlx::query("INSERT OR IGNORE INTO source_control_notifications(id,record) VALUES(?,?)")
            .bind(&id)
            .bind(serde_json::to_string(&item)?)
            .execute(&mut **tx)
            .await?
            .rows_affected();
    if inserted > 0 {
        event(tx, "notification", &id).await?;
    }
    Ok(())
}
fn evidence(e: Evidence) -> EvidenceReference {
    EvidenceReference {
        document_id: e.document_id,
        revision: e.revision,
        line: e.line,
        quote: (e.quote.len() <= 2000).then_some(e.quote),
    }
}
fn notification(
    id: String,
    kind: &str,
    title: String,
    why: String,
    sources: Vec<String>,
    record: String,
    evidence: Vec<EvidenceReference>,
    triggered: String,
) -> WorkNotification {
    WorkNotification {
        id,
        version: 1,
        kind: kind.into(),
        title,
        why_now: why,
        source_ids: sources,
        record_id: record,
        evidence,
        status: "unread".into(),
        triggered_at: triggered,
        created_at: now(),
    }
}
impl Store {
    /// Local reconciliation is safe to call after work events or on a daemon interval.
    /// Stable trigger identities ensure reconnecting never duplicates a suggestion.
    pub async fn reconcile_notifications(&self) -> Result<()> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let rules = mutes(&mut tx).await?;
        for row in sqlx::query("SELECT t.*,d.source_id FROM tasks t JOIN documents d ON d.id=t.document_id WHERE t.status='candidate' ORDER BY t.updated_at DESC LIMIT 500").fetch_all(&mut *tx).await?{
            let task=crate::store::task_row(&row);let source:String=row.get("source_id");let e=evidence(task.evidence);
            emit(&mut tx,&rules,notification(format!("task:{}:{}",task.id,task.version),"action_candidate",task.title,format!("A cited action candidate was extracted at {} and is awaiting your review. Its source line is the new evidence; accepting it remains your decision.",task.updated_at),vec![source],task.id,vec![e],task.updated_at)).await?;
        }
        for row in sqlx::query("SELECT j.id,j.updated_at,j.document_id,j.revision,d.source_id,d.title FROM jobs j JOIN documents d ON d.id=j.document_id AND d.revision=j.revision WHERE j.status='failed' ORDER BY j.updated_at DESC LIMIT 500").fetch_all(&mut *tx).await?{
            let id:String=row.get("id");let stamp:String=row.get("updated_at");let document:String=row.get("document_id");
            emit(&mut tx,&rules,notification(format!("failed:{id}:{stamp}"),"processing_failed",format!("Extraction needs attention: {}",row.get::<String,_>("title")),format!("The extraction for revision {} failed at {stamp}. This may leave action coverage incomplete. Retry locally or explicitly select a model in Sources.",row.get::<i64,_>("revision")),vec![row.get("source_id")],id,vec![EvidenceReference{document_id:document,revision:row.get("revision"),line:1,quote:None}],stamp)).await?;
        }
        for row in sqlx::query("SELECT id,source_id,title,revision,updated_at FROM documents WHERE revision>1 ORDER BY updated_at DESC LIMIT 500").fetch_all(&mut *tx).await?{
            let id:String=row.get("id");let revision:i64=row.get("revision");let stamp:String=row.get("updated_at");
            emit(&mut tx,&rules,notification(format!("source:{id}:{revision}"),"source_changed",format!("Source updated: {}",row.get::<String,_>("title")),format!("A new source snapshot, revision {revision}, was captured at {stamp}. Older briefs and citations may need review against this revision."),vec![row.get("source_id")],id.clone(),vec![EvidenceReference{document_id:id,revision,line:1,quote:None}],stamp)).await?;
        }
        let day = Utc::now().date_naive().to_string();
        for row in sqlx::query("SELECT record FROM waiting_for WHERE json_extract(record,'$.status')='waiting' AND json_extract(record,'$.follow_up_on')<=? ORDER BY id LIMIT 1000").bind(&day).fetch_all(&mut *tx).await?{
            let item:WaitingFor=serde_json::from_str(row.get("record"))?;
            emit(&mut tx,&rules,notification(format!("waiting:{}:{}",item.id,item.follow_up_on),"waiting_due",format!("Follow up with {}: {}",item.person,item.title),format!("Your chosen follow-up date {} has arrived (UTC). The original request evidence is attached; this is a reminder to review, and sends no message.",item.follow_up_on),item.source_ids,item.id,item.evidence.into_iter().map(evidence).collect(),format!("{}T00:00:00Z",item.follow_up_on))).await?;
        }
        tx.commit().await?;
        Ok(())
    }
    pub async fn notifications(&self, access: &Access) -> Result<Vec<WorkNotification>> {
        admin(access)?;
        let mut tx = self.pool.begin().await?;
        let rules = mutes(&mut tx).await?;
        let mut result = Vec::new();
        for row in sqlx::query(
            "SELECT record FROM source_control_notifications ORDER BY rowid DESC LIMIT 1000",
        )
        .fetch_all(&mut *tx)
        .await?
        {
            let item: WorkNotification = serde_json::from_str(row.get("record"))?;
            let relevant=match item.kind.as_str(){
                "action_candidate"=>{
                    let version:Option<i64>=sqlx::query_scalar("SELECT version FROM tasks WHERE id=? AND status='candidate'").bind(&item.record_id).fetch_optional(&mut *tx).await?;
                    version.is_some_and(|version|item.id==hash(&format!("task:{}:{version}",item.record_id)))
                },
                "processing_failed"=>{
                    let stamp:Option<String>=sqlx::query_scalar("SELECT j.updated_at FROM jobs j JOIN documents d ON d.id=j.document_id AND d.revision=j.revision WHERE j.id=? AND j.status='failed'").bind(&item.record_id).fetch_optional(&mut *tx).await?;
                    stamp.is_some_and(|stamp|item.id==hash(&format!("failed:{}:{stamp}",item.record_id)))
                },
                "waiting_due"=>sqlx::query_scalar::<_,bool>("SELECT EXISTS(SELECT 1 FROM waiting_for WHERE id=? AND json_extract(record,'$.status')='waiting' AND json_extract(record,'$.follow_up_on')<=? AND json_extract(record,'$.follow_up_on') || 'T00:00:00Z' = ?)").bind(&item.record_id).bind(Utc::now().date_naive().to_string()).bind(&item.triggered_at).fetch_one(&mut *tx).await?,
                "source_changed"=>{if let Some(e)=item.evidence.first(){sqlx::query_scalar::<_,bool>("SELECT EXISTS(SELECT 1 FROM documents WHERE id=? AND revision=?)").bind(&e.document_id).bind(e.revision).fetch_one(&mut *tx).await?}else{false}},
                _=>false,
            };
            if relevant && item.status != "dismissed" && !muted(&item, &rules) {
                result.push(item);
                if result.len() == 100 {
                    break;
                }
            }
        }
        Ok(result)
    }
    pub async fn update_notification(
        &self,
        id: &str,
        req: NotificationUpdate,
        access: &Access,
    ) -> Result<WorkNotification> {
        admin(access)?;
        if !["unread", "read", "dismissed"].contains(&req.status.as_str()) {
            bail!("invalid notification status");
        }
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let (digest, old) =
            operation(&mut tx, "notification", id, &req.idempotency_key, &req).await?;
        let json: String =
            sqlx::query_scalar("SELECT record FROM source_control_notifications WHERE id=?")
                .bind(id)
                .fetch_optional(&mut *tx)
                .await?
                .context("record not found")?;
        let mut item: WorkNotification = serde_json::from_str(&json)?;
        if old.is_some() {
            return Ok(item);
        }
        if item.version != req.expected_version {
            bail!("conflict: notification changed; refresh");
        }
        item.version += 1;
        item.status = req.status;
        sqlx::query("UPDATE source_control_notifications SET record=? WHERE id=?")
            .bind(serde_json::to_string(&item)?)
            .bind(id)
            .execute(&mut *tx)
            .await?;
        event(&mut tx, "notification", id).await?;
        remember(&mut tx, &req.idempotency_key, &digest, id).await?;
        tx.commit().await?;
        Ok(item)
    }
    pub async fn notification_mutes(&self, access: &Access) -> Result<Vec<NotificationMute>> {
        admin(access)?;
        let mut tx = self.pool.begin().await?;
        mutes(&mut tx).await
    }
    pub async fn set_notification_mute(
        &self,
        req: NotificationMuteUpdate,
        access: &Access,
    ) -> Result<NotificationMute> {
        admin(access)?;
        if !KINDS.contains(&req.kind.as_str()) {
            bail!("invalid notification kind");
        }
        if let Some(until) = &req.until {
            let time = DateTime::parse_from_rfc3339(until).context("invalid mute expiry")?;
            if req.active && time <= Utc::now() {
                bail!("invalid: mute expiry must be in the future");
            }
        }
        let id = hash(&format!(
            "{}:{}",
            req.kind,
            req.source_id.as_deref().unwrap_or("*")
        ));
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        if let Some(source) = &req.source_id {
            let exists: bool =
                sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM sources WHERE id=?)")
                    .bind(source)
                    .fetch_one(&mut *tx)
                    .await?;
            if !exists {
                bail!("record not found");
            }
        }
        let (digest, old) = operation(
            &mut tx,
            "notification-mute",
            &id,
            &req.idempotency_key,
            &req,
        )
        .await?;
        let json: Option<String> =
            sqlx::query_scalar("SELECT record FROM source_control_mutes WHERE id=?")
                .bind(&id)
                .fetch_optional(&mut *tx)
                .await?;
        let current: Option<NotificationMute> =
            json.map(|j| serde_json::from_str(&j)).transpose()?;
        if old.is_some() {
            return current.context("record not found");
        }
        let version = current.map(|item| item.version).unwrap_or(0);
        if version != req.expected_version {
            bail!("conflict: mute changed; refresh");
        }
        let item = NotificationMute {
            id: id.clone(),
            version: version + 1,
            kind: req.kind,
            source_id: req.source_id,
            active: req.active,
            until: req.until,
            updated_at: now(),
        };
        sqlx::query("INSERT INTO source_control_mutes(id,record) VALUES(?,?) ON CONFLICT(id) DO UPDATE SET record=excluded.record").bind(&id).bind(serde_json::to_string(&item)?).execute(&mut *tx).await?;
        event(&mut tx, "notification_mute", &id).await?;
        remember(&mut tx, &req.idempotency_key, &digest, &id).await?;
        tx.commit().await?;
        Ok(item)
    }
}
