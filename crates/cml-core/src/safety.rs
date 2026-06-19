//! Protected-paths safelist — the heart of making a cleaner trustworthy.
//!
//! These paths must NEVER be deleted, even though some of them live under
//! directories we otherwise clean (e.g. `~/.cache`). The list is built-in and
//! not user-overridable; users can only *add* their own exclusions on top.

use std::path::{Path, PathBuf};

/// Directories (relative to `$HOME`) that hold credentials / irreplaceable data
/// and must never be removed by any cleaner.
const PROTECTED_HOME_DIRS: &[&str] = &[
    ".gnupg",
    ".ssh",
    ".password-store",
    ".local/share/keyrings",
    ".cache/keyring",
    ".config/dconf",
    ".mozilla", // Firefox profile root — only its Cache subdirs are cleanable
    ".thunderbird",
];

/// Absolute path prefixes that are pseudo-filesystems or otherwise off-limits to
/// any filesystem walk or deletion.
const FORBIDDEN_PREFIXES: &[&str] = &["/proc", "/sys", "/dev", "/run", "/boot", "/snap"];

/// Decides whether a given path is allowed to be scanned/deleted. Holds both the
/// built-in safelist and the user's personal exclusions.
#[derive(Debug, Clone, Default)]
pub struct Safelist {
    home: PathBuf,
    protected: Vec<PathBuf>,
    user_exclusions: Vec<PathBuf>,
}

impl Safelist {
    /// Build the safelist for the current user's home directory.
    pub fn new(home: impl Into<PathBuf>, user_exclusions: Vec<PathBuf>) -> Self {
        let home = home.into();
        let protected = PROTECTED_HOME_DIRS
            .iter()
            .map(|rel| home.join(rel))
            .collect();
        Self {
            home,
            protected,
            user_exclusions,
        }
    }

    pub fn home(&self) -> &Path {
        &self.home
    }

    /// True if `path` is protected and must not be touched. A path is protected
    /// if it *is* or is *inside* any protected entry, user exclusion, or a
    /// forbidden system prefix.
    pub fn is_protected(&self, path: &Path) -> bool {
        if FORBIDDEN_PREFIXES.iter().any(|p| path.starts_with(p)) {
            return true;
        }
        self.protected
            .iter()
            .chain(self.user_exclusions.iter())
            .any(|prot| path == prot || path.starts_with(prot))
    }

    /// True if the path is safe to act on (the inverse of [`is_protected`], and
    /// it must live under `$HOME` for user-space cleaners).
    pub fn is_safe_user_path(&self, path: &Path) -> bool {
        path.starts_with(&self.home) && !self.is_protected(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protects_credentials_under_home() {
        let sl = Safelist::new("/home/u", vec![]);
        assert!(sl.is_protected(Path::new("/home/u/.ssh")));
        assert!(sl.is_protected(Path::new("/home/u/.ssh/id_ed25519")));
        assert!(sl.is_protected(Path::new("/home/u/.gnupg/secring")));
        assert!(sl.is_protected(Path::new("/home/u/.mozilla/firefox/abc.default")));
    }

    #[test]
    fn allows_plain_cache() {
        let sl = Safelist::new("/home/u", vec![]);
        assert!(!sl.is_protected(Path::new("/home/u/.cache/thumbnails")));
        assert!(sl.is_safe_user_path(Path::new("/home/u/.cache/thumbnails")));
    }

    #[test]
    fn rejects_system_and_outside_home() {
        let sl = Safelist::new("/home/u", vec![]);
        assert!(sl.is_protected(Path::new("/proc/1")));
        assert!(!sl.is_safe_user_path(Path::new("/etc/passwd")));
    }

    #[test]
    fn honors_user_exclusions() {
        let sl = Safelist::new("/home/u", vec![PathBuf::from("/home/u/.cache/keepme")]);
        assert!(sl.is_protected(Path::new("/home/u/.cache/keepme/data")));
    }
}
