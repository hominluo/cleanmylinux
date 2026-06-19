//! cxx-qt build glue: compile the Rust QObject bridge and register the QML
//! module so `import io.cleanmylinux 1.0` resolves to our `Controller`.

use cxx_qt_build::{CxxQtBuilder, QmlModule};

fn main() {
    CxxQtBuilder::new()
        .qt_module("Quick")
        .qml_module(QmlModule {
            uri: "io.cleanmylinux",
            rust_files: &["src/bridge.rs"],
            qml_files: &[
                "qml/Main.qml",
                "qml/SmartScanPage.qml",
                "qml/MonitorPage.qml",
                "qml/Gauge.qml",
            ],
            ..Default::default()
        })
        .build();
}
