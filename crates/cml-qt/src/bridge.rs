//! cxx-qt bridge: exposes the shared `cml-core` engine to QML as a `Controller`
//! QObject. Scans/cleans run on a worker thread and marshal results back to the
//! Qt thread via cxx-qt's threading support, so the Kirigami UI stays responsive.

#![allow(clippy::incompatible_msrv)]

use core::pin::Pin;
use std::sync::{Arc, Mutex};

use cxx_qt::Threading;

use cml_core::progress::CancelToken;
use cml_core::scan::monitor::Monitor;
use cml_core::types::{DeleteMode, Module, Safety, ScanItem};
use cml_core::Engine;
use serde::Serialize;

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
        #[qproperty(QString, rows_json)]
        #[qproperty(QString, status)]
        #[qproperty(bool, has_selection)]
        type Controller = super::ControllerRust;

        /// Scan a module (0 = Smart Scan, 1..=5 map to `Module`).
        #[qinvokable]
        fn scan(self: Pin<&mut Controller>, module: i32);

        /// Clean the pre-selected (safe) items from the last scan.
        #[qinvokable]
        fn clean_safe(self: Pin<&mut Controller>);

        /// Update row selection from QML.
        #[qinvokable]
        fn set_selected(self: Pin<&mut Controller>, index: i32, selected: bool);

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
    rows_json: QString,
    status: QString,
    has_selection: bool,
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
            rows_json: QString::from("[]"),
            status: QString::from(""),
            has_selection: false,
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

#[derive(Serialize)]
struct UiRow {
    category: String,
    label: String,
    size: u64,
    size_text: String,
    safety: &'static str,
    delete_mode: &'static str,
    selected: bool,
    privileged: bool,
    note: String,
}

fn rows_json(items: &[ScanItem]) -> String {
    let rows: Vec<UiRow> = items
        .iter()
        .map(|i| UiRow {
            category: i.category.clone(),
            label: i.label.clone(),
            size: i.size,
            size_text: humansize::format_size(i.size, humansize::DECIMAL),
            safety: match i.safety {
                Safety::Safe => "safe",
                Safety::Review => "review",
                Safety::Risky => "risky",
            },
            delete_mode: match i.delete_mode {
                DeleteMode::Permanent => "permanent",
                DeleteMode::Trash => "trash",
            },
            selected: i.selected,
            privileged: i.path.to_string_lossy().starts_with("priv://"),
            note: i.note.clone().unwrap_or_default(),
        })
        .collect();
    serde_json::to_string(&rows).unwrap_or_else(|_| "[]".into())
}

fn selected_size(items: &[ScanItem]) -> u64 {
    items.iter().filter(|i| i.selected).map(|i| i.size).sum()
}

fn total_size(items: &[ScanItem]) -> u64 {
    items.iter().map(|i| i.size).sum()
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
            let summary = humansize::format_size(selected, humansize::DECIMAL);
            let rows = rows_json(&res.items);
            let found = res.items.len();
            let has_selection = selected > 0;

            qt_thread
                .queue(move |mut ctrl| {
                    ctrl.as_mut().set_busy(false);
                    ctrl.as_mut().set_gauge_value(frac);
                    ctrl.as_mut()
                        .set_summary(QString::from(&format!("{summary} selected")));
                    ctrl.as_mut().set_rows_json(QString::from(&rows));
                    ctrl.as_mut()
                        .set_status(QString::from(&format!("{found} items found")));
                    ctrl.as_mut().set_has_selection(has_selection);
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
            let (report, extra) = cml_core::clean::clean_selected(&items, &safelist, &cancel, None);

            let mut msg = format!("Freed {}", report.human_freed());
            if !extra.is_empty() {
                msg.push_str(&format!(" · {extra}"));
            }
            if !report.failures.is_empty() {
                msg.push_str(&format!(" · {} skipped", report.failures.len()));
            }
            let rows = if let Ok(mut guard) = last.lock() {
                for item in guard.iter_mut() {
                    item.selected = false;
                }
                rows_json(&guard)
            } else {
                "[]".into()
            };

            qt_thread
                .queue(move |mut ctrl| {
                    ctrl.as_mut().set_busy(false);
                    ctrl.as_mut().set_status(QString::from(&msg));
                    ctrl.as_mut().set_gauge_value(0.0);
                    ctrl.as_mut().set_summary(QString::from("0 B selected"));
                    ctrl.as_mut().set_rows_json(QString::from(&rows));
                    ctrl.as_mut().set_has_selection(false);
                })
                .ok();
        });
    }

    /// Update row selection and recompute gauge state.
    pub fn set_selected(self: Pin<&mut Self>, index: i32, selected: bool) {
        if index < 0 {
            return;
        }
        let mut this = self;
        let Ok(mut items) = this.last.lock() else {
            return;
        };
        let Some(item) = items.get_mut(index as usize) else {
            return;
        };
        item.selected = selected;
        let total = total_size(&items);
        let selected = selected_size(&items);
        let frac = if total == 0 {
            0.0
        } else {
            selected as f64 / total as f64
        };
        let summary = humansize::format_size(selected, humansize::DECIMAL);
        let rows = rows_json(&items);
        drop(items);

        this.as_mut().set_gauge_value(frac);
        this.as_mut()
            .set_summary(QString::from(&format!("{summary} selected")));
        this.as_mut().set_rows_json(QString::from(&rows));
        this.as_mut().set_has_selection(selected > 0);
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
