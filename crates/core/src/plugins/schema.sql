CREATE TABLE IF NOT EXISTS plugins (
 id TEXT PRIMARY KEY,version INTEGER NOT NULL,enabled INTEGER NOT NULL,builtin INTEGER NOT NULL,record TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS plugin_events (
 sequence INTEGER PRIMARY KEY AUTOINCREMENT,id TEXT NOT NULL UNIQUE,schema_version INTEGER NOT NULL DEFAULT 1,
 kind TEXT NOT NULL,record_kind TEXT NOT NULL,record_id TEXT NOT NULL,record_revision INTEGER NOT NULL,
 source_ids TEXT NOT NULL DEFAULT '[]',project_ids TEXT NOT NULL DEFAULT '[]',
 correlation_id TEXT NOT NULL,causation_id TEXT,origin_plugin TEXT,depth INTEGER NOT NULL DEFAULT 0,
 created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE INDEX IF NOT EXISTS plugin_events_record ON plugin_events(record_kind,record_id,sequence);
CREATE TABLE IF NOT EXISTS plugin_deliveries (
 id TEXT PRIMARY KEY,plugin_id TEXT NOT NULL,event_id TEXT NOT NULL,event_sequence INTEGER NOT NULL,
 status TEXT NOT NULL,attempts INTEGER NOT NULL DEFAULT 0,next_attempt_at TEXT NOT NULL,
 error TEXT,created_at TEXT NOT NULL,updated_at TEXT NOT NULL,
 UNIQUE(plugin_id,event_id)
);
CREATE INDEX IF NOT EXISTS plugin_deliveries_ready ON plugin_deliveries(status,next_attempt_at,event_sequence);
CREATE TABLE IF NOT EXISTS plugin_delivery_attempts(id TEXT PRIMARY KEY,delivery_id TEXT NOT NULL,plugin_id TEXT NOT NULL,
 configuration_version INTEGER NOT NULL,package_version TEXT NOT NULL,api_version INTEGER NOT NULL,status TEXT NOT NULL,
 started_at TEXT NOT NULL,finished_at TEXT,error TEXT,configuration_hash TEXT,executable_definition_hash TEXT,grant_record TEXT);
CREATE INDEX IF NOT EXISTS plugin_attempts_delivery ON plugin_delivery_attempts(delivery_id,started_at,id);
CREATE TABLE IF NOT EXISTS plugin_cursors(plugin_id TEXT PRIMARY KEY,sequence INTEGER NOT NULL);
CREATE TABLE IF NOT EXISTS plugin_runs(id TEXT PRIMARY KEY,plugin_id TEXT NOT NULL,status TEXT NOT NULL,created_at TEXT NOT NULL,record TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS plugin_setup_epoch(id INTEGER PRIMARY KEY CHECK(id=1),epoch TEXT NOT NULL);
CREATE INDEX IF NOT EXISTS plugin_runs_ready ON plugin_runs(status,created_at,id);
CREATE TABLE IF NOT EXISTS plugin_eval_checks(id TEXT PRIMARY KEY,plugin_run_id TEXT NOT NULL UNIQUE,eval_run_id TEXT NOT NULL,created_at TEXT NOT NULL,record TEXT NOT NULL);
CREATE INDEX IF NOT EXISTS plugin_eval_checks_run ON plugin_eval_checks(eval_run_id,created_at,id);
CREATE TABLE IF NOT EXISTS plugin_model_proposals(id TEXT PRIMARY KEY,plugin_id TEXT NOT NULL,plugin_run_id TEXT NOT NULL,status TEXT NOT NULL,request_key TEXT NOT NULL,runtime_id TEXT UNIQUE,record TEXT NOT NULL,UNIQUE(plugin_run_id,request_key));
CREATE TABLE IF NOT EXISTS plugin_sources(plugin_id TEXT NOT NULL,installed_at TEXT NOT NULL,snapshot_key TEXT NOT NULL,source_id TEXT NOT NULL UNIQUE,document_id TEXT NOT NULL UNIQUE,PRIMARY KEY(plugin_id,installed_at,snapshot_key));
CREATE TABLE IF NOT EXISTS plugin_selections(capability TEXT PRIMARY KEY,plugin_id TEXT NOT NULL,version INTEGER NOT NULL DEFAULT 1);

-- These triggers are a transactional outbox: a rolled-back domain write cannot
-- deliver a lifecycle event. Existing data is not backfilled implicitly.
CREATE TRIGGER IF NOT EXISTS plugin_document_insert AFTER INSERT ON documents BEGIN
 INSERT INTO plugin_events(id,kind,record_kind,record_id,record_revision,source_ids,correlation_id)
 VALUES(lower(hex(randomblob(16))),'document.revision.created','document',NEW.id,NEW.revision,json_array(NEW.source_id),NEW.id);
END;
CREATE TRIGGER IF NOT EXISTS plugin_document_revision AFTER UPDATE OF revision ON documents WHEN NEW.revision!=OLD.revision BEGIN
 INSERT INTO plugin_events(id,kind,record_kind,record_id,record_revision,source_ids,correlation_id)
 VALUES(lower(hex(randomblob(16))),'document.revision.created','document',NEW.id,NEW.revision,json_array(NEW.source_id),NEW.id);
END;
CREATE TRIGGER IF NOT EXISTS plugin_observation_insert AFTER INSERT ON observations BEGIN
 INSERT INTO plugin_events(id,kind,record_kind,record_id,record_revision,source_ids,correlation_id,origin_plugin,depth)
 VALUES(lower(hex(randomblob(16))),'memory.observation.created','observation',NEW.id,1,NEW.source_ids,
 COALESCE((SELECT json_extract(record,'$.event_id') FROM plugin_effect_context WHERE contributor=NEW.contributor),NEW.id),
 (SELECT json_extract(record,'$.plugin_id') FROM plugin_effect_context WHERE contributor=NEW.contributor),
 COALESCE((SELECT json_extract(record,'$.depth') FROM plugin_effect_context WHERE contributor=NEW.contributor),0));
END;
CREATE TABLE IF NOT EXISTS plugin_effect_context(contributor TEXT PRIMARY KEY,record TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS plugin_hosts(host_id TEXT PRIMARY KEY,token_hash TEXT NOT NULL,expires_at TEXT NOT NULL,record TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS plugin_native_commands(id TEXT PRIMARY KEY,host_id TEXT NOT NULL,provider_id TEXT NOT NULL,provider_version INTEGER NOT NULL,
 capability TEXT NOT NULL,command TEXT NOT NULL,input TEXT NOT NULL,status TEXT NOT NULL,reply TEXT,expires_at TEXT NOT NULL,created_at TEXT NOT NULL);
CREATE INDEX IF NOT EXISTS plugin_native_pending ON plugin_native_commands(host_id,status,created_at);
CREATE TABLE IF NOT EXISTS plugin_native_origins(command_id TEXT PRIMARY KEY,plugin_id TEXT NOT NULL,installed_at TEXT NOT NULL,record TEXT NOT NULL);
CREATE INDEX IF NOT EXISTS plugin_native_plugin ON plugin_native_origins(plugin_id,installed_at);
CREATE TRIGGER IF NOT EXISTS plugin_task_status AFTER UPDATE OF status ON tasks WHEN NEW.status!=OLD.status BEGIN
 INSERT INTO plugin_events(id,kind,record_kind,record_id,record_revision,source_ids,correlation_id)
 VALUES(lower(hex(randomblob(16))),CASE NEW.status WHEN 'accepted' THEN 'task.accepted' WHEN 'completed' THEN 'task.completed' ELSE 'task.changed' END,
 'task',NEW.id,NEW.version,json_array((SELECT source_id FROM documents WHERE id=NEW.document_id)),NEW.id);
END;
CREATE TRIGGER IF NOT EXISTS plugin_recording_state AFTER UPDATE OF record ON live_meetings
 WHEN json_extract(NEW.record,'$.state')!=json_extract(OLD.record,'$.state')
 OR COALESCE(json_extract(NEW.record,'$.error'),'')!=COALESCE(json_extract(OLD.record,'$.error'),'') BEGIN
 INSERT INTO plugin_events(id,kind,record_kind,record_id,record_revision,source_ids,project_ids,correlation_id)
 VALUES(lower(hex(randomblob(16))),
 CASE WHEN json_extract(NEW.record,'$.error') IS NOT NULL THEN 'recording.failed'
 WHEN NEW.state='recording' THEN CASE WHEN json_extract(NEW.record,'$.recorded_ms')>0 THEN 'recording.resumed' ELSE 'recording.started' END
 WHEN NEW.state='paused' THEN 'recording.paused' WHEN NEW.state='stopped' THEN 'recording.stopped' ELSE 'recording.state.changed' END,
 'recording',NEW.id,NEW.version,
 CASE WHEN json_extract(NEW.record,'$.source_id') IS NULL THEN '[]' ELSE json_array(json_extract(NEW.record,'$.source_id')) END,
 CASE WHEN json_extract(NEW.record,'$.project_id')='' THEN '[]' ELSE json_array(json_extract(NEW.record,'$.project_id')) END,NEW.id);
END;
CREATE TRIGGER IF NOT EXISTS plugin_transcript_chunk AFTER INSERT ON live_meeting_chunks BEGIN
 INSERT INTO plugin_events(id,kind,record_kind,record_id,record_revision,source_ids,project_ids,correlation_id)
 SELECT lower(hex(randomblob(16))),'transcript.chunk.committed','recording',NEW.session_id,version,
 CASE WHEN json_extract(record,'$.source_id') IS NULL THEN '[]' ELSE json_array(json_extract(record,'$.source_id')) END,
 CASE WHEN json_extract(record,'$.project_id')='' THEN '[]' ELSE json_array(json_extract(record,'$.project_id')) END,NEW.session_id
 FROM live_meetings WHERE id=NEW.session_id;
END;
CREATE TRIGGER IF NOT EXISTS plugin_runtime_state AFTER UPDATE OF status ON runtime_runs WHEN NEW.status!=OLD.status BEGIN
 INSERT INTO plugin_events(id,kind,record_kind,record_id,record_revision,source_ids,correlation_id)
 VALUES(lower(hex(randomblob(16))),CASE NEW.status WHEN 'running' THEN 'agent.run.started' WHEN 'completed' THEN 'agent.run.completed'
 WHEN 'cancelled' THEN 'agent.run.cancelled' WHEN 'failed' THEN 'agent.run.failed' WHEN 'interrupted' THEN 'agent.run.failed' ELSE 'agent.run.state.changed' END,
 'agent_run',NEW.id,0,COALESCE(json_extract(NEW.record,'$.source_ids'),'[]'),NEW.id);
END;
