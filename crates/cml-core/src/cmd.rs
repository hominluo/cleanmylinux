//! Thin wrapper for running system commands with a sane, predictable
//! environment (the user's shell may have conda/etc. ahead on `PATH`, which we
//! don't want when invoking system tools like `apt`, `dpkg`, `snap`).

use std::process::Command;

/// A minimal, trusted PATH for system utilities.
const SYS_PATH: &str = "/usr/sbin:/usr/bin:/sbin:/bin";

/// Run `program args…`, returning stdout as a String on success.
pub fn run(program: &str, args: &[&str]) -> anyhow::Result<String> {
    let output = Command::new(program)
        .args(args)
        .env("PATH", SYS_PATH)
        .env("LC_ALL", "C") // stable, parseable output
        .output()?;
    if !output.status.success() {
        anyhow::bail!(
            "{program} exited with {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Run a command but tolerate non-zero exit, returning stdout regardless. Useful
/// for tools that return non-zero when there's simply "nothing to do".
pub fn run_lenient(program: &str, args: &[&str]) -> String {
    Command::new(program)
        .args(args)
        .env("PATH", SYS_PATH)
        .env("LC_ALL", "C")
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
        .unwrap_or_default()
}

/// Run a command and return only whether it exited successfully.
pub fn status_success(program: &str, args: &[&str]) -> bool {
    Command::new(program)
        .args(args)
        .env("PATH", SYS_PATH)
        .env("LC_ALL", "C")
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Whether a program is resolvable on the system PATH.
pub fn has(program: &str) -> bool {
    Command::new("sh")
        .arg("-c")
        .arg(format!("command -v {program}"))
        .env("PATH", SYS_PATH)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}
