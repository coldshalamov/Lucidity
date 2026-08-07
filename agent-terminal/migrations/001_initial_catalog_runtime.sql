CREATE TABLE catalog_metadata (
  id INTEGER PRIMARY KEY CHECK (id = 1),
  schema_version INTEGER NOT NULL,
  snapshot_version INTEGER NOT NULL,
  updated_at TEXT NOT NULL
);

INSERT INTO catalog_metadata(id, schema_version, snapshot_version, updated_at)
VALUES (1, 1, 0, '1970-01-01T00:00:00.000000000Z')
ON CONFLICT(id) DO NOTHING;

CREATE TABLE profiles (
  profile_id TEXT PRIMARY KEY,
  adapter_id TEXT NOT NULL,
  native_profile_id TEXT NOT NULL,
  display_name TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  UNIQUE(adapter_id, native_profile_id)
);

CREATE TABLE conversations (
  conversation_id TEXT PRIMARY KEY,
  adapter_id TEXT NOT NULL,
  profile_id TEXT NOT NULL REFERENCES profiles(profile_id) ON DELETE RESTRICT,
  native_session_id TEXT NOT NULL,
  native_session_path TEXT,
  title TEXT NOT NULL,
  user_alias TEXT,
  project_path TEXT,
  created_at TEXT NOT NULL,
  last_activity_at TEXT NOT NULL,
  organization_state TEXT NOT NULL CHECK (organization_state IN ('active', 'settled')),
  quarantined INTEGER NOT NULL DEFAULT 0 CHECK (quarantined IN (0, 1)),
  quarantine_reason TEXT,
  updated_at TEXT NOT NULL,
  UNIQUE(adapter_id, profile_id, native_session_id)
);

CREATE TABLE conversation_aliases (
  alias TEXT PRIMARY KEY,
  conversation_id TEXT NOT NULL REFERENCES conversations(conversation_id) ON DELETE CASCADE,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE organization_state_history (
  history_id INTEGER PRIMARY KEY AUTOINCREMENT,
  conversation_id TEXT NOT NULL REFERENCES conversations(conversation_id) ON DELETE CASCADE,
  organization_state TEXT NOT NULL CHECK (organization_state IN ('active', 'settled')),
  changed_at TEXT NOT NULL
);

CREATE TABLE runtime_attachments (
  conversation_id TEXT PRIMARY KEY REFERENCES conversations(conversation_id) ON DELETE CASCADE,
  pane_id INTEGER NOT NULL,
  process_id INTEGER,
  process_owner_json TEXT,
  host_instance_id TEXT NOT NULL,
  attachment_generation INTEGER NOT NULL CHECK (attachment_generation >= 1),
  attached_at TEXT NOT NULL,
  stale INTEGER NOT NULL DEFAULT 0 CHECK (stale IN (0, 1)),
  event_resync_required INTEGER NOT NULL DEFAULT 0 CHECK (event_resync_required IN (0, 1)),
  detach_reason TEXT
);

CREATE TABLE runtime_snapshots (
  conversation_id TEXT PRIMARY KEY REFERENCES conversations(conversation_id) ON DELETE CASCADE,
  runtime_state TEXT NOT NULL CHECK (
    runtime_state IN (
      'not_running',
      'starting',
      'working',
      'waiting_for_input',
      'awaiting_approval',
      'completed_idle',
      'failed',
      'unknown_external'
    )
  ),
  source TEXT NOT NULL CHECK (
    source IN (
      'structured_event',
      'provider_api',
      'session_store',
      'submitted_command',
      'screen_observation',
      'user_binding',
      'process_exit',
      'recovery'
    )
  ),
  confidence TEXT NOT NULL CHECK (confidence IN ('authoritative', 'correlated', 'heuristic', 'unknown')),
  observed_at TEXT NOT NULL,
  last_error TEXT
);

CREATE TABLE usage_snapshots (
  conversation_id TEXT PRIMARY KEY REFERENCES conversations(conversation_id) ON DELETE CASCADE,
  observed_at TEXT NOT NULL,
  source TEXT NOT NULL,
  usage_json TEXT NOT NULL,
  stale_on_open_after TEXT NOT NULL,
  last_error TEXT
);

CREATE TABLE agent_event_dedupe (
  adapter_id TEXT NOT NULL,
  profile_id TEXT NOT NULL,
  event_id TEXT NOT NULL,
  conversation_id TEXT REFERENCES conversations(conversation_id) ON DELETE SET NULL,
  observed_at TEXT NOT NULL,
  PRIMARY KEY(adapter_id, profile_id, event_id)
);

CREATE TABLE catalog_recovery_warnings (
  warning_id INTEGER PRIMARY KEY AUTOINCREMENT,
  created_at TEXT NOT NULL,
  warning TEXT NOT NULL
);
