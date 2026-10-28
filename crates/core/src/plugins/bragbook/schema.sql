CREATE TABLE IF NOT EXISTS bragbook_entries (
 id TEXT PRIMARY KEY, origin_key TEXT NOT NULL UNIQUE,
 status TEXT NOT NULL, version INTEGER NOT NULL,
 collected_at TEXT NOT NULL, record TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS bragbook_entries_status ON bragbook_entries(status,collected_at,id);
CREATE TABLE IF NOT EXISTS bragbook_history (
 entry_id TEXT NOT NULL REFERENCES bragbook_entries(id),
 version INTEGER NOT NULL, contributor TEXT NOT NULL,
 record TEXT NOT NULL, PRIMARY KEY(entry_id,version)
);
-- Delivery state and the plugin outbox share the domain write transaction.
CREATE TRIGGER IF NOT EXISTS bragbook_delivery_history AFTER INSERT ON delivery_history BEGIN
 INSERT INTO plugin_events(id,kind,record_kind,record_id,record_revision,source_ids,project_ids,correlation_id)
 SELECT lower(hex(randomblob(16))),'delivery.changed','commitment',c.id,
 json_extract(NEW.record,'$.version'),json_extract(c.record,'$.source_ids'),
 json_array(c.project_id),c.id FROM delivery_commitments c WHERE c.id=NEW.commitment_id;
END;
