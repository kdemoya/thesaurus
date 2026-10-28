use super::*;

fn markdown_text(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('\\', "\\\\")
        .replace('`', "\\`")
        .replace('*', "\\*")
        .replace('_', "\\_")
        .replace('[', "\\[")
        .replace(']', "\\]")
        .replace('#', "\\#")
}
impl Store {
    pub async fn export_bragbook(
        &self,
        req: BragbookExportRequest,
        access: &Access,
    ) -> Result<Value> {
        admin(access)?;
        let mut ids = req.entry_ids;
        ids.sort();
        ids.dedup();
        if ids.is_empty() || ids.len() > 100 || !["markdown", "json"].contains(&req.format.as_str())
        {
            bail!("invalid: select 1–100 accepted entries and Markdown or JSON");
        }
        let mut tx = self.pool.begin().await?;
        let p = load(&mut tx, ID).await?;
        let mut items = vec![];
        for id in ids {
            let item = raw(&mut tx, &id).await?;
            allowed(&mut tx, &p, &item, access).await?;
            let item = refresh(&mut tx, item).await?;
            if item.status != "accepted" || item.support_changed {
                bail!(
                    "conflict: every selected entry must be accepted and its underlying work still completed"
                );
            }
            items.push(item);
        }
        items.sort_by(|a, b| {
            a.occurred_on
                .cmp(&b.occurred_on)
                .then(a.collected_at.cmp(&b.collected_at))
        });
        let timestamp = now();
        let payload =
            json!({"schema":"thesaurus-bragbook/v1","exported_at":timestamp,"entries":items});
        let body = if req.format == "json" {
            serde_json::to_string_pretty(&payload)?
        } else {
            let mut md = format!(
                "# Accomplishment highlights\n\nPrepared {timestamp} from personally reviewed entries.\n\n"
            );
            for item in &items {
                md.push_str(&format!(
                    "## {}\n\n{}\n\n{}\n\n",
                    markdown_text(&item.title),
                    item.occurred_on
                        .as_deref()
                        .unwrap_or("Accomplishment date not recorded"),
                    markdown_text(&item.contribution)
                ));
                if !item.impact.is_empty() {
                    md.push_str(&format!("Impact: {}\n\n", markdown_text(&item.impact)));
                }
                md.push_str(&format!(
                    "Reviewed: {} · entry {} · version {}\n\n",
                    item.reviewed_at.as_deref().unwrap_or("unknown"),
                    item.id,
                    item.version
                ));
                if item.stale_evidence {
                    md.push_str("Evidence is pinned to an earlier source revision.\n\n");
                }
                for e in &item.evidence {
                    md.push_str(&format!(
                        "- Source {} · revision {} · line {}\n  > {}\n",
                        e.document_id,
                        e.revision,
                        e.line,
                        markdown_text(&e.quote)
                    ));
                }
                md.push('\n');
            }
            md
        };
        if body.len() > 2_000_000 {
            bail!("invalid: highlights exceed 2 MB; select fewer entries");
        }
        tx.commit().await?;
        Ok(
            json!({"body":body,"filename":format!("bragbook-{}.{}",&timestamp[..10],if req.format=="json"{"json"}else{"md"}),"content_type":if req.format=="json"{"application/json"}else{"text/markdown"},"entry_count":items.len(),"exported_at":timestamp}),
        )
    }
}
