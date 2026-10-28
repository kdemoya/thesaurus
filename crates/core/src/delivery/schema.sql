CREATE TABLE IF NOT EXISTS delivery_commitments (
 id TEXT PRIMARY KEY,
 project_id TEXT NOT NULL REFERENCES project_briefs(id),
 record TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS delivery_commitments_project ON delivery_commitments(project_id);
CREATE TABLE IF NOT EXISTS delivery_history (
 sequence INTEGER PRIMARY KEY AUTOINCREMENT,
 commitment_id TEXT NOT NULL REFERENCES delivery_commitments(id),
 record TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS delivery_history_owner ON delivery_history(commitment_id,sequence);
