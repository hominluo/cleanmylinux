//! CleanMyLinux privileged helper.
//!
//! Invoked as `pkexec /usr/libexec/cleanmylinux-helper`. Reads a JSON
//! [`HelperRequest`] from stdin, performs the requested **whitelisted** root
//! operation, and writes a JSON [`HelperResponse`] to stdout.
//!
//! Security posture:
//! - Re-verifies it is actually running as root (`geteuid() == 0`).
//! - Accepts only the closed [`HelperOp`] enum — never a caller-supplied path.
//! - Never spawns a shell; runs system tools directly with a fixed PATH.
//! - Computes destructive candidate lists itself (e.g. which kernels to remove).

use cml_core::cmd;
use cml_core::helper_ipc::{HelperOp, HelperRequest, HelperResponse, PrivilegedTarget};
use std::io::{Read, Write};

fn main() {
    let resp = run();
    let json = serde_json::to_string(&resp).unwrap_or_else(|_| {
        "{\"ok\":false,\"freed_bytes\":0,\"message\":\"serialization error\"}".into()
    });
    let _ = std::io::stdout().write_all(json.as_bytes());
    if !resp.ok {
        std::process::exit(1);
    }
}

fn run() -> HelperResponse {
    // 1) Must be root.
    if !is_root() {
        return HelperResponse::err("helper must run as root (via pkexec)");
    }

    // 2) Read + parse request.
    let mut buf = String::new();
    if std::io::stdin().read_to_string(&mut buf).is_err() {
        return HelperResponse::err("failed to read request");
    }
    let req: HelperRequest = match serde_json::from_str(&buf) {
        Ok(r) => r,
        Err(e) => return HelperResponse::err(format!("invalid request: {e}")),
    };

    // 3) Version gate.
    if req.version != HelperRequest::PROTOCOL_VERSION {
        return HelperResponse::err(format!(
            "protocol mismatch: helper {} vs caller {}",
            HelperRequest::PROTOCOL_VERSION,
            req.version
        ));
    }

    // 4) Dispatch the whitelisted op.
    execute(&req)
}

fn is_root() -> bool {
    // SAFETY: geteuid is always safe to call and has no preconditions.
    unsafe { libc_geteuid() == 0 }
}

// Avoid pulling the whole `libc` crate for one call.
extern "C" {
    #[link_name = "geteuid"]
    fn libc_geteuid() -> u32;
}

fn execute(req: &HelperRequest) -> HelperResponse {
    match &req.op {
        HelperOp::AptClean => apt_clean(req.dry_run),
        HelperOp::AptAutoremove => apt_autoremove(req.dry_run),
        HelperOp::PruneOldKernels { keep } => prune_kernels(*keep, req.dry_run),
        HelperOp::VacuumJournal { max_mb } => vacuum_journal(*max_mb, req.dry_run),
        HelperOp::PruneSnapRevisions => prune_snap(req.dry_run),
        HelperOp::FlatpakRemoveUnused => flatpak_unused(req.dry_run),
        HelperOp::EstimateOnly { what } => estimate(*what),
    }
}

fn apt_clean(dry: bool) -> HelperResponse {
    let before = dir_size("/var/cache/apt/archives");
    if dry {
        return HelperResponse::ok(before, "would clean apt cache");
    }
    match cmd::run("apt-get", &["clean"]) {
        Ok(_) => {
            let after = dir_size("/var/cache/apt/archives");
            HelperResponse::ok(before.saturating_sub(after), "apt cache cleaned")
        }
        Err(e) => HelperResponse::err(e.to_string()),
    }
}

fn apt_autoremove(dry: bool) -> HelperResponse {
    if dry {
        let sim = cmd::run_lenient("apt-get", &["-s", "autoremove"]);
        return HelperResponse::ok(0, format!("dry run:\n{}", sim.lines().take(5).collect::<Vec<_>>().join("\n")));
    }
    match cmd::run("apt-get", &["autoremove", "--purge", "-y"]) {
        Ok(out) => HelperResponse::ok(0, out.lines().last().unwrap_or("done").to_string()),
        Err(e) => HelperResponse::err(e.to_string()),
    }
}

