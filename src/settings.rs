//! Persisted application settings.
//!
//! Stored as JSON in the platform-appropriate config directory (via the
//! `directories` crate). Settings are best-effort: failures to read or write
//! fall back to defaults rather than blocking the app.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::theme::AppTheme;

const MAX_RECENT: usize = 12;

/// User-configurable, persisted settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Selected application theme.
    pub theme: AppTheme,
    /// Recently opened database paths, most-recent first.
    pub recent_files: Vec<String>,
    /// Editor font size in logical pixels.
    pub editor_font_size: u16,
    /// Show line numbers in the editor.
    pub editor_line_numbers: bool,
    /// Maximum rows fetched per data-browser page.
    pub page_size: usize,
    /// Word-wrap long SQL in the editor.
    pub editor_word_wrap: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: AppTheme::default(),
            recent_files: Vec::new(),
            editor_font_size: 14,
            editor_line_numbers: true,
            page_size: 200,
            editor_word_wrap: false,
        }
    }
}

impl Settings {
    /// Load settings from disk, returning defaults on any error.
    pub fn load() -> Self {
        let Some(path) = Self::path() else {
            return Self::default();
        };
        match std::fs::read_to_string(&path) {
            Ok(text) => serde_json::from_str(&text).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    /// Persist settings to disk (best-effort).
    pub fn save(&self) {
        let Some(path) = Self::path() else { return };
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(text) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(&path, text);
        }
    }

    /// Record a database path as most-recently-used.
    pub fn push_recent(&mut self, path: &str) {
        self.recent_files.retain(|p| p != path);
        self.recent_files.insert(0, path.to_string());
        self.recent_files.truncate(MAX_RECENT);
    }

    fn path() -> Option<PathBuf> {
        directories::ProjectDirs::from("org", "DecentDB", "DecentDB Studio")
            .map(|dirs| dirs.config_dir().join("settings.json"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recent_files_dedup_and_truncate() {
        let mut s = Settings::default();
        for i in 0..20 {
            s.push_recent(&format!("/db/{i}.ddb"));
        }
        assert_eq!(s.recent_files.len(), MAX_RECENT);
        s.push_recent("/db/5.ddb");
        assert_eq!(s.recent_files[0], "/db/5.ddb");
        assert_eq!(
            s.recent_files.iter().filter(|p| *p == "/db/5.ddb").count(),
            1
        );
    }

    #[test]
    fn defaults_are_sane() {
        let s = Settings::default();
        assert!(s.editor_font_size >= 8);
        assert!(s.page_size > 0);
    }
}
