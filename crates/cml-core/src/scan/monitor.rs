//! System Monitor: live CPU / memory / swap / disk snapshot via `sysinfo`.
//!
//! Frontends poll [`Monitor::sample`] on a ~1s timer and render the result.

use serde::{Deserialize, Serialize};
use sysinfo::{Disks, System};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskInfo {
    pub mount: String,
    pub total: u64,
    pub available: u64,
}

impl DiskInfo {
    pub fn used(&self) -> u64 {
        self.total.saturating_sub(self.available)
    }
    pub fn used_fraction(&self) -> f64 {
        if self.total == 0 {
            0.0
        } else {
            self.used() as f64 / self.total as f64
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemSample {
    /// Overall CPU usage 0.0..=1.0.
    pub cpu: f64,
    pub mem_total: u64,
    pub mem_used: u64,
    pub swap_total: u64,
    pub swap_used: u64,
    pub disks: Vec<DiskInfo>,
}

impl SystemSample {
    pub fn mem_fraction(&self) -> f64 {
        frac(self.mem_used, self.mem_total)
    }
    pub fn swap_fraction(&self) -> f64 {
        frac(self.swap_used, self.swap_total)
    }
}

fn frac(used: u64, total: u64) -> f64 {
    if total == 0 {
        0.0
    } else {
        used as f64 / total as f64
    }
}

/// Holds the `sysinfo` state across samples (needed for accurate CPU deltas).
pub struct Monitor {
    sys: System,
    disks: Disks,
}

impl Default for Monitor {
    fn default() -> Self {
        Self::new()
    }
}

impl Monitor {
    pub fn new() -> Self {
        Self {
            sys: System::new(),
            disks: Disks::new_with_refreshed_list(),
        }
    }

    /// Take a fresh sample. Call repeatedly (≈1s apart) for live CPU values.
    pub fn sample(&mut self) -> SystemSample {
        self.sys.refresh_cpu_usage();
        self.sys.refresh_memory();
        self.disks.refresh();

        let cpu = (self.sys.global_cpu_usage() as f64 / 100.0).clamp(0.0, 1.0);

        let disks = self
            .disks
            .list()
            .iter()
            .filter(|d| {
                // Only show real, sizeable mounts (skip loopback/snap squashfs).
                d.total_space() > 0 && !d.mount_point().starts_with("/snap")
            })
            .map(|d| DiskInfo {
                mount: d.mount_point().to_string_lossy().to_string(),
                total: d.total_space(),
                available: d.available_space(),
            })
            .collect();

        SystemSample {
            cpu,
            mem_total: self.sys.total_memory(),
            mem_used: self.sys.used_memory(),
            swap_total: self.sys.total_swap(),
            swap_used: self.sys.used_swap(),
            disks,
        }
    }
}
