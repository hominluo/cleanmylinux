//! Shared data types used across the engine and both frontends.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// The cleanup modules the app exposes. Used by frontends to build the sidebar
/// and to ask the engine to scan a specific area.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Module {
    SystemJunk,
    PackageCleanup,
    LargeAndOldFiles,
    Uninstaller,
    SystemMonitor,
}

impl Module {
    pub const ALL: [Module; 5] = [
        Module::SystemJunk,
        Module::PackageCleanup,
        Module::LargeAndOldFiles,
        Module::Uninstaller,
        Module::SystemMonitor,
    ];

    /// Human-readable title for the UI.
    pub fn title(self) -> &'static str {
        match self {
            Module::SystemJunk => "System Junk",
            Module::PackageCleanup => "Package Cleanup",
            Module::LargeAndOldFiles => "Large & Old Files",
            Module::Uninstaller => "Uninstaller",
            Module::SystemMonitor => "System Monitor",
        }
    }

    /// Freedesktop icon name (works in both GTK and Qt icon themes).
    pub fn icon_name(self) -> &'static str {
        match self {
            Module::SystemJunk => "user-trash-symbolic",
            Module::PackageCleanup => "package-x-generic-symbolic",
            Module::LargeAndOldFiles => "folder-documents-symbolic",
            Module::Uninstaller => "application-x-executable-symbolic",
            Module::SystemMonitor => "utilities-system-monitor-symbolic",
        }
    }
}

/// How risky it is to delete a given item. Frontends pre-check only `Safe` items.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Safety {
    /// Pure regenerable junk — safe to remove (default-selected).
    Safe,
    /// Generally fine but worth a glance (default-selected, but flagged).
    Review,
    /// Could affect a running app or remove user data — default-unselected.
    Risky,
}

/// Whether a cleanup deletes permanently or moves to the freedesktop Trash.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeleteMode {
    /// Permanently delete (regenerable junk: caches, thumbnails, pkg caches).
    Permanent,
    /// Move to ~/.local/share/Trash so the user can recover it (user files).
    Trash,
}

/// One scannable / cleanable entry surfaced to the UI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanItem {
    /// Category label within a module, e.g. "User Cache", "Thumbnails".
    pub category: String,
    /// Short human label for the row.
    pub label: String,
    /// Absolute path this item refers to (a dir or a file).
    pub path: PathBuf,
    /// Size in bytes (actual block usage, hardlinks counted once).
    pub size: u64,
    pub safety: Safety,
    pub delete_mode: DeleteMode,
    /// Whether the UI should pre-select this for cleaning.
    pub selected: bool,
    /// Optional note shown to the user (e.g. "Firefox is running — skipped").
    pub note: Option<String>,
}

impl ScanItem {
    pub fn human_size(&self) -> String {
        humansize::format_size(self.size, humansize::DECIMAL)
    }
}

/// Aggregated result of scanning a module.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ScanResult {
    pub items: Vec<ScanItem>,
}

impl ScanResult {
    pub fn total_size(&self) -> u64 {
        self.items.iter().map(|i| i.size).sum()
    }

    pub fn selected_size(&self) -> u64 {
        self.items.iter().filter(|i| i.selected).map(|i| i.size).sum()
    }

    pub fn human_total(&self) -> String {
        humansize::format_size(self.total_size(), humansize::DECIMAL)
    }
}

/// Outcome of a cleanup run — what actually got freed and any per-item failures.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CleanReport {
    pub freed_bytes: u64,
    pub removed: usize,
    /// (path, reason) for items that could not be removed.
    pub failures: Vec<(PathBuf, String)>,
}

impl CleanReport {
    pub fn human_freed(&self) -> String {
        humansize::format_size(self.freed_bytes, humansize::DECIMAL)
    }
}
