//! User configuration, persisted as TOML at
//! `~/.config/cleanmylinux/config.toml`. Shared by both frontends (chosen over
//! GSettings so the Qt/Kirigami build doesn't need a GTK schema).

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Paths the user never wants scanned or deleted (added on top of the
    /// built-in safelist).
    pub exclusions: Vec<PathBuf>,
    /// Minimum size (bytes) for the "Large & Old Files" finder.
    pub large_file_threshold: u64,
    /// Minimum age (days) for a file to count as "old".
    pub old_file_days: u64,
    /// Theme preference: "system" | "light" | "dark".
    pub theme: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            exclusions: Vec::new(),
            large_file_threshold: 100 * 1024 * 1024, // 100 MB
            old_file_days: 180,
            theme: "system".to_string(),
        }
    }
}

impl Config {
    /// `~/.config/cleanmylinux/config.toml`.
    pub fn path() -> Option<PathBuf> {
        dirs::config_dir().map(|d| d.join("cleanmylinux").join("config.toml"))
    }

    /// Load config, falling back to defaults if missing or unparsable.
    pub fn load() -> Self {
        let Some(path) = Self::path() else {
            return Self::default();
        };
        match std::fs::read_to_string(&path) {
            Ok(text) => toml::from_str(&text).unwrap_or_else(|e| {
                tracing::warn!("config parse failed ({e}); using defaults");
                Self::default()
            }),
            Err(_) => Self::default(),
        }
    }

    /// Persist config to disk, creating parent directories as needed.
    pub fn save(&self) -> anyhow::Result<()> {
        let path = Self::path().ok_or_else(|| anyhow::anyhow!("no config dir"))?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let text = toml::to_string_pretty(self)?;
        std::fs::write(&path, text)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrips_via_toml() {
        let cfg = Config {
            exclusions: vec![PathBuf::from("/home/u/keep")],
            large_file_threshold: 42,
            old_file_days: 7,
            theme: "dark".into(),
        };
        let text = toml::to_string_pretty(&cfg).unwrap();
        let back: Config = toml::from_str(&text).unwrap();
        assert_eq!(back.large_file_threshold, 42);
        assert_eq!(back.theme, "dark");
        assert_eq!(back.exclusions.len(), 1);
    }

    #[test]
    fn defaults_are_sane() {
        let c = Config::default();
        assert!(c.large_file_threshold >= 1024 * 1024);
        assert_eq!(c.theme, "system");
    }
}
