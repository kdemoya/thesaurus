CREATE TABLE IF NOT EXISTS repo_connections(id TEXT PRIMARY KEY,project_id TEXT NOT NULL,version INTEGER NOT NULL,record TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS repo_reports(id TEXT PRIMARY KEY,repository_id TEXT NOT NULL,record TEXT NOT NULL,created_at TEXT NOT NULL);
CREATE INDEX IF NOT EXISTS repo_report_owner ON repo_reports(repository_id,created_at);
CREATE TABLE IF NOT EXISTS repo_symbol_queries(id TEXT PRIMARY KEY,repository_id TEXT NOT NULL,record TEXT NOT NULL,created_at TEXT NOT NULL);
CREATE INDEX IF NOT EXISTS repo_symbol_owner ON repo_symbol_queries(repository_id,created_at);
