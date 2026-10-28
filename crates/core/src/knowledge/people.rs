use super::*;
async fn raw(tx: &mut Transaction<'_, Sqlite>, id: &str) -> Result<PersonRecord> {
    let s: String = sqlx::query_scalar("SELECT record FROM knowledge_people WHERE id=?")
        .bind(id)
        .fetch_optional(&mut **tx)
        .await?
        .context("record not found")?;
    Ok(serde_json::from_str(&s)?)
}
async fn save(
    tx: &mut Transaction<'_, Sqlite>,
    p: &PersonRecord,
    action: &str,
    note: &str,
) -> Result<()> {
    if serde_json::to_vec(p)?.len() > 150000 {
        bail!("invalid: person evidence limit reached");
    }
    sqlx::query("INSERT INTO knowledge_people(id,project_id,record) VALUES(?,?,?) ON CONFLICT(id) DO UPDATE SET record=excluded.record").bind(&p.id).bind(&p.project_id).bind(serde_json::to_string(p)?).execute(&mut **tx).await?;
    let h = PersonHistory {
        sequence: 0,
        action: action.into(),
        note: note.into(),
        contributor: "You".into(),
        created_at: now(),
        record: p.clone(),
    };
    sqlx::query("INSERT INTO knowledge_people_history(person_id,record) VALUES(?,?)")
        .bind(&p.id)
        .bind(serde_json::to_string(&h)?)
        .execute(&mut **tx)
        .await?;
    event(tx, "person", &p.id).await
}
impl Store {
    pub async fn person(&self, id: &str, access: &Access) -> Result<PersonRecord> {
        let mut tx = self.pool.begin().await?;
        let p = raw(&mut tx, id).await?;
        permit(&mut tx, &p.source_ids, access).await?;
        tx.commit().await?;
        Ok(p)
    }
    pub async fn create_person(&self, req: PersonCreateRequest) -> Result<PersonRecord> {
        validate_text(&req.display_name, 200, "person display name")?;
        validate_text(&req.identity_note, 2000, "identity note")?;
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let (digest, old) =
            operation(&mut tx, "person-create", "", &req.idempotency_key, &req).await?;
        let id = if let Some(id) = old {
            id
        } else {
            let source_ids = citations(&mut tx, &req.project_id, &req.evidence).await?;
            let count: i64 =
                sqlx::query_scalar("SELECT COUNT(*) FROM knowledge_people WHERE project_id=?")
                    .bind(&req.project_id)
                    .fetch_one(&mut *tx)
                    .await?;
            if count >= 1000 {
                bail!("invalid: project already has 1000 people records");
            }
            let p = PersonRecord {
                id: uuid::Uuid::new_v4().to_string(),
                project_id: req.project_id,
                display_name: req.display_name.trim().into(),
                identity_note: req.identity_note.trim().into(),
                evidence: req.evidence,
                aliases: vec![],
                roles: vec![],
                works: vec![],
                source_ids,
                version: 1,
                status: "active".into(),
                contributor: "You".into(),
                created_at: now(),
                updated_at: now(),
            };
            save(&mut tx, &p, "created", "").await?;
            remember(&mut tx, &req.idempotency_key, &digest, &p.id).await?;
            p.id
        };
        tx.commit().await?;
        self.person(&id, &Access::admin()).await
    }
    pub async fn update_person(&self, id: &str, req: PersonUpdateRequest) -> Result<PersonRecord> {
        validate_text(&req.note, 2000, "person change reason")?;
        if ![
            "edit",
            "alias",
            "role",
            "work",
            "remove_alias",
            "remove_role",
            "remove_work",
            "archive",
            "restore",
        ]
        .contains(&req.action.as_str())
        {
            bail!("invalid person action");
        }
        if req.action != "edit" && (req.display_name.is_some() || req.identity_note.is_some()) {
            bail!("invalid: name changes require edit");
        }
        if (req.action != "alias" && req.alias.is_some())
            || (req.action != "role" && req.role.is_some())
            || (req.action != "work" && req.work.is_some())
        {
            bail!("invalid: profile field does not match action");
        }
        if !["remove_alias", "remove_role", "remove_work"].contains(&req.action.as_str())
            && req.record_index.is_some()
        {
            bail!("invalid: record_index requires a removal action");
        }
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let (digest, old) =
            operation(&mut tx, "person-update", id, &req.idempotency_key, &req).await?;
        if old.is_none() {
            let mut p = raw(&mut tx, id).await?;
            if p.version != req.expected_version {
                bail!("conflict: person changed; refresh");
            }
            if p.status == "archived" && req.action != "restore" {
                bail!("conflict: restore person before editing");
            }
            let mut ev = vec![];
            match req.action.as_str() {
                "edit" => {
                    if let Some(name) = req.display_name {
                        validate_text(&name, 200, "person name")?;
                        p.display_name = name.trim().into();
                    }
                    if let Some(n) = req.identity_note {
                        validate_text(&n, 2000, "identity note")?;
                        p.identity_note = n.trim().into();
                    }
                }
                "alias" => {
                    let a = req.alias.context("invalid: provide alias")?;
                    validate_text(&a.label, 200, "alias label")?;
                    if !["confirmed", "uncertain"].contains(&a.certainty.as_str()) {
                        bail!("invalid alias certainty");
                    }
                    if p.aliases.len() >= 30 || p.aliases.iter().any(|o| o.label == a.label) {
                        bail!("invalid: duplicate alias or 30 alias limit reached");
                    }
                    ev = a.evidence.clone();
                    p.aliases.push(a);
                }
                "role" => {
                    let r = req.role.context("invalid: provide role")?;
                    validate_text(&r.role, 300, "role")?;
                    validate_text(&r.scope, 300, "role scope")?;
                    interval(&r.valid_from, r.valid_until.as_deref())?;
                    if p.roles.len() >= 30 {
                        bail!("invalid: 30 role limit reached");
                    }
                    ev = r.evidence.clone();
                    p.roles.push(r);
                }
                "work" => {
                    let w = req.work.context("invalid: provide authored work")?;
                    validate_text(&w.title, 300, "authored work title")?;
                    validate_text(&w.topic, 300, "authored work topic")?;
                    if !["author", "reviewer", "maintainer"].contains(&w.relationship.as_str()) {
                        bail!("invalid authored work relationship");
                    }
                    if p.works.len() >= 50 {
                        bail!("invalid: 50 authored work limit reached");
                    }
                    ev = w.evidence.clone();
                    p.works.push(w);
                }
                "remove_alias" => {
                    let i = req.record_index.context("invalid: choose alias index")?;
                    if i >= p.aliases.len() {
                        bail!("invalid alias index");
                    }
                    p.aliases.remove(i);
                }
                "remove_role" => {
                    let i = req.record_index.context("invalid: choose role index")?;
                    if i >= p.roles.len() {
                        bail!("invalid role index");
                    }
                    p.roles.remove(i);
                }
                "remove_work" => {
                    let i = req.record_index.context("invalid: choose work index")?;
                    if i >= p.works.len() {
                        bail!("invalid work index");
                    }
                    p.works.remove(i);
                }
                "archive" => p.status = "archived".into(),
                "restore" => {
                    if p.status != "archived" {
                        bail!("conflict: person already active");
                    }
                    p.status = "active".into();
                }
                _ => {}
            }
            if !ev.is_empty() {
                p.source_ids
                    .extend(citations(&mut tx, &p.project_id, &ev).await?);
                p.source_ids.sort();
                p.source_ids.dedup();
            } else if ["alias", "role", "work"].contains(&req.action.as_str()) {
                bail!("invalid: aliases, roles and authored work require evidence");
            }
            p.version += 1;
            p.updated_at = now();
            save(&mut tx, &p, &req.action, &req.note).await?;
            remember(&mut tx, &req.idempotency_key, &digest, id).await?;
        }
        tx.commit().await?;
        self.person(id, &Access::admin()).await
    }
    pub async fn people(
        &self,
        project: &str,
        q: KnowledgeQuery,
        access: &Access,
    ) -> Result<PeoplePage> {
        let (offset, limit) = page(&q)?;
        let query = q.query.clone().unwrap_or_default();
        if query.len() > 1000 {
            bail!("invalid people query");
        }
        let words = terms(&query);
        let lower = query.to_lowercase();
        let mut tx = self.pool.begin().await?;
        let allowed = allowed(&mut tx, project, access).await?;
        let mut matches = vec![];
        for row in sqlx::query("SELECT record FROM knowledge_people WHERE project_id=? ORDER BY id")
            .bind(project)
            .fetch_all(&mut *tx)
            .await?
        {
            let p: PersonRecord = serde_json::from_str(row.get("record"))?;
            if !p.source_ids.iter().all(|s| allowed.contains(s))
                || (!q.include_archived && p.status != "active")
            {
                continue;
            }
            let mut basis = vec![];
            let mut score = 0;
            let mut uncertain = false;
            if !lower.is_empty() && p.display_name.to_lowercase() == lower {
                basis.push("Exact registered display name; identity is user-curated".into());
                score += 100;
            }
            if !lower.is_empty()
                && p.display_name.to_lowercase() != lower
                && p.display_name.to_lowercase().contains(&lower)
            {
                basis.push("Partial registered name; verify identity".into());
                score += 40;
                uncertain = true;
            }
            for a in &p.aliases {
                if !lower.is_empty() && a.label.to_lowercase() == lower {
                    basis.push(format!("{} alias: {}", a.certainty, a.label));
                    score += if a.certainty == "confirmed" { 80 } else { 30 };
                    uncertain |= a.certainty == "uncertain";
                }
            }
            for r in &p.roles {
                if r.valid_from > Utc::now().date_naive().to_string()
                    || r.valid_until
                        .as_ref()
                        .is_some_and(|d| Utc::now().date_naive().to_string() >= *d)
                {
                    continue;
                }
                let text = format!("{} {}", r.role, r.scope).to_lowercase();
                let n = words.iter().filter(|w| text.contains(w.as_str())).count();
                if n > 0 {
                    basis.push(format!("Cited role: {} ({})", r.role, r.scope));
                    score += n * 5;
                }
            }
            for w in &p.works {
                let text = format!("{} {}", w.title, w.topic).to_lowercase();
                let n = words.iter().filter(|t| text.contains(t.as_str())).count();
                if n > 0 {
                    basis.push(format!("Cited {} work: {}", w.relationship, w.title));
                    score += n * 3;
                }
            }
            if score == 0 && !query.is_empty() {
                continue;
            }
            if basis.is_empty() {
                basis.push("Registered identity record; no relevance inference".into());
            }
            basis.truncate(20);
            matches.push(PersonMatch {
                person: p,
                match_basis: basis,
                uncertain_identity: uncertain,
                score,
            });
        }
        matches.sort_by(|a, b| {
            b.score
                .cmp(&a.score)
                .then_with(|| a.person.id.cmp(&b.person.id))
        });
        let total = matches.len();
        let mut items = matches
            .into_iter()
            .skip(offset)
            .take(limit)
            .collect::<Vec<_>>();
        while serde_json::to_vec(&items)?.len() > 300000 {
            if items.pop().is_none() {
                break;
            }
        }
        let next_offset = (offset + items.len() < total).then_some(offset + items.len());
        tx.commit().await?;
        Ok(PeoplePage{project_id:project.into(),query,items,total,offset,next_offset,caution:"Names and owner labels are not automatically linked to identities. Matches use explicit user-curated aliases, cited current roles and attributed authored work. Uncertain aliases remain uncertain; this is a relevance lookup, not an expertise certification.".into()})
    }
    pub async fn person_history(
        &self,
        id: &str,
        q: KnowledgeQuery,
        access: &Access,
    ) -> Result<PersonHistoryPage> {
        let (offset, limit) = page(&q)?;
        let mut tx = self.pool.begin().await?;
        let p = raw(&mut tx, id).await?;
        permit(&mut tx, &p.source_ids, access).await?;
        let total: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM knowledge_people_history WHERE person_id=?")
                .bind(id)
                .fetch_one(&mut *tx)
                .await?;
        let rows=sqlx::query("SELECT sequence,record FROM knowledge_people_history WHERE person_id=? ORDER BY sequence DESC LIMIT ? OFFSET ?").bind(id).bind(limit as i64).bind(offset as i64).fetch_all(&mut *tx).await?;
        let mut items = vec![];
        for r in rows {
            let mut h: PersonHistory = serde_json::from_str(r.get("record"))?;
            h.sequence = r.get("sequence");
            items.push(h);
            if serde_json::to_vec(&items)?.len() > 300000 {
                if items.len() > 1 {
                    items.pop();
                }
                break;
            }
        }
        let next_offset = (offset + items.len() < total as usize).then_some(offset + items.len());
        tx.commit().await?;
        Ok(PersonHistoryPage {
            items,
            total: total as usize,
            offset,
            next_offset,
        })
    }
}