/// Remove old kernel packages, ALWAYS keeping the running kernel + the `keep`
/// newest. The candidate list is computed here; callers cannot name packages.
fn prune_kernels(keep: u32, dry: bool) -> HelperResponse {
    let running = cmd::run_lenient("uname", &["-r"]).trim().to_string();
    let list = cmd::run_lenient("dpkg-query", &["-W", "-f=${Package}\n", "linux-image-*"]);

    let mut versioned: Vec<String> = list
        .lines()
        .filter(|p| p.starts_with("linux-image-") && p.chars().any(|c| c.is_ascii_digit()))
        .map(|s| s.to_string())
        .collect();
    // Sort so newest sorts last (dpkg version ordering is approximated lexically;
    // good enough since we additionally never touch the running kernel).
    versioned.sort();

    let keep_newest = keep.max(1) as usize;
    let mut candidates: Vec<String> = Vec::new();
    let cutoff = versioned.len().saturating_sub(keep_newest);
    for (idx, pkg) in versioned.iter().enumerate() {
        if idx >= cutoff {
            continue; // keep newest N
        }
        if pkg.contains(&running) {
            continue; // never the running kernel
        }
        candidates.push(pkg.clone());
    }

    if candidates.is_empty() {
        return HelperResponse::ok(0, "no removable kernels");
    }
    if dry {
        return HelperResponse::ok(0, format!("would remove: {}", candidates.join(", ")));
    }

    let mut args = vec!["purge", "-y"];
    let refs: Vec<&str> = candidates.iter().map(|s| s.as_str()).collect();
    args.extend(refs);
    match cmd::run("apt-get", &args) {
        Ok(_) => HelperResponse::ok(0, format!("removed {} kernel package(s)", candidates.len())),
        Err(e) => HelperResponse::err(e.to_string()),
    }
}

fn vacuum_journal(max_mb: u64, dry: bool) -> HelperResponse {
    let max_mb = max_mb.clamp(50, 4096);
    if dry {
        return HelperResponse::ok(0, format!("would vacuum journal to {max_mb}M"));
    }
    match cmd::run("journalctl", &[&format!("--vacuum-size={max_mb}M")]) {
        Ok(out) => HelperResponse::ok(0, out.lines().last().unwrap_or("vacuumed").to_string()),
        Err(e) => HelperResponse::err(e.to_string()),
    }
}

fn prune_snap(dry: bool) -> HelperResponse {
    if !cmd::has("snap") {
        return HelperResponse::ok(0, "snap not installed");
    }
    // List disabled (old) revisions.
    let list = cmd::run_lenient("snap", &["list", "--all"]);
    let disabled: Vec<(String, String)> = list
        .lines()
        .skip(1)
        .filter(|l| l.contains("disabled"))
        .filter_map(|l| {
            let mut f = l.split_whitespace();
            let name = f.next()?.to_string();
            let _ver = f.next();
            let rev = f.next()?.to_string();
            Some((name, rev))
        })
        .collect();

    if disabled.is_empty() {
        return HelperResponse::ok(0, "no old snap revisions");
    }
    if dry {
        return HelperResponse::ok(0, format!("would remove {} old snap revision(s)", disabled.len()));
    }
    let mut removed = 0;
    for (name, rev) in &disabled {
        if cmd::run("snap", &["remove", name, "--revision", rev]).is_ok() {
            removed += 1;
        }
    }
    HelperResponse::ok(0, format!("removed {removed} old snap revision(s)"))
}

fn flatpak_unused(dry: bool) -> HelperResponse {
    if !cmd::has("flatpak") {
        return HelperResponse::ok(0, "flatpak not installed");
    }
    if dry {
        let out = cmd::run_lenient("flatpak", &["uninstall", "--unused", "--system", "--noninteractive", "--dry-run"]);
        return HelperResponse::ok(0, format!("dry run:\n{}", out.trim()));
    }
    match cmd::run("flatpak", &["uninstall", "--unused", "--system", "--assumeyes", "--noninteractive"]) {
        Ok(out) => HelperResponse::ok(0, out.lines().last().unwrap_or("done").to_string()),
        Err(e) => HelperResponse::err(e.to_string()),
    }
}

fn estimate(what: PrivilegedTarget) -> HelperResponse {
    let bytes = match what {
        PrivilegedTarget::AptCache => dir_size("/var/cache/apt/archives"),
        PrivilegedTarget::Journal => {
            let out = cmd::run_lenient("journalctl", &["--disk-usage"]);
            out.split("take up ")
                .nth(1)
                .and_then(|s| s.split_whitespace().next())
                .map(parse_iec)
                .unwrap_or(0)
        }
        _ => 0,
    };
    HelperResponse::ok(bytes, "estimate")
}

fn dir_size(path: &str) -> u64 {
    cml_core::fsutil::dir_size(std::path::Path::new(path), &cml_core::progress::CancelToken::new())
}

fn parse_iec(tok: &str) -> u64 {
    let tok = tok.trim_end_matches('B');
    let (n, m) = if let Some(x) = tok.strip_suffix('G') {
        (x, 1u64 << 30)
    } else if let Some(x) = tok.strip_suffix('M') {
        (x, 1u64 << 20)
    } else if let Some(x) = tok.strip_suffix('K') {
        (x, 1u64 << 10)
    } else {
        (tok, 1)
    };
    n.parse::<f64>().ok().map(|v| (v * m as f64) as u64).unwrap_or(0)
}
