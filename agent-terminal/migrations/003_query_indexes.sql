CREATE INDEX idx_conversations_page
  ON conversations(last_activity_at DESC, conversation_id DESC);

CREATE INDEX idx_conversations_native
  ON conversations(adapter_id, profile_id, native_session_id);

CREATE UNIQUE INDEX idx_runtime_current_pane
  ON runtime_attachments(host_instance_id, pane_id)
  WHERE stale = 0;

CREATE INDEX idx_runtime_stale_host
  ON runtime_attachments(host_instance_id, stale);

CREATE INDEX idx_agent_event_conversation
  ON agent_event_dedupe(conversation_id, observed_at);

UPDATE catalog_metadata
SET schema_version = 3,
    updated_at = '1970-01-01T00:00:00.000000000Z'
WHERE id = 1;
