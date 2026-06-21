//! Deletion executors. User-space items are removed here (permanently or to the
//! Trash); privileged items are dispatched to `cml-helper` via `pkexec`.
//!
//! Defense in depth: even though scanners already exclude protected paths, every
//! deletion is re-checked against the [`Safelist`] right before it happens.

use crate::fsutil::dir_size;
use crate::helper_ipc::{HelperOp, HelperRequest, HelperResponse};
use crate::progress::{report, CancelToken, Progress};
use crate::safety::Safelist;
use crate::types::{CleanReport, DeleteMode, ScanItem};
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

#[derive(Debug, Default)]
struct DeleteOutcome {
    freed_bytes: u64,
    skipped_protected: usize,
}

/// Clean the selected user-space items. Privileged items (those whose path uses
/// the `priv://` marker) are skipped here — route them through
/// [`run_privileged`] instead. Reports per-item failures without aborting.
pub fn clean_items(
    items: &[ScanItem],
    safelist: &Safelist,
    cancel: &CancelToken,
    mut progress: Option<&mut Progress<'_>>,
) -> CleanReport {
    let mut report_out = CleanReport::default();
    let selected: Vec<&ScanItem> = items.iter().filter(|i| i.selected).collect();
    let total = selected.len().max(1) as f64;

    for (i, item) in selected.iter().enumerate() {
        if cancel.is_cancelled() {
            break;
        }
        report(
            &mut progress,
            Some(i as f64 / total),
            format!("Cleaning {}…", item.label),
        );

        // Skip privileged markers — not our job here.
        if is_priv_marker(&item.path) {
            continue;
        }

        // Re-verify safety immediately before deleting.
        if !safelist.is_safe_user_path(&item.path) {
            report_out
                .failures
                .push((item.path.clone(), "blocked by safelist".into()));
            continue;
        }

        match delete_one(&item.path, item.delete_mode, safelist, cancel) {
            Ok(outcome) => {
                report_out.freed_bytes += outcome.freed_bytes;
                report_out.removed += 1;
                if outcome.skipped_protected > 0 {
                    report_out.failures.push((
                        item.path.clone(),
                        format!(
                            "skipped {} protected nested path(s)",
                            outcome.skipped_protected
                        ),
                    ));
                }
            }
            Err(e) => report_out.failures.push((item.path.clone(), e.to_string())),
        }
    }

    report(&mut progress, Some(1.0), "Cleanup complete");
    report_out
}

fn delete_one(
    path: &Path,
    mode: DeleteMode,
    safelist: &Safelist,
    cancel: &CancelToken,
) -> anyhow::Result<DeleteOutcome> {
    if !path.exists() {
        return Ok(DeleteOutcome::default());
    }
    // Measure before removing so we can report freed bytes.
    let before = dir_size(path, cancel);

    match mode {
        DeleteMode::Trash => {
            trash::delete(path)?;
            Ok(DeleteOutcome {
                freed_bytes: before,
                skipped_protected: 0,
            })
        }
        DeleteMode::Permanent => {
            let meta = std::fs::symlink_metadata(path)?;
            if meta.is_dir() {
                // Remove directory *contents* but keep the well-known dir itself
                // (e.g. ~/.cache should continue to exist).
                let skipped_protected = remove_dir_contents(path, safelist)?;
                let after = dir_size(path, cancel);
                Ok(DeleteOutcome {
                    freed_bytes: before.saturating_sub(after),
                    skipped_protected,
                })
            } else {
                std::fs::remove_file(path)?;
                Ok(DeleteOutcome {
                    freed_bytes: before,
                    skipped_protected: 0,
                })
            }
        }
    }
}

/// Remove everything inside `dir` but leave `dir` in place.
fn remove_dir_contents(dir: &Path, safelist: &Safelist) -> std::io::Result<usize> {
    let mut skipped_protected = 0;
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let p = entry.path();

        if !safelist.is_safe_user_path(&p) {
            skipped_protected += 1;
            continue;
        }

        let meta = std::fs::symlink_metadata(&p)?;
        if meta.is_dir() && !meta.is_symlink() {
            skipped_protected += remove_dir_contents(&p, safelist)?;
            match std::fs::remove_dir(&p) {
                Ok(()) => {}
                Err(e) if is_directory_not_empty(&e) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(e),
            }
        } else {
            std::fs::remove_file(&p)?;
        }
    }
    Ok(skipped_protected)
}

fn is_directory_not_empty(err: &std::io::Error) -> bool {
    // Rust 1.80 does not have ErrorKind::DirectoryNotEmpty yet.
    matches!(err.raw_os_error(), Some(39) | Some(66))
}

fn is_priv_marker(path: &Path) -> bool {
    path.to_string_lossy().starts_with("priv://")
}

