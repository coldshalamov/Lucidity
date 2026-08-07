//! SQLite-backed catalog and runtime persistence for the native host.

use agent_protocol::{
    AdapterId, AttachmentVersion, CatalogCursor, Confidence, ConversationId, ConversationPage,
    ConversationRecord, HostInstanceId, NativeConversationKey, ObservationSource,
    OrganizationState, PaneId, ProcessExitObservation, ProcessOwnerId, ProfileId,
    RequestedTermination, RuntimeAttachment, RuntimeSnapshot, RuntimeState,
};
use anyhow::{anyhow, bail, Context, Result};
use chrono::{DateTime, SecondsFormat, Utc};
use rusqlite::types::Type;
use rusqlite::{params, Connection, OptionalExtension, Row, Transaction};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::cmp::max;
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

pub const CURRENT_SCHEMA_VERSION: u32 = 3;
pub const MAX_CATALOG_PAGE_LIMIT: u32 = 500;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Migration {
    pub version: u32,
    pub name: &'static str,
    pub sql: &'static str,
}

pub const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        name: "001_initial_catalog_runtime",
        sql: include_str!("../../migrations/001_initial_catalog_runtime.sql"),
    },
    Migration {
        version: 2,
        name: "002_reconciliation_metadata",
        sql: include_str!("../../migrations/002_reconciliation_metadata.sql"),
    },
    Migration {
        version: 3,
        name: "003_query_indexes",
        sql: include_str!("../../migrations/003_query_indexes.sql"),
    },
];

#[derive(Debug)]
pub struct CatalogStore {
    conn: Connection,
}

#[derive(Clone, Debug)]
pub struct ConversationUpsert {
    pub id: Option<ConversationId>,
    pub native: NativeConversationKey,
    pub native_session_path: Option<PathBuf>,
    pub title: String,
    pub user_alias: Option<String>,
    pub project_path: Option<PathBuf>,
    pub created_at: DateTime<Utc>,
    pub last_activity_at: DateTime<Utc>,
    pub organization_state: OrganizationState,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachmentInput {
    pub conversation_id: ConversationId,
    pub pane_id: PaneId,
    pub process_id: Option<u32>,
    pub process_owner: Option<ProcessOwnerId>,
    pub attached_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RebindResult {
    pub detached: AttachmentVersion,
    pub attached: RuntimeAttachment,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StartupReconciliation {
    pub stale_attachments_invalidated: usize,
}

#[derive(Debug)]
pub struct RecoveryOpen {
    pub store: CatalogStore,
    pub quarantined_path: Option<PathBuf>,
    pub warning: Option<String>,
}

impl ConversationUpsert {
    pub fn new(
        native: NativeConversationKey,
        title: impl Into<String>,
        now: DateTime<Utc>,
    ) -> Self {
        Self {
            id: None,
            native,
            native_session_path: None,
            title: title.into(),
            user_alias: None,
            project_path: None,
            created_at: now,
            last_activity_at: now,
            organization_state: OrganizationState::Active,
        }
    }
}

impl CatalogStore {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let mut store = Self {
            conn: Connection::open(path.as_ref())
                .with_context(|| format!("open catalog database {}", path.as_ref().display()))?,
        };
        store.configure()?;
        store.apply_migrations()?;
        Ok(store)
    }

    pub fn open_or_rebuild(path: impl AsRef<Path>) -> Result<RecoveryOpen> {
        let path = path.as_ref();
        match Self::open(path) {
            Ok(store) => Ok(RecoveryOpen {
                store,
                quarantined_path: None,
                warning: None,
            }),
            Err(error) => {
                let quarantined_path = quarantine_catalog_file(path)
                    .with_context(|| format!("quarantine catalog {}", path.display()))?;
                let mut store = Self::open(path)?;
                let warning = format!(
                    "catalog rebuilt after open/migration failure; preserved {}: {error:#}",
                    quarantined_path.display()
                );
                store.record_recovery_warning(&warning)?;
                Ok(RecoveryOpen {
                    store,
                    quarantined_path: Some(quarantined_path),
                    warning: Some(warning),
                })
            }
        }
    }

    pub fn in_memory() -> Result<Self> {
        let mut store = Self {
            conn: Connection::open_in_memory()?,
        };
        store.configure()?;
        store.apply_migrations()?;
        Ok(store)
    }

    pub fn connection(&self) -> &Connection {
        &self.conn
    }

    fn configure(&mut self) -> Result<()> {
        self.conn.pragma_update(None, "foreign_keys", "ON")?;
        self.conn.pragma_update(None, "journal_mode", "WAL")?;
        self.conn.pragma_update(None, "busy_timeout", 5_000u32)?;
        Ok(())
    }

    pub fn apply_migrations(&mut self) -> Result<()> {
        self.conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS schema_migrations (
              version INTEGER PRIMARY KEY,
              name TEXT NOT NULL,
              checksum TEXT NOT NULL,
              applied_at TEXT NOT NULL
            );
            "#,
        )?;

        let tx = self.conn.transaction()?;
        for migration in MIGRATIONS {
            let existing: Option<String> = tx
                .query_row(
                    "SELECT checksum FROM schema_migrations WHERE version = ?1",
                    params![migration.version],
                    |row| row.get(0),
                )
                .optional()?;
            let checksum = checksum_hex(migration.sql.as_bytes());
            match existing {
                Some(found) if found == checksum => continue,
                Some(found) => bail!(
                    "migration {} checksum mismatch: database has {}, source has {}",
                    migration.version,
                    found,
                    checksum
                ),
                None => {
                    tx.execute_batch(migration.sql)
                        .with_context(|| format!("apply migration {}", migration.name))?;
                    tx.execute(
                        "INSERT INTO schema_migrations(version, name, checksum, applied_at)
                         VALUES (?1, ?2, ?3, ?4)",
                        params![
                            migration.version,
                            migration.name,
                            checksum,
                            timestamp(Utc::now())
                        ],
                    )?;
                }
            }
        }
        tx.pragma_update(None, "user_version", CURRENT_SCHEMA_VERSION)?;
        tx.commit()?;
        Ok(())
    }

    pub fn applied_migrations(&self) -> Result<Vec<(u32, String, String)>> {
        let mut stmt = self.conn.prepare(
            "SELECT version, name, checksum FROM schema_migrations ORDER BY version ASC",
        )?;
        let rows = stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    pub fn catalog_snapshot_version(&self) -> Result<u64> {
        self.conn
            .query_row(
                "SELECT snapshot_version FROM catalog_metadata WHERE id = 1",
                [],
                |row| row.get::<_, i64>(0),
            )
            .map(|value| value as u64)
            .map_err(Into::into)
    }

    pub fn ensure_profile(
        &mut self,
        adapter_id: &AdapterId,
        profile_id: ProfileId,
        display_name: Option<&str>,
        now: DateTime<Utc>,
    ) -> Result<()> {
        let tx = self.conn.transaction()?;
        ensure_profile_tx(&tx, adapter_id, profile_id, display_name, now)?;
        tx.commit()?;
        Ok(())
    }

    pub fn upsert_conversation(&mut self, input: ConversationUpsert) -> Result<ConversationRecord> {
        let tx = self.conn.transaction()?;
        ensure_profile_tx(
            &tx,
            &input.native.adapter_id,
            input.native.profile_id,
            None,
            input.created_at,
        )?;

        let existing = tx
            .query_row(
                r#"
                SELECT conversation_id, created_at, last_activity_at, title, user_alias,
                       project_path, native_session_path, organization_state
                FROM conversations
                WHERE adapter_id = ?1 AND profile_id = ?2 AND native_session_id = ?3
                "#,
                params![
                    input.native.adapter_id.0,
                    input.native.profile_id.to_string(),
                    input.native.native_session_id
                ],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, Option<String>>(4)?,
                        row.get::<_, Option<String>>(5)?,
                        row.get::<_, Option<String>>(6)?,
                        row.get::<_, String>(7)?,
                    ))
                },
            )
            .optional()?;

