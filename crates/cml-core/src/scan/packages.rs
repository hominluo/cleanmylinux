//! Package Cleanup scanner (read-only estimation, user-space).
//!
//! This module only *measures* reclaimable space and lists candidates. The
//! actual removals are privileged and go through `cml-helper` (see
//! [`crate::helper_ipc`]). The `path` field carries a synthetic marker so the
//! frontend can map a row back to the [`crate::helper_ipc::HelperOp`] to run.

use crate::cmd;
use crate::fsutil::dir_size;
use crate::progress::{report, CancelToken, Progress};
use crate::types::{DeleteMode, Safety, ScanItem, ScanResult};
use std::path::PathBuf;

/// Synthetic path scheme used to tag privileged rows, e.g. `priv://apt_clean`.
pub fn priv_marker(op: &str) -> PathBuf {
    PathBuf::from(format!("priv://{op}"))
}

const FLATPAK_UNUSED_DRY_RUN_ARGS: &[&str] = &[
    "uninstall",
    "--unused",
    "--system",
    "--noninteractive",
    "--dry-run",
];

pub fn scan(cancel: &CancelToken, mut progress: Option<&mut Progress<'_>>) -> ScanResult {
    let mut result = ScanResult::default();

    // 1) apt archive cache — directory is world-readable, so we can size it now.
    report(&mut progress, Some(0.1), "Measuring apt cache…");
    let apt_archives = PathBuf::from("/var/cache/apt/archives");
    if apt_archives.exists() {
        let size = dir_size(&apt_archives, cancel);
        if size > 0 {
            result.items.push(ScanItem {
                category: "APT Cache".into(),
                label: "Downloaded package files".into(),
                path: priv_marker("apt_clean"),
                size,
                safety: Safety::Safe,
                delete_mode: DeleteMode::Permanent,
                selected: true,
                note: None,
            });
        }
    }

    if cancel.is_cancelled() {
        return result;
    }

    // 2) Orphaned packages (apt-mark / autoremove candidates).
    report(&mut progress, Some(0.4), "Finding orphaned packages…");
    let auto = cmd::run_lenient("apt-get", &["-s", "autoremove"]);
    let removable = auto.lines().filter(|l| l.starts_with("Remv ")).count();
    if removable > 0 {
        result.items.push(ScanItem {
            category: "Orphaned Packages".into(),
            label: format!("{removable} unused dependencies"),
            path: priv_marker("apt_autoremove"),
            // Size is reported by the simulation line if present; otherwise 0
            // (the helper reports exact freed bytes after the real run).
            size: parse_autoremove_freed(&auto),
            safety: Safety::Review,
            delete_mode: DeleteMode::Permanent,
            selected: false,
            note: None,
        });
    }

    // 3) Old kernels.
    report(&mut progress, Some(0.6), "Checking installed kernels…");
    let kernels = count_removable_kernels();
    if kernels > 0 {
        result.items.push(ScanItem {
            category: "Old Kernels".into(),
            label: format!("{kernels} old kernel package(s)"),
            path: priv_marker("prune_old_kernels"),
            size: 0, // exact size computed by helper; UI shows count
            safety: Safety::Review,
            delete_mode: DeleteMode::Permanent,
            selected: false,
            note: Some("Keeps the running and newest kernel".into()),
        });
    }

    // 4) systemd journal size.
    report(&mut progress, Some(0.8), "Measuring system journal…");
    if cmd::has("journalctl") {
        let out = cmd::run_lenient("journalctl", &["--disk-usage"]);
        if let Some(bytes) = parse_journal_usage(&out) {
            if bytes > 50 * 1024 * 1024 {
                result.items.push(ScanItem {
                    category: "System Logs".into(),
                    label: "systemd journal".into(),
                    path: priv_marker("vacuum_journal"),
                    size: bytes,
                    safety: Safety::Review,
                    delete_mode: DeleteMode::Permanent,
                    selected: false,
                    note: Some("Vacuums to a smaller cap".into()),
                });
            }
        }
    }

    // 5) Unused flatpak runtimes (user-removable, but listed here too).
    report(&mut progress, Some(0.95), "Checking flatpak runtimes…");
    if cmd::has("flatpak") {
        let unused = cmd::run_lenient("flatpak", FLATPAK_UNUSED_DRY_RUN_ARGS);
        if flatpak_dry_run_has_unused(&unused) {
            result.items.push(ScanItem {
                category: "Flatpak Runtimes".into(),
                label: "Unused system runtimes".into(),
                path: priv_marker("flatpak_remove_unused"),
                size: 0,
                safety: Safety::Review,
                delete_mode: DeleteMode::Permanent,
                selected: false,
                note: Some("Exact size is reported after cleanup".into()),
            });
        }
    }

    report(&mut progress, Some(1.0), "Scan complete");
    result
}

