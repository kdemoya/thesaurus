-- Trigger replacement and schema migration run in one writer transaction.
-- Event payloads contain references, never source bodies or resolved secrets.
DROP TRIGGER IF EXISTS plugin_runtime_state;
CREATE TRIGGER plugin_runtime_state AFTER UPDATE OF status ON runtime_runs WHEN NEW.status!=OLD.status BEGIN
 INSERT INTO plugin_events(id,kind,record_kind,record_id,record_revision,source_ids,project_ids,correlation_id,causation_id,origin_plugin,depth)
 SELECT lower(hex(randomblob(16))),CASE NEW.status WHEN 'running' THEN 'agent.run.started' WHEN 'completed' THEN 'agent.run.completed'
 WHEN 'cancelled' THEN 'agent.run.cancelled' WHEN 'failed' THEN 'agent.run.failed' WHEN 'interrupted' THEN 'agent.run.failed' ELSE 'agent.run.state.changed' END,
 'agent_run',NEW.id,0,COALESCE(json_extract(NEW.record,'$.source_ids'),'[]'),COALESCE(json_extract(r.record,'$.project_ids'),'[]'),
 COALESCE(e.correlation_id,NEW.id),e.id,p.plugin_id,CASE WHEN p.id IS NULL THEN 0 ELSE COALESCE(e.depth,0)+1 END
 FROM (SELECT 1) root LEFT JOIN plugin_model_proposals p ON p.runtime_id=NEW.id
 LEFT JOIN plugin_runs r ON r.id=p.plugin_run_id LEFT JOIN plugin_events e ON e.id=json_extract(r.record,'$.event_id');
END;
-- Host-owned transaction context attributes all domain events from a connector
-- import. Subscribers only see the final committed records.
CREATE TRIGGER IF NOT EXISTS plugin_transaction_origin AFTER INSERT ON plugin_events
 WHEN EXISTS(SELECT 1 FROM plugin_effect_context WHERE contributor='__host_transaction__') BEGIN
 UPDATE plugin_events SET
 origin_plugin=(SELECT json_extract(record,'$.plugin_id') FROM plugin_effect_context WHERE contributor='__host_transaction__'),
 causation_id=(SELECT json_extract(record,'$.event_id') FROM plugin_effect_context WHERE contributor='__host_transaction__'),
 correlation_id=(SELECT json_extract(record,'$.correlation_id') FROM plugin_effect_context WHERE contributor='__host_transaction__'),
 depth=(SELECT json_extract(record,'$.depth') FROM plugin_effect_context WHERE contributor='__host_transaction__')
 WHERE id=NEW.id;
END;
DROP TRIGGER IF EXISTS plugin_document_insert;
DROP TRIGGER IF EXISTS plugin_document_revision;
CREATE TRIGGER plugin_document_insert AFTER INSERT ON documents BEGIN
 INSERT INTO plugin_events(id,kind,record_kind,record_id,record_revision,source_ids,project_ids,correlation_id)
 SELECT lower(hex(randomblob(16))),'document.revision.created','document',NEW.id,NEW.revision,json_array(NEW.source_id),
 (SELECT json_group_array(p.id) FROM project_briefs p WHERE EXISTS(SELECT 1 FROM json_each(p.source_ids) s WHERE s.value=NEW.source_id)),NEW.id;
END;
CREATE TRIGGER plugin_document_revision AFTER UPDATE OF revision ON documents WHEN NEW.revision!=OLD.revision BEGIN
 INSERT INTO plugin_events(id,kind,record_kind,record_id,record_revision,source_ids,project_ids,correlation_id)
 SELECT lower(hex(randomblob(16))),'document.revision.created','document',NEW.id,NEW.revision,json_array(NEW.source_id),
 (SELECT json_group_array(p.id) FROM project_briefs p WHERE EXISTS(SELECT 1 FROM json_each(p.source_ids) s WHERE s.value=NEW.source_id)),NEW.id;
END;
DROP TRIGGER IF EXISTS plugin_observation_insert;
CREATE TRIGGER plugin_observation_insert AFTER INSERT ON observations BEGIN
 INSERT INTO plugin_events(id,kind,record_kind,record_id,record_revision,source_ids,correlation_id,causation_id,origin_plugin,depth)
 VALUES(lower(hex(randomblob(16))),'memory.observation.created','observation',NEW.id,1,NEW.source_ids,
 COALESCE((SELECT json_extract(record,'$.correlation_id') FROM plugin_effect_context WHERE contributor=NEW.contributor),NEW.id),
 (SELECT json_extract(record,'$.event_id') FROM plugin_effect_context WHERE contributor=NEW.contributor),
 (SELECT json_extract(record,'$.plugin_id') FROM plugin_effect_context WHERE contributor=NEW.contributor),
 COALESCE((SELECT json_extract(record,'$.depth') FROM plugin_effect_context WHERE contributor=NEW.contributor),0));
