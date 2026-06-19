//! IPC contract between the frontends and the privileged `cml-helper`.
//!
//! The frontend serializes a [`HelperRequest`] to JSON on the helper's stdin;
//! the helper replies with a [`HelperResponse`] on stdout. The op set is a
//! **closed enum** — the helper never accepts caller-supplied raw paths for a
//! root deletion, which keeps the privileged attack surface tiny.

use serde::{Deserialize, Serialize};

/// A privileged operation the helper is allowed to perform. This is the entire
/// universe of things that can run as root — nothing else is possible.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum HelperOp {
    /// `apt-get clean` — empty the downloaded-package cache.
    AptClean,
    /// `apt-get autoremove --purge` — remove orphaned dependencies.
    AptAutoremove,
    /// Remove old kernels. The helper computes the candidate list itself
    /// (keeping the running kernel + newest); callers cannot name packages.
    PruneOldKernels { keep: u32 },
    /// `journalctl --vacuum-size=<MB>` — shrink the systemd journal.
    VacuumJournal { max_mb: u64 },
    /// Drop old/disabled snap revisions.
    PruneSnapRevisions,
    /// `flatpak uninstall --unused` (system installation).
    FlatpakRemoveUnused,
    /// A read-only probe used to estimate reclaimable space without changing
    /// anything (used to populate the UI before the user confirms).
    EstimateOnly { what: PrivilegedTarget },
}

/// What an [`HelperOp::EstimateOnly`] should measure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrivilegedTarget {
    AptCache,
    OrphanPackages,
    OldKernels,
    Journal,
    SnapRevisions,
    FlatpakUnused,
}

impl HelperOp {
    /// Map a `priv://<marker>` path (produced by the package scanner) to the
    /// privileged op that performs it. Returns `None` for unknown markers.
    pub fn from_marker(marker: &str) -> Option<Self> {
        let key = marker.strip_prefix("priv://").unwrap_or(marker);
        match key {
            "apt_clean" => Some(HelperOp::AptClean),
            "apt_autoremove" => Some(HelperOp::AptAutoremove),
            "prune_old_kernels" => Some(HelperOp::PruneOldKernels { keep: 1 }),
            "vacuum_journal" => Some(HelperOp::VacuumJournal { max_mb: 500 }),
            "prune_snap_revisions" => Some(HelperOp::PruneSnapRevisions),
            "flatpak_remove_unused" => Some(HelperOp::FlatpakRemoveUnused),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HelperRequest {
    /// Protocol version so the helper can reject mismatched callers.
    pub version: u32,
    pub op: HelperOp,
    /// If true, do not actually change anything — just report what would happen.
    pub dry_run: bool,
}

impl HelperRequest {
    pub const PROTOCOL_VERSION: u32 = 1;

    pub fn new(op: HelperOp) -> Self {
        Self {
            version: Self::PROTOCOL_VERSION,
            op,
            dry_run: false,
        }
    }

    pub fn dry(op: HelperOp) -> Self {
        Self {
            version: Self::PROTOCOL_VERSION,
            op,
            dry_run: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HelperResponse {
    pub ok: bool,
    /// Bytes freed (or estimated to be freed for a dry run / estimate).
    pub freed_bytes: u64,
    /// Human-readable detail or error message.
    pub message: String,
}

impl HelperResponse {
    pub fn ok(freed_bytes: u64, message: impl Into<String>) -> Self {
        Self {
            ok: true,
            freed_bytes,
            message: message.into(),
        }
    }

    pub fn err(message: impl Into<String>) -> Self {
        Self {
            ok: false,
            freed_bytes: 0,
            message: message.into(),
        }
    }
}