        let now = Utc::now();
        let conversation_id = existing
            .as_ref()
            .map(|row| parse_conversation_id(&row.0))
            .transpose()?
            .or(input.id)
            .unwrap_or_else(|| ConversationId(Uuid::new_v4()));
        let created_at = existing
            .as_ref()
            .map(|row| parse_datetime(&row.1))
            .transpose()?
            .unwrap_or(input.created_at);
        let existing_last_activity = existing
            .as_ref()
            .map(|row| parse_datetime(&row.2))
            .transpose()?
            .unwrap_or(input.last_activity_at);
        let last_activity_at = max_datetime(existing_last_activity, input.last_activity_at);
        let title = if input.title.is_empty() {
            existing
                .as_ref()
                .map(|row| row.3.clone())
                .unwrap_or_else(|| input.native.native_session_id.clone())
        } else {
            input.title
        };
        let user_alias = input
            .user_alias
            .or_else(|| existing.as_ref().and_then(|row| row.4.clone()));
        let project_path = path_to_db(input.project_path.as_deref())
            .or_else(|| existing.as_ref().and_then(|row| row.5.clone()));
        let native_session_path = path_to_db(input.native_session_path.as_deref())
            .or_else(|| existing.as_ref().and_then(|row| row.6.clone()));
        let organization_state = input.organization_state;

        tx.execute(
            r#"
            INSERT INTO conversations (
              conversation_id, adapter_id, profile_id, native_session_id, native_session_path,
              title, user_alias, project_path, created_at, last_activity_at,
              organization_state, quarantined, quarantine_reason, updated_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, 0, NULL, ?12)
            ON CONFLICT(adapter_id, profile_id, native_session_id) DO UPDATE SET
              native_session_path = excluded.native_session_path,
              title = excluded.title,
              user_alias = excluded.user_alias,
              project_path = excluded.project_path,
              last_activity_at = excluded.last_activity_at,
              organization_state = excluded.organization_state,
              updated_at = excluded.updated_at
            "#,
            params![
                conversation_id.to_string(),
                input.native.adapter_id.0,
                input.native.profile_id.to_string(),
                input.native.native_session_id,
                native_session_path,
                title,
                user_alias,
                project_path,
                timestamp(created_at),
                timestamp(last_activity_at),
                organization_state_to_db(organization_state),
                timestamp(now)
            ],
        )?;

        if let Some(alias) = &user_alias {
            tx.execute(
                r#"
                INSERT INTO conversation_aliases(conversation_id, alias, created_at, updated_at)
                VALUES (?1, ?2, ?3, ?3)
                ON CONFLICT(alias) DO UPDATE SET
                  conversation_id = excluded.conversation_id,
                  updated_at = excluded.updated_at
                "#,
                params![conversation_id.to_string(), alias, timestamp(now)],
            )?;
        }

