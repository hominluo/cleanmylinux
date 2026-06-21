//! Filesystem helpers with correctness baked in: actual block usage (not
//! apparent size), no symlink following, hardlinked inodes counted once, and
//! cooperative cancellation.

use crate::progress::CancelToken;
use std::collections::HashSet;
use std::os::unix::fs::MetadataExt;
use std::path::Path;

/// Block size assumed for `st_blocks` (POSIX defines it in 512-byte units).
const BLOCK_UNIT: u64 = 512;

/// Real on-disk usage of a single file's metadata (blocks × 512).
fn disk_usage(meta: &std::fs::Metadata) -> u64 {
    meta.blocks() * BLOCK_UNIT
}

/// On-disk usage of one filesystem entry, using the same convention as
/// [`dir_size`]: real block usage, symlinks (and other non-regular files) count
/// as zero, and each hardlinked inode is counted only once via `seen`.
///
/// Pass `meta` from `symlink_metadata` (no link following) and a `seen` set
/// shared across the walk so callers tallying freed space match what the scan
/// reported.
pub fn entry_disk_usage(meta: &std::fs::Metadata, seen: &mut HashSet<(u64, u64)>) -> u64 {
    if !meta.is_file() {
        return 0;
    }
    if meta.nlink() > 1 && !seen.insert((meta.dev(), meta.ino())) {
        return 0;
    }
    disk_usage(meta)
}

/// Compute the actual disk usage of a directory tree (or single file).
///
/// - Does not follow symlinks (avoids loops and escaping the tree).
/// - Counts each hardlinked inode only once.
/// - Returns early (with the partial sum) if `cancel` is tripped.
pub fn dir_size(root: &Path, cancel: &CancelToken) -> u64 {
    let mut seen_inodes: HashSet<(u64, u64)> = HashSet::new();
    let mut total = 0u64;

    // Fast path for a plain file.
    if let Ok(meta) = std::fs::symlink_metadata(root) {
        if meta.is_file() {
            return disk_usage(&meta);
        }
        if meta.is_symlink() {
            return 0;
        }
    }

    let walker = jwalk::WalkDir::new(root)
        .skip_hidden(false)
        .follow_links(false);

    for entry in walker {
        if cancel.is_cancelled() {
            break;
        }
        let Ok(entry) = entry else { continue };
        let Ok(meta) = entry.metadata() else { continue };
        total += entry_disk_usage(&meta, &mut seen_inodes);
    }
    total
}

/// Whether a directory exists and is non-empty (cheap check used to decide if a
/// junk category is worth showing).
pub fn is_nonempty_dir(path: &Path) -> bool {
    std::fs::read_dir(path)
        .map(|mut it| it.next().is_some())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn sums_file_sizes() {
        let dir = tempfile::tempdir().unwrap();
        let mut f = std::fs::File::create(dir.path().join("a.bin")).unwrap();
        f.write_all(&vec![0u8; 8192]).unwrap();
        f.sync_all().unwrap();
        let size = dir_size(dir.path(), &CancelToken::new());
        // At least the data we wrote (block-rounded, so >= 8192).
        assert!(size >= 8192, "got {size}");
    }

    #[test]
    fn empty_dir_is_zero() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(dir_size(dir.path(), &CancelToken::new()), 0);
        assert!(!is_nonempty_dir(dir.path()));
    }
}
