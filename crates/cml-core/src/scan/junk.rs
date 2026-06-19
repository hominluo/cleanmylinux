//! System Junk scanner: user & app caches, thumbnails, Trash, browser caches.
//!
//! Everything here is user-space (no root needed). To avoid double-counting,
//! `~/.cache` is partitioned by its top-level entries: known entries
//! (thumbnails, browser caches) get their own category and safety handling, and
//! the rest are reported as generic per-app cache rows. Browser caches are
//! flagged and de-selected when the browser is running, since wiping a live
//! cache can corrupt the session.

use crate::fsutil::{dir_size, is_nonempty_dir};
use crate::process::is_process_running;
use crate::progress::{report, CancelToken, Progress};
use crate::safety::Safelist;
use crate::types::{DeleteMode, Safety, ScanItem, ScanResult};
use std::path::{Path, PathBuf};

/// A `~/.cache` top-level entry that gets special treatment.
struct Special {
    /// directory name directly under `~/.cache`
    dir: &'static str,
    category: &'static str,
    label: &'static str,
    safety: Safety,
    /// de-select + note when this process is running
    guard_process: Option<&'static str>,
}

const SPECIAL_CACHE_ENTRIES: &[Special] = &[
    Special {
        dir: "thumbnails",
        category: "Thumbnails",
        label: "Thumbnail cache",
        safety: Safety::Safe,
        guard_process: None,
    },
    Special {
        dir: "mozilla",
        category: "Browser Cache",
        label: "Firefox cache",
        safety: Safety::Review,
        guard_process: Some("firefox"),
    },
    Special {
        dir: "chromium",
        category: "Browser Cache",
        label: "Chromium cache",
        safety: Safety::Review,
        guard_process: Some("chromium"),
    },
    Special {
        dir: "google-chrome",
        category: "Browser Cache",
        label: "Google Chrome cache",
        safety: Safety::Review,
        guard_process: Some("chrome"),
    },
];

/// Scan all junk categories. `progress` is called as scanning proceeds.
pub fn scan(
    safelist: &Safelist,
    cancel: &CancelToken,
    mut progress: Option<&mut Progress<'_>>,
) -> ScanResult {
    let mut result = ScanResult::default();
    let home = safelist.home().to_path_buf();

    report(&mut progress, Some(0.1), "Scanning application caches…");
    scan_cache_tree(&home.join(".cache"), safelist, cancel, &mut result);

    if cancel.is_cancelled() {
        return result;
    }

    // Trash (separate location, not under ~/.cache).
    report(&mut progress, Some(0.7), "Scanning Trash…");
    let trash = home.join(".local/share/Trash");
    if trash.exists() && is_nonempty_dir(&trash) && !safelist.is_protected(&trash) {
        let size = dir_size(&trash, cancel);
        if size > 0 {
            result.items.push(ScanItem {
                category: "Trash".into(),
                label: "Trash bin".into(),
                path: trash,
                size,
                safety: Safety::Review,
                delete_mode: DeleteMode::Permanent,
                selected: true,
                note: None,
            });
        }
    }

    // Flatpak per-app caches: ~/.var/app/*/cache
    report(&mut progress, Some(0.85), "Scanning flatpak caches…");
    scan_flatpak_caches(&home, safelist, cancel, &mut result);

    report(&mut progress, Some(1.0), "Scan complete");
    result
}

/// Partition `~/.cache` by top-level entry so categories never overlap.
fn scan_cache_tree(
    cache: &Path,
    safelist: &Safelist,
    cancel: &CancelToken,
    result: &mut ScanResult,
) {
    let Ok(entries) = std::fs::read_dir(cache) else {
        return;
    };
    for entry in entries.flatten() {
        if cancel.is_cancelled() {
            break;
        }
        let path = entry.path();
        if safelist.is_protected(&path) {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();

        let size = dir_size(&path, cancel);
        if size == 0 {
            continue;
        }

        if let Some(special) = SPECIAL_CACHE_ENTRIES.iter().find(|s| s.dir == name) {
            let mut selected = true;
            let mut note = None;
            if let Some(proc_name) = special.guard_process {
                if is_process_running(proc_name) {
                    selected = false;
                    note = Some(format!("{proc_name} is running — review before cleaning"));
                }
            }
            result.items.push(ScanItem {
                category: special.category.into(),
                label: special.label.into(),
                path,
                size,
                safety: special.safety,
                delete_mode: DeleteMode::Permanent,
                selected,
                note,
            });
        } else {
            // Generic per-app cache directory.
            result.items.push(ScanItem {
                category: "Application Cache".into(),
                label: format!("{name} cache"),
                path,
                size,
                safety: Safety::Safe,
                delete_mode: DeleteMode::Permanent,
                selected: true,
                note: None,
            });
        }
    }
}

fn scan_flatpak_caches(
    home: &Path,
    safelist: &Safelist,
    cancel: &CancelToken,
    result: &mut ScanResult,
) {
    let var_app = home.join(".var/app");
    let Ok(entries) = std::fs::read_dir(&var_app) else {
        return;
    };
    for entry in entries.flatten() {
        if cancel.is_cancelled() {
            break;
        }
        let cache = entry.path().join("cache");
        if !cache.exists() || safelist.is_protected(&cache) {
            continue;
        }
        let size = dir_size(&cache, cancel);
        if size == 0 {
            continue;
        }
        let app_id = entry.file_name().to_string_lossy().to_string();
        result.items.push(ScanItem {
            category: "Flatpak Cache".to_string(),
            label: format!("{app_id} cache"),
            path: cache,
            size,
            safety: Safety::Safe,
            delete_mode: DeleteMode::Permanent,
            selected: true,
            note: None,
        });
    }
}

/// Convenience: total junk discovered with an empty exclusion list.
pub fn quick_scan_home() -> ScanResult {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"));
    let safelist = Safelist::new(home, vec![]);
    scan(&safelist, &CancelToken::new(), None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn partitions_cache_without_overlap() {
        let home = tempfile::tempdir().unwrap();
        let cache = home.path().join(".cache");
        std::fs::create_dir_all(cache.join("thumbnails")).unwrap();
        std::fs::create_dir_all(cache.join("myapp")).unwrap();
        std::fs::write(cache.join("thumbnails/t.png"), vec![0u8; 4096]).unwrap();
        std::fs::write(cache.join("myapp/c.bin"), vec![0u8; 8192]).unwrap();

        let sl = Safelist::new(home.path(), vec![]);
        let res = scan(&sl, &CancelToken::new(), None);

        // One thumbnails row + one generic app row, no broad ~/.cache row.
        assert!(res.items.iter().any(|i| i.category == "Thumbnails"));
        assert!(res
            .items
            .iter()
            .any(|i| i.category == "Application Cache" && i.label.contains("myapp")));
        // No item should point at ~/.cache itself (that would double-count).
        assert!(!res.items.iter().any(|i| i.path == cache));
    }
}