        bump_catalog_snapshot_tx(&tx, now)?;
        tx.commit()?;
        self.get_conversation(conversation_id)?
            .ok_or_else(|| anyhow!("conversation disappeared after upsert"))
    }

    pub fn get_conversation(
        &self,
        conversation_id: ConversationId,
    ) -> Result<Option<ConversationRecord>> {
        self.conn
            .query_row(
                r#"
                SELECT conversation_id, adapter_id, profile_id, native_session_id,
                       native_session_path, title, user_alias, project_path,
                       created_at, last_activity_at, organization_state
                FROM conversations
                WHERE conversation_id = ?1 AND quarantined = 0
                "#,
                params![conversation_id.to_string()],
                row_to_conversation,
            )
            .optional()
            .map_err(Into::into)
    }

    pub fn conversation_by_native(
        &self,
        native: &NativeConversationKey,
    ) -> Result<Option<ConversationRecord>> {
        self.conn
            .query_row(
                r#"
                SELECT conversation_id, adapter_id, profile_id, native_session_id,
                       native_session_path, title, user_alias, project_path,
                       created_at, last_activity_at, organization_state
                FROM conversations
                WHERE adapter_id = ?1 AND profile_id = ?2 AND native_session_id = ?3
                  AND quarantined = 0
                "#,
                params![
                    native.adapter_id.0,
                    native.profile_id.to_string(),
                    native.native_session_id
                ],
                row_to_conversation,
            )
            .optional()
            .map_err(Into::into)
    }

    pub fn list_conversations(
        &self,
        cursor: Option<CatalogCursor>,
        limit: u32,
    ) -> Result<ConversationPage> {
        let snapshot_version = self.catalog_snapshot_version()?;
        if cursor
            .as_ref()
            .is_some_and(|cursor| cursor.catalog_snapshot_version != snapshot_version)
        {
            return Ok(ConversationPage {
                catalog_snapshot_version: snapshot_version,
                conversations: Vec::new(),
                next_cursor: None,
                resync_required: true,
            });
        }

        let limit = limit.clamp(1, MAX_CATALOG_PAGE_LIMIT);
        let mut conversations = Vec::new();
        if let Some(cursor) = cursor {
            let mut stmt = self.conn.prepare(
                r#"
                SELECT conversation_id, adapter_id, profile_id, native_session_id,
                       native_session_path, title, user_alias, project_path,
                       created_at, last_activity_at, organization_state
                FROM conversations
                WHERE quarantined = 0
                  AND (
                    last_activity_at < ?1
                    OR (last_activity_at = ?1 AND conversation_id < ?2)
                  )
                ORDER BY last_activity_at DESC, conversation_id DESC
                LIMIT ?3
                "#,
            )?;
            let rows = stmt.query_map(
                params![
                    timestamp(cursor.last_activity_at),
                    cursor.conversation_id.to_string(),
                    limit
                ],
                row_to_conversation,
            )?;
            for row in rows {
                conversations.push(row?);
            }
        } else {
            let mut stmt = self.conn.prepare(
                r#"
                SELECT conversation_id, adapter_id, profile_id, native_session_id,
                       native_session_path, title, user_alias, project_path,
                       created_at, last_activity_at, organization_state
                FROM conversations
                WHERE quarantined = 0
                ORDER BY last_activity_at DESC, conversation_id DESC
                LIMIT ?1
                "#,
            )?;
            let rows = stmt.query_map(params![limit], row_to_conversation)?;
            for row in rows {
                conversations.push(row?);
            }
        }

        let next_cursor = if conversations.len() == limit as usize {
            conversations.last().map(|last| CatalogCursor {
                catalog_snapshot_version: snapshot_version,
                last_activity_at: last.last_activity_at,
                conversation_id: last.id,
            })
        } else {
            None
        };

        Ok(ConversationPage {
            catalog_snapshot_version: snapshot_version,
            conversations,
            next_cursor,
            resync_required: false,
        })
    }

    pub fn set_organization_state(
        &mut self,
        conversation_id: ConversationId,
        organization_state: OrganizationState,
        changed_at: DateTime<Utc>,
    ) -> Result<bool> {
        let tx = self.conn.transaction()?;
        let current: Option<String> = tx
            .query_row(
                "SELECT organization_state FROM conversations WHERE conversation_id = ?1",
                params![conversation_id.to_string()],
                |row| row.get(0),
            )
            .optional()?;
        let Some(current) = current else {
            return Ok(false);
        };
        if organization_state_from_db(&current)? == organization_state {
            tx.commit()?;
            return Ok(false);
        }
        tx.execute(
            "UPDATE conversations SET organization_state = ?2, updated_at = ?3 WHERE conversation_id = ?1",
            params![
                conversation_id.to_string(),
                organization_state_to_db(organization_state),
                timestamp(changed_at)
            ],
        )?;
        tx.execute(
            "INSERT INTO organization_state_history(conversation_id, organization_state, changed_at)
             VALUES (?1, ?2, ?3)",
            params![
                conversation_id.to_string(),
                organization_state_to_db(organization_state),
                timestamp(changed_at)
            ],
        )?;
        bump_catalog_snapshot_tx(&tx, changed_at)?;
        tx.commit()?;
        Ok(true)
    }

    pub fn touch_conversation(
        &mut self,
        conversation_id: ConversationId,
        last_activity_at: DateTime<Utc>,
    ) -> Result<()> {
        let tx = self.conn.transaction()?;
        tx.execute(
            r#"
            UPDATE conversations
            SET last_activity_at =
              CASE WHEN last_activity_at < ?2 THEN ?2 ELSE last_activity_at END,
              updated_at = ?2
            WHERE conversation_id = ?1
            "#,
            params![conversation_id.to_string(), timestamp(last_activity_at)],
        )?;
        bump_catalog_snapshot_tx(&tx, last_activity_at)?;
        tx.commit()?;
        Ok(())
    }

    pub fn register_attachment(
        &mut self,
        host_instance_id: HostInstanceId,
        input: AttachmentInput,
    ) -> Result<RuntimeAttachment> {
        let tx = self.conn.transaction()?;
        ensure_conversation_exists_tx(&tx, input.conversation_id)?;
        let process_owner_json = input
            .process_owner
            .as_ref()
            .map(serde_json::to_string)
            .transpose()?;
        let next_generation = next_generation_tx(&tx, host_instance_id)?;

        let replaced_by_pane =
            current_attachment_for_pane_tx(&tx, host_instance_id, input.pane_id)?;
        if let Some(existing) = replaced_by_pane {
            if existing.conversation_id != input.conversation_id {
                mark_attachment_stale_tx(
                    &tx,
                    existing.conversation_id,
                    "DuplicatePaneBindingReplaced",
                    input.attached_at,
                )?;
                upsert_runtime_snapshot_tx(
                    &tx,
                    RuntimeSnapshot {
                        conversation_id: existing.conversation_id,
                        state: RuntimeState::NotRunning,
                        source: ObservationSource::Recovery,
                        confidence: Confidence::Authoritative,
                        observed_at: input.attached_at,
                        last_error: Some("duplicate pane binding replaced".to_owned()),
                    },
                )?;
            }
        }

        tx.execute(
            r#"
            INSERT INTO runtime_attachments (
              conversation_id, pane_id, process_id, process_owner_json, host_instance_id,
              attachment_generation, attached_at, stale, event_resync_required, detach_reason
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 0, 0, NULL)
            ON CONFLICT(conversation_id) DO UPDATE SET
              pane_id = excluded.pane_id,
              process_id = excluded.process_id,
              process_owner_json = excluded.process_owner_json,
              host_instance_id = excluded.host_instance_id,
              attachment_generation = excluded.attachment_generation,
              attached_at = excluded.attached_at,
              stale = 0,
              event_resync_required = 0,
              detach_reason = NULL
            "#,
            params![
                input.conversation_id.to_string(),
                input.pane_id.0 as i64,
                input.process_id.map(i64::from),
                process_owner_json,
                host_instance_id.to_string(),
                next_generation as i64,
                timestamp(input.attached_at)
            ],
        )?;
        upsert_runtime_snapshot_tx(
            &tx,
            RuntimeSnapshot {
                conversation_id: input.conversation_id,
                state: RuntimeState::Starting,
                source: ObservationSource::ProviderApi,
                confidence: Confidence::Authoritative,
                observed_at: input.attached_at,
                last_error: None,
            },
        )?;
        tx.commit()?;

        Ok(RuntimeAttachment {
            conversation_id: input.conversation_id,
            pane_id: input.pane_id,
            process_id: input.process_id,
            process_owner: input.process_owner,
            host_instance_id,
            attachment_generation: next_generation,
            attached_at: input.attached_at,
        })
    }

    pub fn current_attachment(
        &self,
        conversation_id: ConversationId,
    ) -> Result<Option<RuntimeAttachment>> {
        self.conn
            .query_row(
                r#"
                SELECT conversation_id, pane_id, process_id, process_owner_json,
                       host_instance_id, attachment_generation, attached_at
                FROM runtime_attachments
                WHERE conversation_id = ?1 AND stale = 0
                "#,
                params![conversation_id.to_string()],
                row_to_attachment,
            )
            .optional()
            .map_err(Into::into)
    }

    pub fn current_attachment_for_pane(
        &self,
        host_instance_id: HostInstanceId,
        pane_id: PaneId,
    ) -> Result<Option<RuntimeAttachment>> {
        current_attachment_for_pane_tx_unchecked(&self.conn, host_instance_id, pane_id)
    }

    pub fn detach_attachment(
        &mut self,
        conversation_id: ConversationId,
        version: AttachmentVersion,
        reason: &str,
        observed_at: DateTime<Utc>,
    ) -> Result<bool> {
        let tx = self.conn.transaction()?;
        let current = current_attachment_tx(&tx, conversation_id)?;
        let Some(current) = current else {
            tx.commit()?;
            return Ok(false);
        };
        if current.host_instance_id != version.host_instance_id
            || current.attachment_generation != version.attachment_generation
        {
            tx.commit()?;
            return Ok(false);
        }
        mark_attachment_stale_tx(&tx, conversation_id, reason, observed_at)?;
        upsert_runtime_snapshot_tx(
            &tx,
            RuntimeSnapshot {
                conversation_id,
                state: RuntimeState::NotRunning,
                source: ObservationSource::Recovery,
                confidence: Confidence::Authoritative,
                observed_at,
                last_error: Some(reason.to_owned()),
            },
        )?;
        tx.commit()?;
        Ok(true)
    }

    pub fn rebind_attachment(
        &mut self,
        from_conversation_id: ConversationId,
        to_conversation_id: ConversationId,
        version: AttachmentVersion,
        observed_at: DateTime<Utc>,
    ) -> Result<Option<RebindResult>> {
        let tx = self.conn.transaction()?;
        ensure_conversation_exists_tx(&tx, to_conversation_id)?;
        let Some(current) = current_attachment_tx(&tx, from_conversation_id)? else {
            tx.commit()?;
            return Ok(None);
        };
        if current.host_instance_id != version.host_instance_id
            || current.attachment_generation != version.attachment_generation
        {
            tx.commit()?;
            return Ok(None);
        }

        mark_attachment_stale_tx(
            &tx,
            from_conversation_id,
            "NativeIdentitySuperseded",
            observed_at,
        )?;
        upsert_runtime_snapshot_tx(
            &tx,
            RuntimeSnapshot {
                conversation_id: from_conversation_id,
                state: RuntimeState::NotRunning,
                source: ObservationSource::Recovery,
                confidence: Confidence::Authoritative,
                observed_at,
                last_error: Some("native identity superseded".to_owned()),
            },
        )?;

        if let Some(existing_to) = current_attachment_tx(&tx, to_conversation_id)? {
            mark_attachment_stale_tx(
                &tx,
                existing_to.conversation_id,
                "NativeIdentityRebindTargetReplaced",
                observed_at,
            )?;
        }

        let next_generation = max(
            current.attachment_generation + 1,
            next_generation_tx(&tx, version.host_instance_id)?,
        );
        let process_owner_json = current
            .process_owner
            .as_ref()
            .map(serde_json::to_string)
            .transpose()?;
        tx.execute(
            r#"
            INSERT INTO runtime_attachments (
              conversation_id, pane_id, process_id, process_owner_json, host_instance_id,
              attachment_generation, attached_at, stale, event_resync_required, detach_reason
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 0, 0, NULL)
            ON CONFLICT(conversation_id) DO UPDATE SET
              pane_id = excluded.pane_id,
              process_id = excluded.process_id,
              process_owner_json = excluded.process_owner_json,
              host_instance_id = excluded.host_instance_id,
              attachment_generation = excluded.attachment_generation,
              attached_at = excluded.attached_at,
              stale = 0,
              event_resync_required = 0,
              detach_reason = NULL
            "#,
            params![
                to_conversation_id.to_string(),
                current.pane_id.0 as i64,
                current.process_id.map(i64::from),
                process_owner_json,
                version.host_instance_id.to_string(),
                next_generation as i64,
                timestamp(observed_at)
            ],
        )?;
        upsert_runtime_snapshot_tx(
            &tx,
            RuntimeSnapshot {
                conversation_id: to_conversation_id,
                state: RuntimeState::Working,
                source: ObservationSource::StructuredEvent,
                confidence: Confidence::Authoritative,
                observed_at,
                last_error: None,
            },
        )?;
        tx.commit()?;

        Ok(Some(RebindResult {
            detached: version,
            attached: RuntimeAttachment {
                conversation_id: to_conversation_id,
                pane_id: current.pane_id,
                process_id: current.process_id,
                process_owner: current.process_owner,
                host_instance_id: version.host_instance_id,
                attachment_generation: next_generation,
                attached_at: observed_at,
            },
        }))
    }

    pub fn startup_reconcile(
        &mut self,
        host_instance_id: HostInstanceId,
        observed_at: DateTime<Utc>,
    ) -> Result<StartupReconciliation> {
        let tx = self.conn.transaction()?;
        let stale = collect_current_attachments_not_host_tx(&tx, host_instance_id)?;
        for attachment in &stale {
            mark_attachment_stale_tx(
                &tx,
                attachment.conversation_id,
                "HostInstanceChanged",
                observed_at,
            )?;
            upsert_runtime_snapshot_tx(
                &tx,
                RuntimeSnapshot {
                    conversation_id: attachment.conversation_id,
                    state: RuntimeState::NotRunning,
                    source: ObservationSource::Recovery,
                    confidence: Confidence::Authoritative,
                    observed_at,
                    last_error: Some(
                        "stale attachment invalidated by new host instance".to_owned(),
                    ),
                },
            )?;
        }
        tx.execute(
            r#"
            INSERT INTO reconciliation_metadata(key, value, updated_at)
            VALUES ('current_host_instance_id', ?1, ?2)
            ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at
            "#,
            params![host_instance_id.to_string(), timestamp(observed_at)],
        )?;
        tx.commit()?;
        Ok(StartupReconciliation {
            stale_attachments_invalidated: stale.len(),
        })
    }

    pub fn apply_process_exit(
        &mut self,
        host_instance_id: HostInstanceId,
        observation: ProcessExitObservation,
    ) -> Result<Option<(ConversationId, AttachmentVersion, RuntimeState)>> {
        let tx = self.conn.transaction()?;
        let Some(current) =
            current_attachment_for_pane_tx(&tx, host_instance_id, observation.pane_id)?
        else {
            tx.commit()?;
            return Ok(None);
        };
        let state = runtime_state_for_exit(&observation);
        let version = AttachmentVersion {
            host_instance_id: current.host_instance_id,
            attachment_generation: current.attachment_generation,
        };
        mark_attachment_stale_tx(
            &tx,
            current.conversation_id,
            "ProcessExitObserved",
            observation.observed_at,
        )?;
        upsert_runtime_snapshot_tx(
            &tx,
            RuntimeSnapshot {
                conversation_id: current.conversation_id,
                state,
                source: ObservationSource::ProcessExit,
                confidence: Confidence::Authoritative,
                observed_at: observation.observed_at,
                last_error: match state {
                    RuntimeState::Failed => Some("process exited unsuccessfully".to_owned()),
                    _ => None,
                },
            },
        )?;
        tx.commit()?;
        Ok(Some((current.conversation_id, version, state)))
    }

    pub fn apply_bare_pane_removed(
        &mut self,
        _host_instance_id: HostInstanceId,
        _pane_id: PaneId,
    ) -> Result<bool> {
        Ok(false)
    }

    pub fn update_runtime_snapshot(&mut self, snapshot: RuntimeSnapshot) -> Result<()> {
        let tx = self.conn.transaction()?;
        upsert_runtime_snapshot_tx(&tx, snapshot)?;
        tx.commit()?;
        Ok(())
    }

    pub fn runtime_snapshot(
        &self,
        conversation_id: ConversationId,
    ) -> Result<Option<RuntimeSnapshot>> {
        self.conn
            .query_row(
                r#"
                SELECT conversation_id, runtime_state, source, confidence, observed_at, last_error
                FROM runtime_snapshots
                WHERE conversation_id = ?1
                "#,
                params![conversation_id.to_string()],
                row_to_runtime_snapshot,
            )
            .optional()
            .map_err(Into::into)
    }

    pub fn stop_conversation(
        &mut self,
        conversation_id: ConversationId,
        observed_at: DateTime<Utc>,
    ) -> Result<Option<AttachmentVersion>> {
        let tx = self.conn.transaction()?;
        let Some(current) = current_attachment_tx(&tx, conversation_id)? else {
            upsert_runtime_snapshot_tx(
                &tx,
                RuntimeSnapshot {
                    conversation_id,
                    state: RuntimeState::NotRunning,
                    source: ObservationSource::ProcessExit,
                    confidence: Confidence::Authoritative,
                    observed_at,
                    last_error: Some(
                        "stop requested while no current attachment existed".to_owned(),
                    ),
                },
            )?;
            tx.commit()?;
            return Ok(None);
        };
        let version = AttachmentVersion {
            host_instance_id: current.host_instance_id,
            attachment_generation: current.attachment_generation,
        };
        mark_attachment_stale_tx(&tx, conversation_id, "LucidityStop", observed_at)?;
        upsert_runtime_snapshot_tx(
            &tx,
            RuntimeSnapshot {
                conversation_id,
                state: RuntimeState::NotRunning,
                source: ObservationSource::ProcessExit,
                confidence: Confidence::Authoritative,
                observed_at,
                last_error: Some("lucidity stop requested".to_owned()),
            },
        )?;
        tx.commit()?;
        Ok(Some(version))
    }

    pub fn mark_event_resync_required(
        &mut self,
        host_instance_id: HostInstanceId,
        pane_id: PaneId,
        observed_at: DateTime<Utc>,
    ) -> Result<Option<ConversationId>> {
        let tx = self.conn.transaction()?;
        let current = current_attachment_for_pane_tx(&tx, host_instance_id, pane_id)?;
        let Some(current) = current else {
            tx.commit()?;
            return Ok(None);
        };
        tx.execute(
            "UPDATE runtime_attachments SET event_resync_required = 1 WHERE conversation_id = ?1",
            params![current.conversation_id.to_string()],
        )?;
        tx.execute(
            r#"
            INSERT INTO reconciliation_metadata(key, value, updated_at)
            VALUES (?1, ?2, ?3)
            ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at
            "#,
            params![
                format!("event_resync_required:{}", current.conversation_id),
                format!("pane:{}", pane_id.0),
                timestamp(observed_at)
            ],
        )?;
        tx.commit()?;
        Ok(Some(current.conversation_id))
    }

    pub fn event_resync_required(&self, conversation_id: ConversationId) -> Result<bool> {
        self.conn
            .query_row(
                "SELECT event_resync_required FROM runtime_attachments WHERE conversation_id = ?1",
                params![conversation_id.to_string()],
                |row| row.get::<_, i64>(0),
            )
            .optional()
            .map(|value| value.unwrap_or(0) != 0)
            .map_err(Into::into)
    }

    pub fn record_agent_event(
        &mut self,
        adapter_id: &AdapterId,
        profile_id: ProfileId,
        event_id: &str,
        conversation_id: Option<ConversationId>,
        observed_at: DateTime<Utc>,
    ) -> Result<bool> {
        let inserted = self.conn.execute(
            r#"
            INSERT OR IGNORE INTO agent_event_dedupe(
              adapter_id, profile_id, event_id, conversation_id, observed_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5)
            "#,
            params![
                adapter_id.0,
                profile_id.to_string(),
                event_id,
                conversation_id.map(|id| id.to_string()),
                timestamp(observed_at)
            ],
        )?;
        Ok(inserted == 1)
    }

    pub fn record_usage_snapshot(
        &mut self,
        conversation_id: ConversationId,
        observed_at: DateTime<Utc>,
        source: &str,
        usage_json: &Value,
        last_error: Option<&str>,
    ) -> Result<()> {
        self.conn.execute(
            r#"
            INSERT INTO usage_snapshots(
              conversation_id, observed_at, source, usage_json, stale_on_open_after, last_error
            )
            VALUES (?1, ?2, ?3, ?4, datetime(?2, '+5 minutes'), ?5)
            ON CONFLICT(conversation_id) DO UPDATE SET
              observed_at = excluded.observed_at,
              source = excluded.source,
              usage_json = excluded.usage_json,
              stale_on_open_after = excluded.stale_on_open_after,
              last_error = excluded.last_error
            "#,
            params![
                conversation_id.to_string(),
                timestamp(observed_at),
                source,
                serde_json::to_string(usage_json)?,
                last_error
            ],
        )?;
        Ok(())
    }

    pub fn record_recovery_warning(&mut self, warning: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO catalog_recovery_warnings(created_at, warning) VALUES (?1, ?2)",
            params![timestamp(Utc::now()), warning],
        )?;
        Ok(())
    }

    pub fn quarantine_conversation(
        &mut self,
        conversation_id: ConversationId,
        reason: &str,
    ) -> Result<()> {
        let tx = self.conn.transaction()?;
        tx.execute(
            r#"
            UPDATE conversations
            SET quarantined = 1, quarantine_reason = ?2, updated_at = ?3
            WHERE conversation_id = ?1
            "#,
            params![conversation_id.to_string(), reason, timestamp(Utc::now())],
        )?;
        bump_catalog_snapshot_tx(&tx, Utc::now())?;
        tx.commit()?;
        Ok(())
    }
}

