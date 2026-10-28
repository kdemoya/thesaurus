CREATE TABLE IF NOT EXISTS retrieval_upstreams(id TEXT PRIMARY KEY,source_id TEXT NOT NULL UNIQUE,project_id TEXT NOT NULL,version INTEGER NOT NULL,record TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS retrieval_policies(source_id TEXT PRIMARY KEY,policy TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS retrieval_fetches(id TEXT PRIMARY KEY,upstream_id TEXT NOT NULL,request_hash TEXT NOT NULL,request_json TEXT NOT NULL DEFAULT '{}',status TEXT NOT NULL,document_id TEXT,revision INTEGER,error TEXT,created_at TEXT NOT NULL);
CREATE INDEX IF NOT EXISTS retrieval_fetch_usage ON retrieval_fetches(upstream_id,created_at);
