//! Dev smoke-test: run a Smart Scan and print what would be reclaimed.
//! Usage: `cargo run -p cml-core --example scan`

use cml_core::progress::CancelToken;
use cml_core::Engine;

fn main() {
    let engine = Engine::new();
    let cancel = CancelToken::new();

    println!("== Smart Scan (read-only) ==");
    let res = engine.smart_scan(&cancel, None);
    let mut by_cat: std::collections::BTreeMap<String, u64> = Default::default();
    for item in &res.items {
        *by_cat.entry(item.category.clone()).or_default() += item.size;
        println!(
            "  [{:>6}] {:<40} {}",
            format!("{:?}", item.safety),
            truncate(&item.label, 40),
            item.human_size()
        );
    }
    println!("\n-- by category --");
    for (cat, size) in &by_cat {
        println!("  {cat:<20} {}", humansize::format_size(*size, humansize::DECIMAL));
    }
    println!("\nTOTAL reclaimable: {}", res.human_total());
    println!("Pre-selected:      {}", humansize::format_size(res.selected_size(), humansize::DECIMAL));
}

fn truncate(s: &str, n: usize) -> String {
    if s.len() <= n {
        s.to_string()
    } else {
        format!("{}…", &s[..n - 1])
    }
}