fn ensure_profile_tx(
    tx: &Transaction<'_>,
    adapter_id: &AdapterId,
    profile_id: ProfileId,
    display_name: Option<&str>,
    now: DateTime<Utc>,
) -> Result<()> {
    tx.execute(
        r#"
        INSERT INTO profiles(profile_id, adapter_id, native_profile_id, display_name, created_at, updated_at)
        VALUES (?1, ?2, ?1, ?3, ?4, ?4)
        ON CONFLICT(profile_id) DO UPDATE SET
          adapter_id = excluded.adapter_id,
          display_name = CASE
            WHEN excluded.display_name = '' THEN profiles.display_name
            ELSE excluded.display_name
          END,
          updated_at = excluded.updated_at
        "#,
        params![
            profile_id.to_string(),
            adapter_id.0,
            display_name.unwrap_or(""),
            timestamp(now)
        ],
    )?;
    Ok(())
}

fn ensure_conversation_exists_tx(
    tx: &Transaction<'_>,
    conversation_id: ConversationId,
) -> Result<()> {
    let exists: Option<i64> = tx
        .query_row(
            "SELECT 1 FROM conversations WHERE conversation_id = ?1",
            params![conversation_id.to_string()],
            |row| row.get(0),
        )
        .optional()?;
    if exists.is_none() {
        bail!("unknown conversation {conversation_id}");
    }
    Ok(())
}

