//! Large & Old Files finder: a parallel walk of `$HOME` surfacing space hogs.
//!
//! Matches are moved to the Trash (recoverable) rather than deleted outright —
//! these are user files, not regenerable junk.

use crate::progress::{report, CancelToken, Progress};
use crate::safety::Safelist;
use crate::types::{DeleteMode, Safety, ScanItem, ScanResult};
use std::os::unix::fs::MetadataExt;
use std::time::{SystemTime, UNIX_EPOCH};

const BLOCK_UNIT: u64 = 512;

/// Find files under `$HOME` at least `min_bytes` large. If `older_than_days` is
/// `Some`, only files not modified within that many days are returned.
pub fn scan(
    safelist: &Safelist,
    min_bytes: u64,
    older_than_days: Option<u64>,
    cancel: &CancelToken,
    mut progress: Option<&mut Progress<'_>>,
) -> ScanResult {
    let mut result = ScanResult::default();
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let age_cutoff = older_than_days.map(|d| now.saturating_sub(d * 86_400));

    report(&mut progress, None, "Scanning home folder…");

    let walker = jwalk::WalkDir::new(safelist.home())
        .skip_hidden(false)
        .follow_links(false);

    let mut scanned = 0u64;
    for entry in walker {
        if cancel.is_cancelled() {
            break;
        }
        let Ok(entry) = entry else { continue };
        let path = entry.path();

        if safelist.is_protected(&path) {
            continue;
        }
        let Ok(meta) = entry.metadata() else { continue };
        if !meta.is_file() {
            continue;
        }

        scanned += 1;
        if scanned % 2000 == 0 {
            report(&mut progress, None, format!("Scanned {scanned} files…"));
        }

        let size = meta.blocks() * BLOCK_UNIT;
        if size < min_bytes {
            continue;
        }
        if let Some(cutoff) = age_cutoff {
            let mtime = meta.mtime() as u64;
            if mtime > cutoff {
                continue; // too recently modified
            }
        }

        let label = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| path.display().to_string());

        result.items.push(ScanItem {
            category: "Large File".to_string(),
            label,
            path,
            size,
            safety: Safety::Risky, // user data — never auto-select
            delete_mode: DeleteMode::Trash,
            selected: false,
            note: None,
        });
    }

    // Biggest first.
    result.items.sort_by(|a, b| b.size.cmp(&a.size));
    report(&mut progress, Some(1.0), "Scan complete");
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn finds_files_over_threshold() {
        let dir = tempfile::tempdir().unwrap();
        let big = dir.path().join("big.bin");
        let mut f = std::fs::File::create(&big).unwrap();
        f.write_all(&vec![0u8; 200_000]).unwrap();
        f.sync_all().unwrap();
        // small file should be excluded
        std::fs::write(dir.path().join("small.txt"), b"hi").unwrap();

        let sl = Safelist::new(dir.path(), vec![]);
        let res = scan(&sl, 100_000, None, &CancelToken::new(), None);
        assert_eq!(res.items.len(), 1);
        assert!(res.items[0].label.contains("big.bin"));
        assert_eq!(res.items[0].delete_mode, DeleteMode::Trash);
    }
}
