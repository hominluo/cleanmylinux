//! Tiny process inspection via `/proc` — used to avoid touching the live state
//! of running apps (e.g. a browser's cache while the browser is open).

use std::path::Path;

/// True if any running process's command name contains `needle`
/// (case-insensitive substring match against `/proc/<pid>/comm`).
pub fn is_process_running(needle: &str) -> bool {
    let needle = needle.to_lowercase();
    let Ok(entries) = std::fs::read_dir("/proc") else {
        return false;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        // Only numeric entries are PIDs.
        if !name.to_string_lossy().bytes().all(|b| b.is_ascii_digit()) {
            continue;
        }
        let comm = entry.path().join("comm");
        if let Ok(content) = std::fs::read_to_string(&comm) {
            if content.trim().to_lowercase().contains(&needle) {
                return true;
            }
        }
    }
    false
}

/// Whether the apt/dpkg lock is held — i.e. another package operation (or
/// unattended-upgrades) is in progress, so we must not start one.
pub fn dpkg_locked() -> bool {
    // The presence of the lock file doesn't prove a lock is *held*; a held lock
    // is what matters. We approximate by checking for a running apt/dpkg/unattended
    // process, which is robust and needs no privileges.
    const LOCK: &str = "/var/lib/dpkg/lock-frontend";
    if !Path::new(LOCK).exists() {
        return false;
    }
    ["apt", "apt-get", "dpkg", "unattended-upgr", "packagekitd"]
        .iter()
        .any(|p| is_process_running(p))
}
