CREATE TABLE IF NOT EXISTS discovery_cues (id TEXT PRIMARY KEY,project_id TEXT NOT NULL REFERENCES project_briefs(id),record TEXT NOT NULL);
CREATE INDEX IF NOT EXISTS discovery_cues_project ON discovery_cues(project_id);
CREATE TABLE IF NOT EXISTS discovery_gaps (id TEXT PRIMARY KEY,project_id TEXT NOT NULL REFERENCES project_briefs(id),record TEXT NOT NULL);
CREATE INDEX IF NOT EXISTS discovery_gaps_project ON discovery_gaps(project_id);
CREATE TABLE IF NOT EXISTS discovery_watermarks (project_id TEXT NOT NULL REFERENCES project_briefs(id),document_id TEXT NOT NULL REFERENCES documents(id),revision INTEGER NOT NULL,PRIMARY KEY(project_id,document_id));
CREATE TABLE IF NOT EXISTS discovery_settings (project_id TEXT PRIMARY KEY REFERENCES project_briefs(id),muted INTEGER NOT NULL DEFAULT 0);
CREATE TABLE IF NOT EXISTS discovery_history (sequence INTEGER PRIMARY KEY AUTOINCREMENT,kind TEXT NOT NULL,record_id TEXT NOT NULL,record TEXT NOT NULL);
CREATE INDEX IF NOT EXISTS discovery_history_owner ON discovery_history(kind,record_id,sequence);
CREATE TABLE IF NOT EXISTS discovery_suggestion_dispositions(gap_id TEXT NOT NULL REFERENCES discovery_gaps(id),suggestion_id TEXT NOT NULL,status TEXT NOT NULL,PRIMARY KEY(gap_id,suggestion_id));
