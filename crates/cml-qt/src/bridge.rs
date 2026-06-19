//! cxx-qt bridge: exposes the shared `cml-core` engine to QML as a `Controller`
//! QObject. Scans/cleans run on a worker thread and marshal results back to the
//! Qt thread via cxx-qt's threading support, so the Kirigami UI stays responsive.

use core::pin::Pin;
use std::sync::{Arc, Mutex};

use cxx_qt::Threading;

use cml_core::progress::CancelToken;
use cml_core::scan::monitor::Monitor;
use cml_core::types::{Module, ScanItem, ScanResult};
use cml_core::Engine;

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
    }

    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qproperty(bool, busy)]
        #[qproperty(f64, gauge_value)]
        #[qproperty(QString, summary)]
        #[qproperty(QString, detail)]
        #[qproperty(QString, status)]
        type Controller = super::ControllerRust;

        /// Scan a module (0 = Smart Scan, 1..=5 map to `Module`).
        #[qinvokable]
        fn scan(self: Pin<&mut Controller>, module: i32);

        /// Clean the pre-selected (safe) items from the last scan.
        #[qinvokable]
        fn clean_safe(self: Pin<&mut Controller>);

        /// Sample the live system monitor; returns a formatted multiline string.
        #[qinvokable]
        fn sample_monitor(self: Pin<&mut Controller>) -> QString;
    }

    impl cxx_qt::Threading for Controller {}
}

use cxx_qt_lib::QString;

pub struct ControllerRust {
    busy: bool,
    gauge_value: f64,
    summary: QString,
    detail: QString,
    status: QString,
    /// Backing scan result, shared with worker threads.
    last: Arc<Mutex<Vec<ScanItem>>>,
    monitor: Arc<Mutex<Monitor>>,
}

impl Default for ControllerRust {
    fn default() -> Self {
        Self {
            busy: false,
            gauge_value: 0.0,
            summary: QString::from("Press Scan"),
            detail: QString::from(""),
            status: QString::from(""),
            last: Arc::new(Mutex::new(Vec::new())),
            monitor: Arc::new(Mutex::new(Monitor::new())),
        }
    }
}

fn module_from_i32(m: i32) -> Option<Module> {
    match m {
        1 => Some(Module::SystemJunk),
        2 => Some(Module::PackageCleanup),
        3 => Some(Module::LargeAndOldFiles),
        4 => Some(Module::Uninstaller),
        5 => Some(Module::SystemMonitor),
        _ => None, // 0 → Smart Scan
    }
}

fn format_detail(res: &ScanResult) -> String {
    res.items
        .iter()
        .map(|i| {
            let mark = if i.selected { "☑" } else { "☐" };
            format!(
                "{mark}  {}  —  {}",
                i.label,
                humansize::format_size(i.size, humansize::DECIMAL)
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

impl qobject::Controller {
    /// Start a background scan and stream the result back to the Qt thread.
    pub fn scan(self: Pin<&mut Self>, module: i32) {
        if *self.busy() {
            return;
        }
        let mut this = self;
        this.as_mut().set_busy(true);
        this.as_mut().set_summary(QString::from("Scanning…"));
        this.as_mut().set_status(QString::from("Scanning…"));

        let last = this.last.clone();
        let qt_thread = this.qt_thread();

        std::thread::spawn(move || {
            let engine = Engine::new();
            let cancel = CancelToken::new();
            let res = match module_from_i32(module) {
                Some(m) => engine.scan_module(m, &cancel, None),
                None => engine.smart_scan(&cancel, None),
            };

            if let Ok(mut guard) = last.lock() {
                *guard = res.items.clone();
            }

            let total = res.total_size();
            let selected = res.selected_size();
            let frac = if total == 0 {
                0.0
            } else {
                selected as f64 / total as f64
            };
            let summary = humansize::format_size(total, humansize::DECIMAL);
            let detail = format_detail(&res);

            qt_thread
                .queue(move |mut ctrl| {
                    ctrl.as_mut().set_busy(false);
                    ctrl.as_mut().set_gauge_value(frac);
                    ctrl.as_mut()
                        .set_summary(QString::from(&format!("{summary} reclaimable")));
                    ctrl.as_mut().set_detail(QString::from(&detail));
                    ctrl.as_mut().set_status(QString::from(&format!(
                        "{} items found",
                        detail.lines().count()
                    )));
                })
                .ok();
        });
    }

    /// Clean the pre-selected items from the last scan, on a worker thread.
    pub fn clean_safe(self: Pin<&mut Self>) {
        if *self.busy() {
            return;
        }
        let mut this = self;
        this.as_mut().set_busy(true);
        this.as_mut().set_status(QString::from("Cleaning…"));

        let last = this.last.clone();
        let qt_thread = this.qt_thread();

        std::thread::spawn(move || {
            let items = last.lock().map(|g| g.clone()).unwrap_or_default();
            let home = dirs::home_dir().unwrap_or_default();
            let cfg = cml_core::config::Config::load();
            let safelist = cml_core::safety::Safelist::new(home, cfg.exclusions);
            let cancel = CancelToken::new();
            let (report, extra) =
                cml_core::clean::clean_selected(&items, &safelist, &cancel, None);

            let mut msg = format!("Freed {}", report.human_freed());
            if !extra.is_empty() {
                msg.push_str(&format!(" · {extra}"));
            }
            if !report.failures.is_empty() {
                msg.push_str(&format!(" · {} skipped", report.failures.len()));
            }

            qt_thread
                .queue(move |mut ctrl| {
                    ctrl.as_mut().set_busy(false);
                    ctrl.as_mut().set_status(QString::from(&msg));
                })
                .ok();
        });
    }

    /// Sample the system monitor synchronously (cheap; called on a QML Timer).
    pub fn sample_monitor(self: Pin<&mut Self>) -> QString {
        let text = self
            .monitor
            .lock()
            .map(|mut m| crate::format_monitor(&m.sample()))
            .unwrap_or_default();
        QString::from(&text)
    }
}
