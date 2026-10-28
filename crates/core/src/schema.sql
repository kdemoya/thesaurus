PRAGMA foreign_keys=ON;
CREATE TABLE IF NOT EXISTS sources (
 id TEXT PRIMARY KEY, name TEXT NOT NULL, kind TEXT NOT NULL, uri TEXT NOT NULL UNIQUE,
 allow_external INTEGER NOT NULL DEFAULT 0, updated_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS documents (
 id TEXT PRIMARY KEY, source_id TEXT NOT NULL REFERENCES sources(id), title TEXT NOT NULL,
 body TEXT NOT NULL, revision INTEGER NOT NULL, content_hash TEXT NOT NULL, updated_at TEXT NOT NULL,
 UNIQUE(source_id)
);
CREATE TABLE IF NOT EXISTS revisions (
 document_id TEXT NOT NULL REFERENCES documents(id), revision INTEGER NOT NULL, title TEXT NOT NULL,
 body TEXT NOT NULL, content_hash TEXT NOT NULL, updated_at TEXT NOT NULL,
 PRIMARY KEY(document_id,revision)
);
CREATE VIRTUAL TABLE IF NOT EXISTS documents_fts USING fts5(document_id UNINDEXED, title, body, tokenize='unicode61');
CREATE TABLE IF NOT EXISTS tasks (
 id TEXT PRIMARY KEY, title TEXT NOT NULL, owner TEXT, due TEXT, status TEXT NOT NULL,
 document_id TEXT NOT NULL REFERENCES documents(id), revision INTEGER NOT NULL, line INTEGER NOT NULL,
 quote TEXT NOT NULL, method TEXT NOT NULL, version INTEGER NOT NULL DEFAULT 1, updated_at TEXT NOT NULL,
 UNIQUE(document_id,revision,line,title)
);
CREATE TABLE IF NOT EXISTS observations (
 id TEXT PRIMARY KEY, body TEXT NOT NULL, contributor TEXT NOT NULL,
 source_ids TEXT NOT NULL, evidence TEXT NOT NULL, idempotency_key TEXT NOT NULL,
 created_at TEXT NOT NULL, UNIQUE(contributor,idempotency_key)
);
CREATE TABLE IF NOT EXISTS jobs (
 id TEXT PRIMARY KEY, document_id TEXT NOT NULL REFERENCES documents(id), revision INTEGER NOT NULL,
 connection TEXT NOT NULL, status TEXT NOT NULL, attempt INTEGER NOT NULL DEFAULT 0,
 summary TEXT NOT NULL DEFAULT '', updated_at TEXT NOT NULL, UNIQUE(document_id,revision,connection)
);
CREATE TABLE IF NOT EXISTS events (
 sequence INTEGER PRIMARY KEY AUTOINCREMENT, kind TEXT NOT NULL, record_id TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS grants (
 id TEXT PRIMARY KEY, name TEXT NOT NULL, token_hash TEXT NOT NULL UNIQUE,
 source_ids TEXT NOT NULL, allow_write INTEGER NOT NULL, allow_external INTEGER NOT NULL,
 revoked INTEGER NOT NULL DEFAULT 0
);
PRAGMA user_version=1;