END;
DROP TRIGGER IF EXISTS plugin_recording_state;
CREATE TRIGGER plugin_recording_state AFTER UPDATE OF record ON live_meetings
 WHEN json_extract(NEW.record,'$.state')!=json_extract(OLD.record,'$.state')
 OR COALESCE(json_extract(NEW.record,'$.error'),'')!=COALESCE(json_extract(OLD.record,'$.error'),'') BEGIN
 INSERT INTO plugin_events(id,kind,record_kind,record_id,record_revision,source_ids,project_ids,correlation_id)
 VALUES(lower(hex(randomblob(16))),
 CASE WHEN json_extract(NEW.record,'$.error') IS NOT NULL THEN 'recording.failed'
 WHEN NEW.state='recording' THEN CASE WHEN json_extract(NEW.record,'$.recorded_ms')>0 OR EXISTS(SELECT 1 FROM plugin_events e WHERE e.record_kind='recording' AND e.record_id=NEW.id AND e.kind IN ('recording.started','recording.resumed')) THEN 'recording.resumed' ELSE 'recording.started' END
 WHEN NEW.state='paused' THEN 'recording.paused' WHEN NEW.state='stopped' THEN 'recording.stopped' ELSE 'recording.state.changed' END,
 'recording',NEW.id,NEW.version,
 CASE WHEN json_extract(NEW.record,'$.source_id') IS NULL THEN '[]' ELSE json_array(json_extract(NEW.record,'$.source_id')) END,
 CASE WHEN COALESCE(json_extract(NEW.record,'$.project_id'),'')='' THEN '[]' ELSE json_array(json_extract(NEW.record,'$.project_id')) END,NEW.id);
END;

CREATE TRIGGER IF NOT EXISTS plugin_source_disclosure AFTER UPDATE OF allow_external ON sources WHEN NEW.allow_external!=OLD.allow_external BEGIN
 INSERT INTO plugin_events(id,kind,record_kind,record_id,record_revision,source_ids,correlation_id)
 VALUES(lower(hex(randomblob(16))),'source.policy.changed','source',NEW.id,0,json_array(NEW.id),NEW.id);
END;
CREATE TRIGGER IF NOT EXISTS plugin_source_policy_insert AFTER INSERT ON source_control_policy BEGIN
 INSERT INTO plugin_events(id,kind,record_kind,record_id,record_revision,source_ids,correlation_id)
 VALUES(lower(hex(randomblob(16))),'source.policy.changed','source',NEW.source_id,COALESCE(json_extract(NEW.record,'$.version'),1),json_array(NEW.source_id),NEW.source_id);
END;
CREATE TRIGGER IF NOT EXISTS plugin_source_policy_update AFTER UPDATE OF record ON source_control_policy WHEN NEW.record!=OLD.record BEGIN
 INSERT INTO plugin_events(id,kind,record_kind,record_id,record_revision,source_ids,correlation_id)
 VALUES(lower(hex(randomblob(16))),'source.policy.changed','source',NEW.source_id,COALESCE(json_extract(NEW.record,'$.version'),1),json_array(NEW.source_id),NEW.source_id);
END;
CREATE TRIGGER IF NOT EXISTS plugin_retrieval_policy_insert AFTER INSERT ON retrieval_policies BEGIN
 INSERT INTO plugin_events(id,kind,record_kind,record_id,record_revision,source_ids,correlation_id)
 VALUES(lower(hex(randomblob(16))),'source.policy.changed','source',NEW.source_id,0,json_array(NEW.source_id),NEW.source_id);
END;
CREATE TRIGGER IF NOT EXISTS plugin_retrieval_policy_update AFTER UPDATE OF policy ON retrieval_policies WHEN NEW.policy!=OLD.policy BEGIN
 INSERT INTO plugin_events(id,kind,record_kind,record_id,record_revision,source_ids,correlation_id)
 VALUES(lower(hex(randomblob(16))),'source.policy.changed','source',NEW.source_id,0,json_array(NEW.source_id),NEW.source_id);
END;
CREATE TRIGGER IF NOT EXISTS plugin_source_fetch AFTER UPDATE OF status ON retrieval_fetches
 WHEN NEW.status!=OLD.status AND NEW.status IN ('completed','failed','interrupted') BEGIN
 INSERT INTO plugin_events(id,kind,record_kind,record_id,record_revision,source_ids,project_ids,correlation_id)
 SELECT lower(hex(randomblob(16))),CASE NEW.status WHEN 'completed' THEN 'source.sync.completed' ELSE 'source.sync.failed' END,
 'source_fetch',NEW.id,COALESCE(NEW.revision,0),json_array(u.source_id),json_array(u.project_id),NEW.id FROM retrieval_upstreams u WHERE u.id=NEW.upstream_id;
