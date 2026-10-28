CREATE TABLE IF NOT EXISTS knowledge_claims(id TEXT PRIMARY KEY,project_id TEXT NOT NULL REFERENCES project_briefs(id),source_ids TEXT NOT NULL,version INTEGER NOT NULL);
CREATE INDEX IF NOT EXISTS knowledge_claims_project ON knowledge_claims(project_id);
CREATE TABLE IF NOT EXISTS knowledge_claim_revisions(id TEXT NOT NULL REFERENCES knowledge_claims(id),version INTEGER NOT NULL,observed_at TEXT NOT NULL,record TEXT NOT NULL,PRIMARY KEY(id,version));
CREATE TABLE IF NOT EXISTS knowledge_people(id TEXT PRIMARY KEY,project_id TEXT NOT NULL REFERENCES project_briefs(id),record TEXT NOT NULL);
CREATE INDEX IF NOT EXISTS knowledge_people_project ON knowledge_people(project_id);
CREATE TABLE IF NOT EXISTS knowledge_people_history(sequence INTEGER PRIMARY KEY AUTOINCREMENT,person_id TEXT NOT NULL REFERENCES knowledge_people(id),record TEXT NOT NULL);
CREATE INDEX IF NOT EXISTS knowledge_people_history_owner ON knowledge_people_history(person_id,sequence);
