//! Graphical settings persistence for terminal and agent profiles.

use lucidity_ui::SettingsDraft;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PersistedSettings {
    pub terminal_font_size: f32,
    pub terminal_font_family: String,
    pub agent_executable_overrides: Vec<(String, String)>,
    pub agent_extra_args: Vec<(String, String)>,
}

impl From<SettingsDraft> for PersistedSettings {
    fn from(value: SettingsDraft) -> Self {
        Self {
            terminal_font_size: value.terminal_font_size,
            terminal_font_family: value.terminal_font_family,
            agent_executable_overrides: value.agent_executable_overrides,
            agent_extra_args: value.agent_extra_args,
        }
    }
}

impl From<PersistedSettings> for SettingsDraft {
    fn from(value: PersistedSettings) -> Self {
        Self {
            terminal_font_size: value.terminal_font_size,
            terminal_font_family: value.terminal_font_family,
            agent_executable_overrides: value.agent_executable_overrides,
            agent_extra_args: value.agent_extra_args,
        }
    }
}

pub fn settings_path() -> PathBuf {
    if let Some(path) = std::env::var_os("LUCIDITY_SETTINGS_PATH") {
        return PathBuf::from(path);
    }
    #[cfg(windows)]
    if let Some(root) = std::env::var_os("LOCALAPPDATA").or_else(|| std::env::var_os("APPDATA")) {
        return PathBuf::from(root).join("Lucidity").join("settings.json");
    }
    PathBuf::from(".lucidity").join("settings.json")
}

pub fn load_settings(path: &Path) -> SettingsDraft {
    let Ok(bytes) = fs::read(path) else {
        return SettingsDraft::default();
    };
    serde_json::from_slice::<PersistedSettings>(&bytes)
        .map(Into::into)
        .unwrap_or_default()
}

pub fn save_settings(path: &Path, draft: &SettingsDraft) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let persisted = PersistedSettings::from(draft.clone());
    let bytes = serde_json::to_vec_pretty(&persisted)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    fs::write(path, bytes)
}

pub fn extra_args_for(draft: &SettingsDraft, adapter_id: &str) -> Vec<String> {
    draft
        .agent_extra_args
        .iter()
        .find(|(id, _)| id == adapter_id)
        .map(|(_, args)| {
            args.split_whitespace()
                .map(|s| s.to_owned())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

pub fn executable_override_for(draft: &SettingsDraft, adapter_id: &str) -> Option<PathBuf> {
    draft
        .agent_executable_overrides
        .iter()
        .find(|(id, _)| id == adapter_id)
        .map(|(_, path)| path.trim())
        .filter(|path| !path.is_empty())
        .map(PathBuf::from)
}

/// Map a settings draft onto the WezTerm product config override pairs.
///
/// Delegates to the shipped `wezterm_gui::terminal_appearance_overrides` so the
/// product path and tests share one implementation.
pub fn terminal_overrides_from_draft(draft: &SettingsDraft) -> Vec<(String, String)> {
    wezterm_gui::terminal_appearance_overrides(
        draft.terminal_font_size as f64,
        &draft.terminal_font_family,
        false,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn round_trip_settings_file() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let draft = SettingsDraft {
            terminal_font_size: 14.0,
            terminal_font_family: "Cascadia Mono".into(),
            agent_executable_overrides: vec![("claude".into(), r"C:\tools\claude.exe".into())],
            agent_extra_args: vec![("claude".into(), "--verbose".into())],
        };
        save_settings(&path, &draft).unwrap();
        let loaded = load_settings(&path);
        assert_eq!(loaded.terminal_font_size, 14.0);
        assert_eq!(loaded.terminal_font_family, "Cascadia Mono");
        assert_eq!(
            executable_override_for(&loaded, "claude").unwrap(),
            PathBuf::from(r"C:\tools\claude.exe")
        );
        assert_eq!(extra_args_for(&loaded, "claude"), vec!["--verbose"]);
    }

    #[test]
    fn terminal_overrides_drive_shipped_wezterm_mapping() {
        let draft = SettingsDraft {
            terminal_font_size: 16.5,
            terminal_font_family: "JetBrains Mono".into(),
            agent_executable_overrides: vec![],
            agent_extra_args: vec![],
        };
        let overrides = terminal_overrides_from_draft(&draft);
        let map: std::collections::HashMap<_, _> = overrides.into_iter().collect();
        assert_eq!(map.get("font_size").map(String::as_str), Some("16.5"));
        let font = map.get("font").expect("font");
        assert!(font.contains("JetBrains Mono"), "{font}");
        // Must keep WebGPU for the egui product shell.
        assert_eq!(map.get("front_end").map(String::as_str), Some("\"WebGpu\""));
    }
}