END;
CREATE TRIGGER IF NOT EXISTS plugin_memory_superseded AFTER INSERT ON memory_review_revisions BEGIN
 INSERT INTO plugin_events(id,kind,record_kind,record_id,record_revision,source_ids,correlation_id)
 SELECT lower(hex(randomblob(16))),'memory.observation.superseded','observation',NEW.observation_id,NEW.version,o.source_ids,NEW.observation_id FROM observations o WHERE o.id=NEW.observation_id;
END;
CREATE TRIGGER IF NOT EXISTS plugin_priority_insert AFTER INSERT ON priority_preferences BEGIN
 INSERT INTO plugin_events(id,kind,record_kind,record_id,record_revision,source_ids,project_ids,correlation_id)
 SELECT lower(hex(randomblob(16))),'priority.changed','task',NEW.task_id,COALESCE(json_extract(NEW.record,'$.version'),1),json_array(d.source_id),json_array(NEW.project_id),NEW.task_id FROM tasks t JOIN documents d ON d.id=t.document_id WHERE t.id=NEW.task_id;
END;
CREATE TRIGGER IF NOT EXISTS plugin_priority_update AFTER UPDATE OF record ON priority_preferences WHEN NEW.record!=OLD.record BEGIN
 INSERT INTO plugin_events(id,kind,record_kind,record_id,record_revision,source_ids,project_ids,correlation_id)
 SELECT lower(hex(randomblob(16))),'priority.changed','task',NEW.task_id,COALESCE(json_extract(NEW.record,'$.version'),1),json_array(d.source_id),json_array(NEW.project_id),NEW.task_id FROM tasks t JOIN documents d ON d.id=t.document_id WHERE t.id=NEW.task_id;
END;
CREATE TRIGGER IF NOT EXISTS plugin_priority_review AFTER INSERT ON priority_reviews BEGIN
 INSERT INTO plugin_events(id,kind,record_kind,record_id,record_revision,source_ids,project_ids,correlation_id)
 SELECT lower(hex(randomblob(16))),'priority.changed','priority_review',NEW.id,NEW.sequence,p.source_ids,json_array(NEW.project_id),NEW.id FROM project_briefs p WHERE p.id=NEW.project_id;
END;
CREATE TRIGGER IF NOT EXISTS plugin_thread_context AFTER INSERT ON thread_contexts BEGIN
 INSERT INTO plugin_events(id,kind,record_kind,record_id,record_revision,source_ids,project_ids,correlation_id)
 VALUES(lower(hex(randomblob(16))),'context.bundle.created','thread_context',NEW.id,1,
 (SELECT json_group_array(json_extract(s.value,'$.source_id')) FROM json_each(NEW.record,'$.sources') s),json_array(json_extract(NEW.record,'$.project_id')),NEW.id);
END;
CREATE TRIGGER IF NOT EXISTS plugin_handoff_insert AFTER INSERT ON handoff_receipts WHEN json_extract(NEW.record,'$.state')='acknowledged' BEGIN
 INSERT INTO plugin_events(id,kind,record_kind,record_id,record_revision,source_ids,project_ids,correlation_id)
 SELECT lower(hex(randomblob(16))),'handoff.acknowledged','handoff',NEW.context_id,NEW.version,
 (SELECT json_group_array(json_extract(s.value,'$.source_id')) FROM json_each(c.record,'$.sources') s),json_array(json_extract(c.record,'$.project_id')),NEW.context_id FROM thread_contexts c WHERE c.id=NEW.context_id;
END;
CREATE TRIGGER IF NOT EXISTS plugin_handoff_update AFTER UPDATE OF record ON handoff_receipts
 WHEN json_extract(NEW.record,'$.state')='acknowledged' AND json_extract(OLD.record,'$.state')!='acknowledged' BEGIN
 INSERT INTO plugin_events(id,kind,record_kind,record_id,record_revision,source_ids,project_ids,correlation_id)
 SELECT lower(hex(randomblob(16))),'handoff.acknowledged','handoff',NEW.context_id,NEW.version,
 (SELECT json_group_array(json_extract(s.value,'$.source_id')) FROM json_each(c.record,'$.sources') s),json_array(json_extract(c.record,'$.project_id')),NEW.context_id FROM thread_contexts c WHERE c.id=NEW.context_id;
END;
CREATE TRIGGER IF NOT EXISTS plugin_eval_run AFTER INSERT ON eval_runs BEGIN
 INSERT INTO plugin_events(id,kind,record_kind,record_id,record_revision,source_ids,project_ids,correlation_id)
 VALUES(lower(hex(randomblob(16))),'eval.run.completed','eval_run',NEW.id,1,COALESCE(json_extract(NEW.record,'$.source_ids'),'[]'),json_array(NEW.project_id),NEW.id);
END;
