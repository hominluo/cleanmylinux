//! CleanMyLinux — GNOME frontend entry point.

mod app;
mod factory;
mod gauge;

use relm4::RelmApp;
use std::path::Path;

const APP_ID: &str = "io.cleanmylinux.CleanMyLinux";

/// Minimal bundled styling. The installed package also ships a richer
/// `style.css`, but embedding a baseline keeps run-from-source self-contained.
const INLINE_CSS: &str = "
.pill { padding: 8px 24px; border-radius: 999px; font-weight: bold; }
list.boxed-list row { padding: 4px; }
";

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    let app = RelmApp::new(APP_ID);
    load_css();
    app.run::<app::App>(());
}

fn load_css() {
    install_css(INLINE_CSS);

    let dev_css = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/gnome/style.css");
    if let Ok(css) = std::fs::read_to_string(&dev_css) {
        install_css(&css);
    }
    if let Ok(css) = std::fs::read_to_string("/usr/share/cleanmylinux/style.css") {
        install_css(&css);
    }
}

fn install_css(css: &str) {
    let provider = gtk::CssProvider::new();
    provider.load_from_data(css);
    if let Some(display) = gtk::gdk::Display::default() {
        gtk::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
}
