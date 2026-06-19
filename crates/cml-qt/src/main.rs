//! CleanMyLinux — KDE/Kirigami frontend entry point (cxx-qt + Qt6/QML).

mod bridge;

use cxx_qt_lib::{QGuiApplication, QQmlApplicationEngine, QUrl};

fn main() {
    let mut app = QGuiApplication::new();
    let mut engine = QQmlApplicationEngine::new();

    if let Some(engine) = engine.as_mut() {
        engine.load(&QUrl::from(
            "qrc:/qt/qml/io/cleanmylinux/qml/Main.qml",
        ));
    }

    if let Some(app) = app.as_mut() {
        app.exec();
    }
}

/// Format a system-monitor sample as a multiline string for the KDE UI.
/// (Shared shape with the GTK frontend's monitor view.)
pub fn format_monitor(s: &cml_core::scan::monitor::SystemSample) -> String {
    use humansize::{format_size, DECIMAL};
    let mut out = String::new();
    out.push_str(&format!("CPU:    {:.1}%\n", s.cpu * 100.0));
    out.push_str(&format!(
        "Memory: {} / {}  ({:.0}%)\n",
        format_size(s.mem_used, DECIMAL),
        format_size(s.mem_total, DECIMAL),
        s.mem_fraction() * 100.0
    ));
    out.push_str(&format!(
        "Swap:   {} / {}\n\nDisks:\n",
        format_size(s.swap_used, DECIMAL),
        format_size(s.swap_total, DECIMAL),
    ));
    for d in &s.disks {
        out.push_str(&format!(
            "  {}  {} free of {}  ({:.0}% used)\n",
            d.mount,
            format_size(d.available, DECIMAL),
            format_size(d.total, DECIMAL),
            d.used_fraction() * 100.0
        ));
    }
    out
}