/// One-shot cleanup used by both frontends: deletes selected user-space items
/// and dispatches selected privileged (`priv://`) items to the helper. Returns
/// the combined report plus a human summary of the privileged results.
pub fn clean_selected(
    items: &[ScanItem],
    safelist: &Safelist,
    cancel: &CancelToken,
    progress: Option<&mut Progress<'_>>,
) -> (CleanReport, String) {
    // User-space deletions.
    let mut report = clean_items(items, safelist, cancel, progress);

    // Privileged deletions (selected priv:// rows).
    let mut priv_notes = Vec::new();
    for item in items.iter().filter(|i| i.selected) {
        let marker = item.path.to_string_lossy();
        if let Some(op) = HelperOp::from_marker(&marker) {
            match run_privileged(&HelperRequest::new(op)) {
                Ok(resp) => {
                    report.freed_bytes += resp.freed_bytes;
                    priv_notes.push(resp.message);
                }
                Err(e) => priv_notes.push(format!("{}: {e}", item.label)),
            }
        }
    }

    (report, priv_notes.join("; "))
}

/// Path to the installed privileged helper.
pub const HELPER_PATH: &str = "/usr/libexec/cleanmylinux-helper";

/// Dispatch a privileged operation: `pkexec <helper>`, request JSON on stdin,
/// response JSON on stdout. The polkit prompt is shown by `pkexec` itself.
pub fn run_privileged(request: &HelperRequest) -> anyhow::Result<HelperResponse> {
    let payload = serde_json::to_vec(request)?;

    let mut child = Command::new("pkexec")
        .arg(HELPER_PATH)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    child
        .stdin
        .take()
        .ok_or_else(|| anyhow::anyhow!("no stdin"))?
        .write_all(&payload)?;

    let output = child.wait_with_output()?;
    if !output.status.success() {
        // pkexec exits 126/127 when the user dismisses/auth fails.
        anyhow::bail!(
            "helper failed ({}): {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    let resp: HelperResponse = serde_json::from_slice(&output.stdout)?;
    Ok(resp)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Safety, ScanItem};

    fn item(path: std::path::PathBuf, mode: DeleteMode) -> ScanItem {
        ScanItem {
            category: "t".into(),
            label: "t".into(),
            path,
            size: 0,
            safety: Safety::Safe,
            delete_mode: mode,
            selected: true,
            note: None,
        }
    }

    #[test]
    fn permanently_clears_dir_contents_but_keeps_dir() {
        let home = tempfile::tempdir().unwrap();
        let cache = home.path().join(".cache");
        std::fs::create_dir_all(cache.join("sub")).unwrap();
        std::fs::write(cache.join("sub/a.bin"), vec![0u8; 4096]).unwrap();

        let sl = Safelist::new(home.path(), vec![]);
        let items = vec![item(cache.clone(), DeleteMode::Permanent)];
        let rep = clean_items(&items, &sl, &CancelToken::new(), None);

        assert_eq!(rep.removed, 1);
        assert!(cache.exists(), "the .cache dir itself must remain");
        assert!(!cache.join("sub").exists(), "contents should be gone");
    }

    #[test]
    fn refuses_protected_path() {
        let home = tempfile::tempdir().unwrap();
        let ssh = home.path().join(".ssh");
        std::fs::create_dir_all(&ssh).unwrap();
        std::fs::write(ssh.join("id"), b"secret").unwrap();

        let sl = Safelist::new(home.path(), vec![]);
        let items = vec![item(ssh.clone(), DeleteMode::Permanent)];
        let rep = clean_items(&items, &sl, &CancelToken::new(), None);

        assert_eq!(rep.removed, 0);
        assert_eq!(rep.failures.len(), 1);
        assert!(ssh.join("id").exists(), "protected file must survive");
    }

    #[test]
    fn skips_privileged_markers() {
        let sl = Safelist::new("/home/u", vec![]);
        let items = vec![item("priv://apt_clean".into(), DeleteMode::Permanent)];
        let rep = clean_items(&items, &sl, &CancelToken::new(), None);
        assert_eq!(rep.removed, 0);
        assert_eq!(rep.failures.len(), 0);
    }

    #[test]
    fn recursive_delete_skips_protected_descendants() {
        let home = tempfile::tempdir().unwrap();
        let cache = home.path().join(".cache");
        let protected = cache.join("keyring");
        let removable = cache.join("app");
        std::fs::create_dir_all(&protected).unwrap();
        std::fs::create_dir_all(&removable).unwrap();
        std::fs::write(protected.join("secret"), b"secret").unwrap();
        std::fs::write(removable.join("junk"), b"junk").unwrap();

        let sl = Safelist::new(home.path(), vec![]);
        let items = vec![item(cache.clone(), DeleteMode::Permanent)];
        let rep = clean_items(&items, &sl, &CancelToken::new(), None);

        assert_eq!(rep.removed, 1);
        assert_eq!(rep.failures.len(), 1);
        assert!(protected.join("secret").exists());
        assert!(!removable.exists());
        assert!(cache.exists());
    }
}
