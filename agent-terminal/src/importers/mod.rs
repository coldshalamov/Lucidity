//! Native history importers for configured coding agents.

use agent_protocol::{
    AdapterId, ConversationId, ConversationRecord, NativeConversationKey, OrganizationState,
    ProfileId,
};
use chrono::{TimeZone, Utc};
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImportedHistory {
    pub adapter_id: AdapterId,
    pub native_session_id: String,
    pub title: String,
    pub project_path: Option<PathBuf>,
    pub native_session_path: Option<PathBuf>,
    pub last_activity_unix: i64,
}

/// Import Claude Code-style session index entries when present.
///
/// Looks under `%USERPROFILE%\.claude\projects` for `*.jsonl` transcripts and
/// produces settled, NotRunning catalog candidates. Allocates no PTY.
pub fn import_claude_histories(home: &Path) -> Vec<ImportedHistory> {
    let projects = home.join(".claude").join("projects");
    if !projects.is_dir() {
        return Vec::new();
    }
    let mut out = Vec::new();
    let Ok(entries) = fs::read_dir(&projects) else {
        return out;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            out.extend(scan_claude_project_dir(&path));
        } else if path.extension().and_then(|e| e.to_str()) == Some("jsonl") {
            if let Some(item) = claude_from_jsonl(&path) {
                out.push(item);
            }
        }
    }
    out
}

fn scan_claude_project_dir(dir: &Path) -> Vec<ImportedHistory> {
    let mut out = Vec::new();
    let Ok(entries) = fs::read_dir(dir) else {
        return out;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("jsonl") {
            if let Some(item) = claude_from_jsonl(&path) {
                out.push(item);
            }
        }
    }
    out
}

fn claude_from_jsonl(path: &Path) -> Option<ImportedHistory> {
    let meta = fs::metadata(path).ok()?;
    let modified = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let stem = path.file_stem()?.to_string_lossy().into_owned();
    let parent_name = path
        .parent()
        .and_then(|p| p.file_name())
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "project".into());
    // First line may contain session metadata.
    let title = read_claude_title(path).unwrap_or_else(|| format!("Claude · {stem}"));
    Some(ImportedHistory {
        adapter_id: AdapterId::from("claude"),
        native_session_id: stem,
        title,
        project_path: Some(PathBuf::from(parent_name)),
        native_session_path: Some(path.to_path_buf()),
        last_activity_unix: modified,
    })
}

#[derive(Deserialize)]
struct ClaudeLine {
    #[serde(default)]
    sessionId: Option<String>,
    #[serde(default)]
    summary: Option<String>,
    #[serde(default)]
    type_field: Option<String>,
    #[serde(default, rename = "type")]
    kind: Option<String>,
    #[serde(default)]
    message: Option<ClaudeMessage>,
}

#[derive(Deserialize)]
struct ClaudeMessage {
    #[serde(default)]
    content: Option<String>,
}

fn read_claude_title(path: &Path) -> Option<String> {
    let text = fs::read_to_string(path).ok()?;
    for line in text.lines().take(20) {
        if let Ok(parsed) = serde_json::from_str::<ClaudeLine>(line) {
            if let Some(summary) = parsed.summary.filter(|s| !s.is_empty()) {
                return Some(summary);
            }
            if let Some(content) = parsed
                .message
                .and_then(|m| m.content)
                .filter(|s| !s.is_empty())
            {
                let short: String = content.chars().take(64).collect();
                return Some(short);
            }
            let _ = (parsed.sessionId, parsed.type_field, parsed.kind);
        }
    }
    None
}

/// Convert an import candidate into a catalog upsert-shaped record (settled).
pub fn imported_to_record(item: &ImportedHistory, profile_id: ProfileId) -> ConversationRecord {
    let ts = Utc
        .timestamp_opt(item.last_activity_unix, 0)
        .single()
        .unwrap_or_else(Utc::now);
    ConversationRecord {
        id: ConversationId(Uuid::new_v4()),
        native: NativeConversationKey {
            adapter_id: item.adapter_id.clone(),
            profile_id,
            native_session_id: item.native_session_id.clone(),
        },
        native_session_path: item.native_session_path.clone(),
        title: item.title.clone(),
        user_alias: None,
        project_path: item.project_path.clone(),
        created_at: ts,
        last_activity_at: ts,
        organization_state: OrganizationState::Settled,
    }
}

/// Import Codex CLI session files under `~/.codex/sessions` when present.
pub fn import_codex_histories(home: &Path) -> Vec<ImportedHistory> {
    let root = home.join(".codex").join("sessions");
    if !root.is_dir() {
        return Vec::new();
    }
    let mut out = Vec::new();
    let Ok(entries) = fs::read_dir(&root) else {
        return out;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let meta = fs::metadata(&path).ok();
        let modified = meta
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        let stem = path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "codex-session".into());
        out.push(ImportedHistory {
            adapter_id: AdapterId::from("codex"),
            native_session_id: stem.clone(),
            title: format!("Codex · {stem}"),
            project_path: None,
            native_session_path: Some(path),
            last_activity_unix: modified,
        });
    }
    out
}

pub fn import_all_known(home: &Path) -> Vec<ImportedHistory> {
    let mut all = import_claude_histories(home);
    all.extend(import_codex_histories(home));
    all
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn import_claude_jsonl_without_pty() {
        let dir = tempdir().unwrap();
        let projects = dir.path().join(".claude").join("projects").join("demo");
        fs::create_dir_all(&projects).unwrap();
        let file = projects.join("abc123.jsonl");
        let mut f = fs::File::create(&file).unwrap();
        writeln!(
            f,
            r#"{{"summary":"Refactor renderer","sessionId":"abc123"}}"#
        )
        .unwrap();
        let items = import_claude_histories(dir.path());
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].adapter_id.0, "claude");
        assert_eq!(items[0].native_session_id, "abc123");
        assert!(items[0].title.contains("Refactor"));
        let record = imported_to_record(&items[0], ProfileId(Uuid::new_v4()));
        assert_eq!(record.organization_state, OrganizationState::Settled);
    }

    #[test]
    fn missing_home_dirs_return_empty() {
        let dir = tempdir().unwrap();
        assert!(import_all_known(dir.path()).is_empty());
    }
}
