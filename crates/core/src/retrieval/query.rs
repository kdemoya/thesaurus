use super::*;
fn result(
    doc: &Document,
    path: &str,
    policy: &str,
    fetched: Option<String>,
    stale: bool,
) -> RetrievalResult {
    let evidence = doc
        .body
        .lines()
        .enumerate()
        .filter(|(_, l)| !l.trim().is_empty() && l.len() <= 2000)
        .take(3)
        .map(|(i, l)| Evidence {
            document_id: doc.id.clone(),
            revision: doc.revision,
            line: i as i64 + 1,
            quote: l.into(),
        })
        .collect();
    RetrievalResult {
        hit: SearchHit {
            document_id: doc.id.clone(),
            source_id: doc.source_id.clone(),
            title: doc.title.clone(),
            revision: doc.revision,
            excerpt: doc.body.chars().take(2000).collect(),
        },
        path: path.into(),
        policy: policy.into(),
        fetched_at: fetched,
        stale,
        evidence,
    }
}
impl Store {
    pub async fn fetch_upstream(
        &self,
        id: &str,
        query: &str,
        approve: bool,
        key: &str,
        access: &Access,
    ) -> Result<(Document, Vec<String>, String, bool)> {
        validate_text(query, 1024, "retrieval query")?;
        validate_text(key, 200, "idempotency key")?;
        if !approve {
            bail!("forbidden: explicitly approve this upstream read");
        }
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let c = load(&mut tx, id, access).await?;
        if access.source_ids.is_some() && !c.allow_agent_live {
            bail!("forbidden: upstream live reads are user-only for this source");
        }
        let digest = hash(&format!(
            "{}:{query}:{}:{}",
            c.id, c.version, access.contributor
        ));
        let fetch_id = hash(&format!("{id}:{key}"));
        if let Some(row) = sqlx::query("SELECT * FROM retrieval_fetches WHERE id=?")
            .bind(&fetch_id)
            .fetch_optional(&mut *tx)
            .await?
        {
            if row.get::<String, _>("request_hash") != digest {
                bail!("conflict: retrieval key was used for different input");
            }
            let status: String = row.get("status");
            if status != "completed" {
                bail!(
                    "conflict: this upstream attempt is {status}; inspect its error and use a new request key for an explicit retry"
                );
            }
            let document: String = row
                .get::<Option<String>, _>("document_id")
                .context("invalid stored upstream receipt")?;
            let revision: i64 = row.get::<Option<i64>, _>("revision").context(
                "invalid: this legacy receipt has no pinned revision; use a new explicit request",
            )?;
            let fetched_at: String = row.get("created_at");
            tx.commit().await?;
            return Ok((
                self.document(&document, Some(revision), access).await?,
                vec!["Replayed an existing fetch receipt; no upstream call made".into()],
                fetched_at,
                false,
            ));
        }
        let count:i64=sqlx::query_scalar("SELECT COUNT(*) FROM retrieval_fetches WHERE upstream_id=? AND substr(created_at,1,10)=?").bind(id).bind(chrono::Utc::now().format("%Y-%m-%d").to_string()).fetch_one(&mut *tx).await?;
        if count >= c.daily_limit as i64 {
            bail!("forbidden: upstream daily read limit reached");
        }
        sqlx::query("INSERT INTO retrieval_fetches(id,upstream_id,request_hash,request_json,status,created_at) VALUES(?,?,?,?,'fetching',?)").bind(&fetch_id).bind(id).bind(&digest).bind(serde_json::to_string(&json!({"query":query,"resource_uri":c.resource_uri,"read_tool":c.read_tool,"arguments":client::arguments(&c.tool_arguments,query),"connector_version":c.version,"destination_hash":hash(&serde_json::to_string(&json!({"transport":c.transport,"command":c.command,"args":c.args,"endpoint":c.endpoint}))?)}))?).bind(now()).execute(&mut *tx).await?;
        tx.commit().await?;
        let fetched = tokio::time::timeout(
            std::time::Duration::from_secs(35),
            client::fetch(self, &c, query),
        )
        .await
        .context("invalid: upstream request timed out")
        .and_then(|r| r);
        let (body, notes) = match fetched {
            Ok(r) => r,
            Err(e) => {
                let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
                sqlx::query("UPDATE retrieval_fetches SET status='failed',error=? WHERE id=?")
                    .bind(e.to_string())
                    .bind(&fetch_id)
                    .execute(&mut *tx)
                    .await?;
                sqlx::query("UPDATE retrieval_upstreams SET record=json_set(record,'$.last_error',?,'$.updated_at',?) WHERE id=? AND version=?").bind(e.to_string()).bind(now()).bind(id).bind(c.version).execute(&mut *tx).await?;
                event(&mut tx, "upstream", id).await?;
                tx.commit().await?;
                return Err(e);
            }
        };
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let mut current = load(&mut tx, id, access).await?;
        if current.version != c.version {
            bail!("conflict: upstream config changed during fetch; response not imported");
        }
        let document_id = format!("doc-mcp-{}", &hash(&c.id)[..32]);
        let title = format!(
            "{} · {}",
            c.name,
            c.resource_uri
                .as_deref()
                .or(c.read_tool.as_deref())
                .unwrap_or("upstream")
                .chars()
                .take(80)
                .collect::<String>()
        );
        let digest = hash(&format!("{title}\0{body}"));
        let old = sqlx::query("SELECT revision,content_hash FROM documents WHERE id=?")
            .bind(&document_id)
            .fetch_optional(&mut *tx)
            .await?;
        let previous = old
            .as_ref()
            .map(|r| r.get::<i64, _>("revision"))
            .unwrap_or(0);
        let changed = old
            .as_ref()
            .is_none_or(|r| r.get::<String, _>("content_hash") != digest);
        let revision = if changed { previous + 1 } else { previous };
        let timestamp = now();
        if changed {
            sqlx::query("INSERT INTO documents(id,source_id,title,body,revision,content_hash,updated_at) VALUES(?,?,?,?,?,?,?) ON CONFLICT(id) DO UPDATE SET title=excluded.title,body=excluded.body,revision=excluded.revision,content_hash=excluded.content_hash,updated_at=excluded.updated_at").bind(&document_id).bind(&c.source_id).bind(&title).bind(&body).bind(revision).bind(&digest).bind(&timestamp).execute(&mut *tx).await?;
            sqlx::query("INSERT INTO revisions(document_id,revision,title,body,content_hash,updated_at) VALUES(?,?,?,?,?,?)").bind(&document_id).bind(revision).bind(&title).bind(&body).bind(&digest).bind(&timestamp).execute(&mut *tx).await?;
            sqlx::query("DELETE FROM documents_fts WHERE document_id=?")
                .bind(&document_id)
                .execute(&mut *tx)
                .await?;
            sqlx::query("INSERT INTO documents_fts(document_id,title,body) VALUES(?,?,?)")
                .bind(&document_id)
                .bind(&title)
                .bind(&body)
                .execute(&mut *tx)
                .await?;
            sqlx::query("UPDATE tasks SET status='superseded',version=version+1,updated_at=? WHERE document_id=? AND status='candidate'").bind(&timestamp).bind(&document_id).execute(&mut *tx).await?;
            sqlx::query("INSERT INTO jobs(id,document_id,revision,connection,status,updated_at) VALUES(?,?,?,'local','queued',?)").bind(uuid::Uuid::new_v4().to_string()).bind(&document_id).bind(revision).bind(&timestamp).execute(&mut *tx).await?;
        }
        current.last_document_id = Some(document_id.clone());
        current.last_revision = Some(revision);
        current.last_fetched_at = Some(timestamp.clone());
        current.last_error = None;
        current.updated_at = timestamp.clone();
        sqlx::query("UPDATE retrieval_upstreams SET record=? WHERE id=? AND version=?")
            .bind(serde_json::to_string(&current)?)
            .bind(id)
            .bind(c.version)
            .execute(&mut *tx)
            .await?;
        sqlx::query("UPDATE sources SET updated_at=? WHERE id=?")
            .bind(&timestamp)
            .bind(&c.source_id)
            .execute(&mut *tx)
            .await?;
        sqlx::query(
            "UPDATE retrieval_fetches SET status='completed',document_id=?,revision=? WHERE id=?",
        )
        .bind(&document_id)
        .bind(revision)
        .bind(&fetch_id)
        .execute(&mut *tx)
        .await?;
        event(&mut tx, "document", &document_id).await?;
        tx.commit().await?;
        self.refresh_discovery_document(&document_id).await?;
        Ok((
            self.document(&document_id, Some(revision), access).await?,
            notes,
            timestamp,
            true,
        ))
    }
    pub async fn federated_retrieve(
        &self,
        req: FederatedRequest,
        access: &Access,
    ) -> Result<FederatedResponse> {
        validate_text(&req.query, 1024, "retrieval query")?;
        validate_text(&req.idempotency_key, 200, "idempotency key")?;
        if req.source_ids.is_empty()
            || req.source_ids.len() > 20
            || req.limit == 0
            || req.limit > 50
        {
            bail!("invalid: select1–20sources and1–50results");
        }
        let mut source_ids = req.source_ids.clone();
        source_ids.sort();
        source_ids.dedup();
        let allowed = self.sources(access).await?;
        if source_ids
            .iter()
            .any(|id| !allowed.iter().any(|s| &s.id == id))
        {
            bail!("record not found");
        }
        let mut response = FederatedResponse {
            query: req.query.clone(),
            results: vec![],
            sources: vec![],
            omissions: vec![],
            watermark: 0,
        };
        for source in source_ids {
            let mut tx = self.pool.begin().await?;
            let id: Option<String> =
                sqlx::query_scalar("SELECT id FROM retrieval_upstreams WHERE source_id=?")
                    .bind(&source)
                    .fetch_optional(&mut *tx)
                    .await?;
            let c = if let Some(id) = id {
                Some(load(&mut tx, &id, access).await?)
            } else {
                None
            };
            tx.commit().await?;
            let policy = c
                .as_ref()
                .map(|c| c.policy.clone())
                .unwrap_or("index".into());
            let mut outcome = RetrievalOutcome {
                source_id: source.clone(),
                policy: policy.clone(),
                path: "index".into(),
                error: None,
                checkpoint: None,
            };
            if ["live", "hybrid"].contains(&policy.as_str()) {
                if let Some(c) = &c {
                    match self
                        .fetch_upstream(
                            &c.id,
                            &req.query,
                            req.approve_upstream,
                            &hash(&format!("{}:{source}", req.idempotency_key)),
                            access,
                        )
                        .await
                    {
                        Ok((doc, notes, fetched_at, fresh)) => {
                            outcome.path = if fresh {
                                "live".into()
                            } else {
                                "live_receipt".into()
                            };
                            outcome.checkpoint = Some(BriefSourceRevision {
                                source_id: source.clone(),
                                document_id: doc.id.clone(),
                                title: doc.title.clone(),
                                revision: doc.revision,
                                content_hash: doc.content_hash.clone(),
                            });
                            response.results.push(result(
                                &doc,
                                &outcome.path,
                                &policy,
                                Some(fetched_at),
                                !fresh,
                            ));
                            response.omissions.extend(notes);
                        }
                        Err(e) => {
                            outcome.error = Some(e.to_string());
                            outcome.path = if policy == "hybrid" {
                                "index_after_live_error".into()
                            } else {
                                "live_error".into()
                            };
                        }
                    }
                }
            }
            if policy != "live" {
                let hits = self
                    .search(
                        &SearchRequest {
                            query: req.query.clone(),
                            source_id: Some(source.clone()),
                            limit: Some(req.limit),
                        },
                        access,
                    )
                    .await?;
                for hit in hits {
                    if response
                        .results
                        .iter()
                        .any(|r| r.hit.document_id == hit.document_id)
                    {
                        continue;
                    }
                    let d = self
                        .document(&hit.document_id, Some(hit.revision), access)
                        .await?;
                    let mut r = result(
                        &d,
                        if outcome.error.is_some() {
                            "index_after_live_error"
                        } else {
                            "index"
                        },
                        &policy,
                        c.as_ref().and_then(|c| c.last_fetched_at.clone()),
                        c.is_some(),
                    );
                    r.hit.excerpt = hit.excerpt;
                    response.results.push(r);
                }
            }
            response.sources.push(outcome);
        }
        if response.results.len() > req.limit as usize {
            response.omissions.push(format!(
                "{} results omitted by requested limit",
                response.results.len() - req.limit as usize
            ));
            response.results.truncate(req.limit as usize);
        }
        while serde_json::to_vec(&response)?.len() > 250000 {
            if response.results.pop().is_none() {
                bail!("invalid: retrieval metadata exceeds250KB");
            }
            response
                .omissions
                .push("One result omitted to keep response within250KB".into());
        }
        response.watermark = self.watermark().await?;
        Ok(response)
    }
}
