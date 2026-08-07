CREATE TABLE reconciliation_metadata (
  key TEXT PRIMARY KEY,
  value TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE pending_identities (
  host_instance_id TEXT NOT NULL,
  pane_id INTEGER NOT NULL,
  candidate_native_session_id TEXT NOT NULL,
  evidence_id TEXT NOT NULL,
  observed_at TEXT NOT NULL,
  expires_at TEXT NOT NULL,
  PRIMARY KEY(host_instance_id, pane_id)
);

UPDATE catalog_metadata
SET schema_version = 2,
    updated_at = '1970-01-01T00:00:00.000000000Z'
WHERE id = 1;
