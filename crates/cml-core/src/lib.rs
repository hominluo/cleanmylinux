//! # cml-core
//!
//! The shared, UI-agnostic engine behind CleanMyLinux. Both the GNOME (GTK) and
//! KDE (Qt) frontends depend on this crate and nothing of each other — all
//! scanning, sizing, safety, configuration and cleanup logic lives here so it is
//! written once and unit-tested headless.
//!
//! Key entry points:
//! - [`Engine`] — high-level facade a frontend holds for the session.
//! - [`scan`] — per-module read-only scanners.
//! - [`clean`] — user-space deletion + privileged dispatch.
//! - [`safety::Safelist`] — the protected-paths guard.

pub mod clean;
pub mod cmd;
pub mod config;
pub mod fsutil;
pub mod helper_ipc;
pub mod process;
pub mod progress;
pub mod safety;
pub mod scan;
pub mod types;

use crate::config::Config;
use crate::progress::{CancelToken, Progress};
use crate::safety::Safelist;
use crate::types::{Module, ScanResult};
use std::path::PathBuf;

/// Session-level facade. Construct once per app run; cheap to clone the pieces
/// you need into worker threads.
pub struct Engine {
    pub config: Config,
    pub safelist: Safelist,
}

impl Default for Engine {
    fn default() -> Self {
        Self::new()
    }
}

impl Engine {
    /// Build the engine from on-disk config and the current user's home.
    pub fn new() -> Self {
        let config = Config::load();
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"));
        let safelist = Safelist::new(home, config.exclusions.clone());
        Self { config, safelist }
    }

    /// Run the read-only scanner for a single module.
    pub fn scan_module(
        &self,
        module: Module,
        cancel: &CancelToken,
        progress: Option<&mut Progress<'_>>,
    ) -> ScanResult {
        match module {
            Module::SystemJunk => scan::junk::scan(&self.safelist, cancel, progress),
            Module::LargeAndOldFiles => scan::large_files::scan(
                &self.safelist,
                self.config.large_file_threshold,
                Some(self.config.old_file_days),
                cancel,
                progress,
            ),
            Module::PackageCleanup => scan::packages::scan(cancel, progress),
            // Uninstaller and Monitor aren't reclaimable-space scans; frontends
            // call their dedicated APIs (apps::inventory / monitor::Monitor).
            Module::Uninstaller | Module::SystemMonitor => ScanResult::default(),
        }
    }

    /// **Smart Scan**: run every reclaimable-space module and merge results into
    /// one list — this powers the flagship dashboard gauge.
    pub fn smart_scan(
        &self,
        cancel: &CancelToken,
        mut progress: Option<&mut Progress<'_>>,
    ) -> ScanResult {
        let mut merged = ScanResult::default();
        let modules = [
            Module::SystemJunk,
            Module::PackageCleanup,
            Module::LargeAndOldFiles,
        ];
        let n = modules.len() as f64;
        for (i, m) in modules.iter().enumerate() {
            if cancel.is_cancelled() {
                break;
            }
            crate::progress::report(
                &mut progress,
                Some(i as f64 / n),
                format!("Scanning {}…", m.title()),
            );
            let res = self.scan_module(*m, cancel, None);
            merged.items.extend(res.items);
        }
        crate::progress::report(&mut progress, Some(1.0), "Smart Scan complete");
        merged
    }
}
