//! Scanners for each cleanup module. Every scanner is user-space and read-only;
//! actual deletion happens in [`crate::clean`] (and, for root ops, the helper).

pub mod apps;
pub mod junk;
pub mod large_files;
pub mod monitor;
pub mod packages;

/// Parse a human size string such as "1.2 MB", "512 kB", "3,4 GB" (flatpak
/// localizes the decimal separator) into bytes. Best-effort.
pub fn packages_size_from_human(s: &str) -> u64 {
    let s = s.trim().replace(',', ".");
    let mut parts = s.split_whitespace();
    let Some(num) = parts.next().and_then(|n| n.parse::<f64>().ok()) else {
        return 0;
    };
    let mult = match parts.next().map(|u| u.to_lowercase()) {
        Some(ref u) if u.starts_with("kb") || u.starts_with("kib") => 1024.0,
        Some(ref u) if u.starts_with("mb") || u.starts_with("mib") => 1024.0 * 1024.0,
        Some(ref u) if u.starts_with("gb") || u.starts_with("gib") => 1024.0 * 1024.0 * 1024.0,
        Some(ref u) if u.starts_with('b') => 1.0,
        _ => 1.0,
    };
    (num * mult) as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_human_sizes() {
        assert_eq!(packages_size_from_human("1.0 MB"), 1024 * 1024);
        assert_eq!(packages_size_from_human("512 kB"), 512 * 1024);
        assert_eq!(packages_size_from_human(""), 0);
    }
}