fn bump_catalog_snapshot_tx(tx: &Transaction<'_>, now: DateTime<Utc>) -> Result<u64> {
    tx.execute(
        "UPDATE catalog_metadata SET snapshot_version = snapshot_version + 1, updated_at = ?1 WHERE id = 1",
        params![timestamp(now)],
    )?;
    let version = tx.query_row(
        "SELECT snapshot_version FROM catalog_metadata WHERE id = 1",
        [],
        |row| row.get::<_, i64>(0),
    )?;
    Ok(version as u64)
}

fn current_attachment_tx(
    tx: &Transaction<'_>,
    conversation_id: ConversationId,
) -> Result<Option<RuntimeAttachment>> {
    tx.query_row(
        r#"
        SELECT conversation_id, pane_id, process_id, process_owner_json,
               host_instance_id, attachment_generation, attached_at
        FROM runtime_attachments
        WHERE conversation_id = ?1 AND stale = 0
        "#,
        params![conversation_id.to_string()],
        row_to_attachment,
    )
    .optional()
    .map_err(Into::into)
}

fn current_attachment_for_pane_tx(
    tx: &Transaction<'_>,
    host_instance_id: HostInstanceId,
    pane_id: PaneId,
) -> Result<Option<RuntimeAttachment>> {
    tx.query_row(
        r#"
        SELECT conversation_id, pane_id, process_id, process_owner_json,
               host_instance_id, attachment_generation, attached_at
        FROM runtime_attachments
        WHERE host_instance_id = ?1 AND pane_id = ?2 AND stale = 0
        "#,
        params![host_instance_id.to_string(), pane_id.0 as i64],
        row_to_attachment,
    )
    .optional()
    .map_err(Into::into)
}