fn flatpak_dry_run_has_unused(out: &str) -> bool {
    let normalized = out.trim().to_lowercase();
    !normalized.is_empty()
        && !normalized.contains("nothing unused")
        && !normalized.contains("no unused")
        && !normalized.contains("error")
}

/// Parse "After this operation, NNN MB disk space will be freed." from apt -s.
fn parse_autoremove_freed(sim: &str) -> u64 {
    for line in sim.lines() {
        if let Some(rest) = line.strip_prefix("After this operation, ") {
            if let Some(idx) = rest.find(" disk space will be freed") {
                let qty = &rest[..idx];
                return parse_apt_size(qty);
            }
        }
    }
    0
}

/// Parse apt size strings like "123 MB" / "1,024 kB" / "2.0 GB" into bytes.
fn parse_apt_size(s: &str) -> u64 {
    let s = s.replace(',', "");
    let mut parts = s.split_whitespace();
    let Some(num) = parts.next().and_then(|n| n.parse::<f64>().ok()) else {
        return 0;
    };
    let mult = match parts.next() {
        Some("kB") => 1_000.0,
        Some("MB") => 1_000_000.0,
        Some("GB") => 1_000_000_000.0,
        Some("B") | None => 1.0,
        _ => 1.0,
    };
    (num * mult) as u64
}

/// Parse "Archived and active journals take up 1.2G in the file system."
fn parse_journal_usage(s: &str) -> Option<u64> {
    let idx = s.find("take up ")?;
    let rest = &s[idx + "take up ".len()..];
    let token: String = rest.chars().take_while(|c| !c.is_whitespace()).collect();
    parse_iec_size(&token)
}

/// Parse IEC-ish size token like "1.2G", "512M", "900K", "123B".
fn parse_iec_size(token: &str) -> Option<u64> {
    let token = token.trim_end_matches('B');
    let (num, mult) = if let Some(n) = token.strip_suffix('G') {
        (n, 1u64 << 30)
    } else if let Some(n) = token.strip_suffix('M') {
        (n, 1u64 << 20)
    } else if let Some(n) = token.strip_suffix('K') {
        (n, 1u64 << 10)
    } else {
        (token, 1)
    };
    num.parse::<f64>().ok().map(|v| (v * mult as f64) as u64)
}

/// Count old kernel image packages that could be removed (excluding running).
fn count_removable_kernels() -> usize {
    let running = cmd::run_lenient("uname", &["-r"]);
    let running = running.trim();
    let list = cmd::run_lenient("dpkg-query", &["-W", "-f=${Package}\n", "linux-image-*"]);
    list.lines()
        .filter(|pkg| {
            pkg.starts_with("linux-image-")
                && !pkg.contains(running)
                && pkg.chars().any(|c| c.is_ascii_digit())
        })
        .count()
        .saturating_sub(1) // keep newest as well
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_apt_sizes() {
        assert_eq!(parse_apt_size("123 MB"), 123_000_000);
        assert_eq!(parse_apt_size("1,024 kB"), 1_024_000);
        assert_eq!(parse_apt_size("2.0 GB"), 2_000_000_000);
    }

    #[test]
    fn parses_journal_usage_line() {
        let s = "Archived and active journals take up 1.2G in the file system.";
        assert_eq!(
            parse_journal_usage(s),
            Some((1.2 * (1u64 << 30) as f64) as u64)
        );
    }

    #[test]
    fn priv_marker_roundtrip() {
        assert_eq!(
            priv_marker("apt_clean").to_string_lossy(),
            "priv://apt_clean"
        );
    }

    #[test]
    fn flatpak_probe_is_dry_run_only() {
        assert!(FLATPAK_UNUSED_DRY_RUN_ARGS.contains(&"--dry-run"));
        assert!(FLATPAK_UNUSED_DRY_RUN_ARGS.contains(&"--noninteractive"));
        assert!(!FLATPAK_UNUSED_DRY_RUN_ARGS.contains(&"--assumeyes"));
        assert!(!FLATPAK_UNUSED_DRY_RUN_ARGS.contains(&"-y"));
    }

    #[test]
    fn parses_flatpak_unused_probe() {
        assert!(!flatpak_dry_run_has_unused(""));
        assert!(!flatpak_dry_run_has_unused("Nothing unused to uninstall"));
        assert!(flatpak_dry_run_has_unused(
            "Would uninstall:\norg.example.Runtime"
        ));
    }
}
