//! Uninstaller: inventory of installed applications across apt, flatpak and snap.

use crate::cmd;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AppSource {
    Apt,
    Flatpak,
    Snap,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledApp {
    pub name: String,
    /// Package / app id used for removal.
    pub id: String,
    pub version: String,
    pub source: AppSource,
    /// Best-effort installed size in bytes (0 if unknown).
    pub size: u64,
}

/// Inventory all installed user-facing apps from every available source.
pub fn inventory() -> Vec<InstalledApp> {
    let mut apps = Vec::new();
    apps.extend(apt_apps());
    apps.extend(flatpak_apps());
    apps.extend(snap_apps());
    apps.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    apps
}

fn apt_apps() -> Vec<InstalledApp> {
    // Installed-Size is in KiB.
    let out = cmd::run_lenient(
        "dpkg-query",
        &["-W", "-f=${Package}\t${Version}\t${Installed-Size}\n"],
    );
    out.lines()
        .filter_map(|line| {
            let mut f = line.split('\t');
            let pkg = f.next()?.to_string();
            let ver = f.next().unwrap_or("").to_string();
            let size_kib: u64 = f.next().unwrap_or("0").trim().parse().unwrap_or(0);
            Some(InstalledApp {
                name: pkg.clone(),
                id: pkg,
                version: ver,
                source: AppSource::Apt,
                size: size_kib * 1024,
            })
        })
        .collect()
}

fn flatpak_apps() -> Vec<InstalledApp> {
    if !cmd::has("flatpak") {
        return Vec::new();
    }
    let out = cmd::run_lenient(
        "flatpak",
        &["list", "--app", "--columns=name,application,version,size"],
    );
    out.lines()
        .filter_map(|line| {
            let mut f = line.split('\t');
            let name = f.next()?.trim().to_string();
            let id = f.next().unwrap_or("").trim().to_string();
            let ver = f.next().unwrap_or("").trim().to_string();
            let size = crate::scan::packages_size_from_human(f.next().unwrap_or(""));
            if id.is_empty() {
                return None;
            }
            Some(InstalledApp {
                name,
                id,
                version: ver,
                source: AppSource::Flatpak,
                size,
            })
        })
        .collect()
}

fn snap_apps() -> Vec<InstalledApp> {
    if !cmd::has("snap") {
        return Vec::new();
    }
    // `snap list` is column-formatted text; skip the header row.
    let out = cmd::run_lenient("snap", &["list"]);
    out.lines()
        .skip(1)
        .filter_map(|line| {
            let mut f = line.split_whitespace();
            let name = f.next()?.to_string();
            let ver = f.next().unwrap_or("").to_string();
            Some(InstalledApp {
                name: name.clone(),
                id: name,
                version: ver,
                source: AppSource::Snap,
                size: 0, // snap doesn't report size in `list`
            })
        })
        .collect()
}