fn current_attachment_for_pane_tx_unchecked(
    conn: &Connection,
    host_instance_id: HostInstanceId,
    pane_id: PaneId,
) -> Result<Option<RuntimeAttachment>> {
    conn.query_row(
        r#"
        SELECT conversation_id, pane_id, process_id, process_owner_json,
               host_instance_id, attachment_generation, attached_at
        FROM runtime_attachments
        WHERE host_instance_id = ?1 AND pane_id = ?2 AND stale = 0
        "#,
        params![host_instance_id.to_string(), pane_id.0 as i64],
        row_to_attachment,
    )
    .optional()
    .map_err(Into::into)
}

fn collect_current_attachments_not_host_tx(
    tx: &Transaction<'_>,
    host_instance_id: HostInstanceId,
) -> Result<Vec<RuntimeAttachment>> {
    let mut stmt = tx.prepare(
        r#"
        SELECT conversation_id, pane_id, process_id, process_owner_json,
               host_instance_id, attachment_generation, attached_at
        FROM runtime_attachments
        WHERE host_instance_id <> ?1 AND stale = 0
        ORDER BY conversation_id ASC
        "#,
    )?;
    let rows = stmt.query_map(params![host_instance_id.to_string()], row_to_attachment)?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Into::into)
}

fn mark_attachment_stale_tx(
    tx: &Transaction<'_>,
    conversation_id: ConversationId,
    reason: &str,
    observed_at: DateTime<Utc>,
) -> Result<()> {
    tx.execute(
        r#"
        UPDATE runtime_attachments
        SET stale = 1, detach_reason = ?2
        WHERE conversation_id = ?1 AND stale = 0
        "#,
        params![conversation_id.to_string(), reason],
    )?;
    tx.execute(
        r#"
        INSERT INTO reconciliation_metadata(key, value, updated_at)
        VALUES (?1, ?2, ?3)
        ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at
        "#,
        params![
            format!("runtime_detach_reason:{conversation_id}"),
            reason,
            timestamp(observed_at)
        ],
    )?;
    Ok(())
}

