CREATE TABLE IF NOT EXISTS procedure_run_reviews(project_id TEXT NOT NULL REFERENCES project_briefs(id),run_id TEXT NOT NULL REFERENCES runtime_runs(id),record TEXT NOT NULL,PRIMARY KEY(project_id,run_id));
CREATE TABLE IF NOT EXISTS procedure_suggestions(id TEXT PRIMARY KEY,project_id TEXT NOT NULL REFERENCES project_briefs(id),record TEXT NOT NULL);
CREATE INDEX IF NOT EXISTS procedure_suggestions_project ON procedure_suggestions(project_id);
CREATE TABLE IF NOT EXISTS procedure_history(sequence INTEGER PRIMARY KEY AUTOINCREMENT,kind TEXT NOT NULL,record_id TEXT NOT NULL,record TEXT NOT NULL);
CREATE INDEX IF NOT EXISTS procedure_history_owner ON procedure_history(kind,record_id,sequence);
