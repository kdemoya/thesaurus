CREATE TABLE IF NOT EXISTS cap_audio(id TEXT PRIMARY KEY,project_id TEXT NOT NULL,record TEXT NOT NULL,created_at TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS cap_sessions(id TEXT PRIMARY KEY,version INTEGER NOT NULL,state TEXT NOT NULL,project_id TEXT NOT NULL,record TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS cap_frames(id TEXT PRIMARY KEY,session_id TEXT NOT NULL,record TEXT NOT NULL,png BLOB NOT NULL,created_at TEXT NOT NULL);
CREATE INDEX IF NOT EXISTS cap_frame_owner ON cap_frames(session_id,created_at);
CREATE VIRTUAL TABLE IF NOT EXISTS cap_frames_fts USING fts5(frame_id UNINDEXED,text);

CREATE TABLE IF NOT EXISTS live_meetings(id TEXT PRIMARY KEY, version INTEGER NOT NULL, state TEXT NOT NULL, record TEXT NOT NULL, created_at TEXT NOT NULL);
CREATE UNIQUE INDEX IF NOT EXISTS live_meeting_single_recording ON live_meetings((1)) WHERE state IN ('starting','recording','pausing','stopping');
CREATE TABLE IF NOT EXISTS live_meeting_notes(id TEXT PRIMARY KEY,session_id TEXT NOT NULL REFERENCES live_meetings(id),record TEXT NOT NULL,created_at TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS live_meeting_chunks(id TEXT PRIMARY KEY,session_id TEXT NOT NULL REFERENCES live_meetings(id),record TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS live_recording_settings(id INTEGER PRIMARY KEY CHECK(id=1),record TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS meeting_summaries(id TEXT PRIMARY KEY REFERENCES runtime_runs(id),session_id TEXT NOT NULL REFERENCES live_meetings(id),status TEXT NOT NULL,record TEXT NOT NULL);
CREATE INDEX IF NOT EXISTS meeting_summary_session ON meeting_summaries(session_id);
CREATE TABLE IF NOT EXISTS meeting_processing_settings(id INTEGER PRIMARY KEY CHECK(id=1),record TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS meeting_processing_jobs(session_id TEXT PRIMARY KEY REFERENCES live_meetings(id),connection TEXT NOT NULL,generation INTEGER NOT NULL DEFAULT 1,status TEXT NOT NULL,run_id TEXT,error TEXT);
CREATE TABLE IF NOT EXISTS transcription_models(id TEXT PRIMARY KEY,state TEXT NOT NULL,downloaded_bytes INTEGER NOT NULL DEFAULT 0,detail TEXT NOT NULL DEFAULT '');
CREATE TABLE IF NOT EXISTS transcription_audio_grants(route TEXT PRIMARY KEY,active INTEGER NOT NULL);
CREATE TABLE IF NOT EXISTS transcription_receipts(chunk_id TEXT PRIMARY KEY,session_id TEXT NOT NULL REFERENCES live_meetings(id),audio_seconds INTEGER NOT NULL,created_at TEXT NOT NULL,record TEXT NOT NULL);
CREATE INDEX IF NOT EXISTS transcription_receipt_session ON transcription_receipts(session_id,created_at);

CREATE TABLE IF NOT EXISTS transcription_delivery_credentials(chunk_id TEXT PRIMARY KEY REFERENCES transcription_receipts(chunk_id),fingerprint TEXT NOT NULL);