fn next_generation_tx(tx: &Transaction<'_>, host_instance_id: HostInstanceId) -> Result<u64> {
    let max_generation: Option<i64> = tx.query_row(
        "SELECT MAX(attachment_generation) FROM runtime_attachments WHERE host_instance_id = ?1",
        params![host_instance_id.to_string()],
        |row| row.get(0),
    )?;
    Ok(max_generation.unwrap_or(0) as u64 + 1)
}

fn upsert_runtime_snapshot_tx(tx: &Transaction<'_>, snapshot: RuntimeSnapshot) -> Result<()> {
    tx.execute(
        r#"
        INSERT INTO runtime_snapshots(
          conversation_id, runtime_state, source, confidence, observed_at, last_error
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6)
        ON CONFLICT(conversation_id) DO UPDATE SET
          runtime_state = excluded.runtime_state,
          source = excluded.source,
          confidence = excluded.confidence,
          observed_at = excluded.observed_at,
          last_error = excluded.last_error
        "#,
        params![
            snapshot.conversation_id.to_string(),
            runtime_state_to_db(snapshot.state),
            observation_source_to_db(snapshot.source),
            confidence_to_db(snapshot.confidence),
            timestamp(snapshot.observed_at),
            snapshot.last_error
        ],
    )?;
    Ok(())
}

fn row_to_conversation(row: &Row<'_>) -> rusqlite::Result<ConversationRecord> {
    let conversation_id: String = row.get(0)?;
    let adapter_id: String = row.get(1)?;
    let profile_id: String = row.get(2)?;
    let native_session_id: String = row.get(3)?;
    let native_session_path: Option<String> = row.get(4)?;
    let title: String = row.get(5)?;
    let user_alias: Option<String> = row.get(6)?;
    let project_path: Option<String> = row.get(7)?;
    let created_at: String = row.get(8)?;
    let last_activity_at: String = row.get(9)?;
    let organization_state: String = row.get(10)?;
    Ok(ConversationRecord {
        id: parse_conversation_id_sql(0, conversation_id)?,
        native: NativeConversationKey {
            adapter_id: AdapterId(adapter_id),
            profile_id: parse_profile_id_sql(2, profile_id)?,
            native_session_id,
        },
        native_session_path: native_session_path.map(PathBuf::from),
        title,
        user_alias,
        project_path: project_path.map(PathBuf::from),
        created_at: parse_datetime_sql(8, created_at)?,
        last_activity_at: parse_datetime_sql(9, last_activity_at)?,
        organization_state: organization_state_from_db_sql(10, organization_state)?,
    })
}

fn row_to_attachment(row: &Row<'_>) -> rusqlite::Result<RuntimeAttachment> {
    let conversation_id: String = row.get(0)?;
    let pane_id: i64 = row.get(1)?;
    let process_id: Option<i64> = row.get(2)?;
    let process_owner_json: Option<String> = row.get(3)?;
    let host_instance_id: String = row.get(4)?;
    let generation: i64 = row.get(5)?;
    let attached_at: String = row.get(6)?;
    let process_owner = match process_owner_json {
        Some(json) => Some(serde_json::from_str(&json).map_err(|err| {
            rusqlite::Error::FromSqlConversionFailure(3, Type::Text, Box::new(err))
        })?),
        None => None,
    };
    Ok(RuntimeAttachment {
        conversation_id: parse_conversation_id_sql(0, conversation_id)?,
        pane_id: PaneId(pane_id as u64),
        process_id: process_id.map(|pid| pid as u32),
        process_owner,
        host_instance_id: parse_host_instance_id_sql(4, host_instance_id)?,
        attachment_generation: generation as u64,
        attached_at: parse_datetime_sql(6, attached_at)?,
    })
}

fn row_to_runtime_snapshot(row: &Row<'_>) -> rusqlite::Result<RuntimeSnapshot> {
    let conversation_id: String = row.get(0)?;
    let runtime_state: String = row.get(1)?;
    let source: String = row.get(2)?;
    let confidence: String = row.get(3)?;
    let observed_at: String = row.get(4)?;
    let last_error: Option<String> = row.get(5)?;
    Ok(RuntimeSnapshot {
        conversation_id: parse_conversation_id_sql(0, conversation_id)?,
        state: runtime_state_from_db_sql(1, runtime_state)?,
        source: observation_source_from_db_sql(2, source)?,
        confidence: confidence_from_db_sql(3, confidence)?,
        observed_at: parse_datetime_sql(4, observed_at)?,
        last_error,
    })
}

fn runtime_state_for_exit(observation: &ProcessExitObservation) -> RuntimeState {
    match observation.requested_termination {
        RequestedTermination::LucidityStop
        | RequestedTermination::LucidityQuit
        | RequestedTermination::WezTermKill => RuntimeState::NotRunning,
        RequestedTermination::None => match &observation.result {
            agent_protocol::ExitResult::Exited { success, .. } if *success => {
                RuntimeState::CompletedIdle
            }
            _ => RuntimeState::Failed,
        },
    }
}

fn quarantine_catalog_file(path: &Path) -> Result<PathBuf> {
    if !path.exists() {
        bail!("cannot quarantine missing catalog {}", path.display());
    }
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| anyhow!("catalog path has no UTF-8 file name: {}", path.display()))?;
    for index in 0..10_000u32 {
        let candidate = parent.join(format!(
            "{file_name}.quarantine.{}.{}",
            Utc::now().format("%Y%m%dT%H%M%S%.fZ"),
            index
        ));
        if !candidate.exists() {
            fs::rename(path, &candidate)?;
            return Ok(candidate);
        }
    }
    bail!(
        "unable to allocate unique quarantine name for {}",
        path.display()
    )
}

fn timestamp(value: DateTime<Utc>) -> String {
    value.to_rfc3339_opts(SecondsFormat::Nanos, true)
}

fn parse_datetime(value: &str) -> Result<DateTime<Utc>> {
    Ok(DateTime::parse_from_rfc3339(value)?.with_timezone(&Utc))
}

fn parse_datetime_sql(index: usize, value: String) -> rusqlite::Result<DateTime<Utc>> {
    parse_datetime(&value)
        .map_err(|err| rusqlite::Error::FromSqlConversionFailure(index, Type::Text, err.into()))
}

fn parse_conversation_id(value: &str) -> Result<ConversationId> {
    value.parse().map_err(Into::into)
}

fn parse_conversation_id_sql(index: usize, value: String) -> rusqlite::Result<ConversationId> {
    value
        .parse()
        .map_err(|err| rusqlite::Error::FromSqlConversionFailure(index, Type::Text, Box::new(err)))
}

fn parse_profile_id_sql(index: usize, value: String) -> rusqlite::Result<ProfileId> {
    value
        .parse()
        .map_err(|err| rusqlite::Error::FromSqlConversionFailure(index, Type::Text, Box::new(err)))
}

fn parse_host_instance_id_sql(index: usize, value: String) -> rusqlite::Result<HostInstanceId> {
    value
        .parse()
        .map_err(|err| rusqlite::Error::FromSqlConversionFailure(index, Type::Text, Box::new(err)))
}

fn path_to_db(path: Option<&Path>) -> Option<String> {
    path.map(|path| path.to_string_lossy().into_owned())
}

fn max_datetime(left: DateTime<Utc>, right: DateTime<Utc>) -> DateTime<Utc> {
    if left >= right {
        left
    } else {
        right
    }
}

fn checksum_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut output = String::with_capacity(digest.len() * 2);
    for byte in digest {
        use std::fmt::Write as _;
        let _ = write!(output, "{byte:02x}");
    }
    output
}

macro_rules! db_enum {
    ($to_fn:ident, $from_fn:ident, $from_sql_fn:ident, $ty:ty, {
        $($variant:path => $name:literal),+ $(,)?
    }) => {
        fn $to_fn(value: $ty) -> &'static str {
            match value {
                $($variant => $name,)+
            }
        }

        fn $from_fn(value: &str) -> Result<$ty> {
            match value {
                $($name => Ok($variant),)+
                other => bail!("unknown catalog enum value {other:?}"),
            }
        }

        fn $from_sql_fn(index: usize, value: String) -> rusqlite::Result<$ty> {
            $from_fn(&value).map_err(|err| {
                rusqlite::Error::FromSqlConversionFailure(index, Type::Text, err.into())
            })
        }
    };
}

db_enum!(
    organization_state_to_db,
    organization_state_from_db,
    organization_state_from_db_sql,
    OrganizationState,
    {
        OrganizationState::Active => "active",
        OrganizationState::Settled => "settled",
    }
);

db_enum!(
    runtime_state_to_db,
    runtime_state_from_db,
    runtime_state_from_db_sql,
    RuntimeState,
    {
        RuntimeState::NotRunning => "not_running",
        RuntimeState::Starting => "starting",
        RuntimeState::Working => "working",
        RuntimeState::WaitingForInput => "waiting_for_input",
        RuntimeState::AwaitingApproval => "awaiting_approval",
        RuntimeState::CompletedIdle => "completed_idle",
        RuntimeState::Failed => "failed",
        RuntimeState::UnknownExternal => "unknown_external",
    }
);

db_enum!(
    observation_source_to_db,
    observation_source_from_db,
    observation_source_from_db_sql,
    ObservationSource,
    {
        ObservationSource::StructuredEvent => "structured_event",
        ObservationSource::ProviderApi => "provider_api",
        ObservationSource::SessionStore => "session_store",
        ObservationSource::SubmittedCommand => "submitted_command",
        ObservationSource::ScreenObservation => "screen_observation",
        ObservationSource::UserBinding => "user_binding",
        ObservationSource::ProcessExit => "process_exit",
        ObservationSource::Recovery => "recovery",
    }
);

db_enum!(
    confidence_to_db,
    confidence_from_db,
    confidence_from_db_sql,
    Confidence,
    {
        Confidence::Authoritative => "authoritative",
        Confidence::Correlated => "correlated",
        Confidence::Heuristic => "heuristic",
        Confidence::Unknown => "unknown",
    }
);
