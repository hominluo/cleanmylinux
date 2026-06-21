//! The main CleanMyLinux GNOME window: an Adwaita navigation split view with a
//! module sidebar and a content area that hosts the Smart Scan dashboard, the
//! per-module scan/clean views, and the live System Monitor.

use adw::prelude::*;
use cml_core::clean;
use cml_core::progress::CancelToken;
use cml_core::scan::monitor::Monitor;
use cml_core::types::{DeleteMode, Module, Safety, ScanItem, ScanResult};
use cml_core::Engine;
use relm4::factory::{DynamicIndex, FactoryVecDeque};
use relm4::prelude::*;

use crate::factory::{ItemRow, ItemRowOutput};
use crate::gauge::Gauge;

/// Which view the content area is showing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    SmartScan,
    Module(Module),
}

pub struct App {
    view: View,
    busy: bool,
    title: String,
    subtitle: String,
    gauge: Gauge,
    rows: FactoryVecDeque<ItemRow>,
    /// The backing scan result (source of truth for selection + cleaning).
    result: ScanResult,
    status: String,
    monitor: Monitor,
    monitor_text: String,
    cancel: CancelToken,
}

#[derive(Debug)]
pub enum Msg {
    Select(View),
    Scan,
    RowToggled(DynamicIndex, bool),
    SelectAll(bool),
    ConfirmClean,
    Clean,
    Cancel,
    Tick,
}

#[derive(Debug)]
pub enum CmdOut {
    ScanDone(ScanResult),
    CleanDone(cml_core::types::CleanReport, String),
}

#[relm4::component(pub)]
impl Component for App {
    type Init = ();
    type Input = Msg;
    type Output = ();
    type CommandOutput = CmdOut;

    view! {
        adw::ApplicationWindow {
            set_title: Some("CleanMyLinux"),
            set_default_width: 980,
            set_default_height: 680,

            #[name = "toaster"]
            adw::ToastOverlay {
                adw::NavigationSplitView {
                    set_max_sidebar_width: 260.0,

                    #[wrap(Some)]
                    set_sidebar = &adw::NavigationPage {
                        set_title: "CleanMyLinux",
                        #[wrap(Some)]
                        set_child = &adw::ToolbarView {
                            add_top_bar = &adw::HeaderBar {
                                #[wrap(Some)]
                                set_title_widget = &adw::WindowTitle {
                                    set_title: "CleanMyLinux",
                                    set_subtitle: "Keep your system clean",
                                },
                            },
                            #[wrap(Some)]
                            set_content = &gtk::ScrolledWindow {
                                set_hscrollbar_policy: gtk::PolicyType::Never,
                                gtk::ListBox {
                                    add_css_class: "navigation-sidebar",
                                    set_selection_mode: gtk::SelectionMode::Single,

                                    // Smart Scan entry (the flagship).
                                    append = &row_button("Smart Scan", "edit-find-symbolic"),
                                    append = &row_button(Module::SystemJunk.title(), Module::SystemJunk.icon_name()),
                                    append = &row_button(Module::PackageCleanup.title(), Module::PackageCleanup.icon_name()),
                                    append = &row_button(Module::LargeAndOldFiles.title(), Module::LargeAndOldFiles.icon_name()),
                                    append = &row_button(Module::SystemMonitor.title(), Module::SystemMonitor.icon_name()),

                                    connect_row_activated[sender] => move |_, row| {
                                        let idx = row.index();
                                        let view = match idx {
                                            0 => View::SmartScan,
                                            1 => View::Module(Module::SystemJunk),
                                            2 => View::Module(Module::PackageCleanup),
                                            3 => View::Module(Module::LargeAndOldFiles),
                                            _ => View::Module(Module::SystemMonitor),
                                        };
                                        sender.input(Msg::Select(view));
                                    },
                                },
                            },
                        },
                    },

                    #[wrap(Some)]
                    set_content = &adw::NavigationPage {
                        #[watch]
                        set_title: &model.title,
                        #[wrap(Some)]
                        set_child = &adw::ToolbarView {
                            add_top_bar = &adw::HeaderBar {
                                #[wrap(Some)]
                                set_title_widget = &adw::WindowTitle {
                                    #[watch]
                                    set_title: &model.title,
                                    #[watch]
                                    set_subtitle: &model.subtitle,
                                },
                            },

                            #[wrap(Some)]
                            #[name = "content_stack"]
                            set_content = &gtk::Stack {
                                // ---- Scan / clean view ----
                                add_named[Some("scan")] = &gtk::Box {
                                    set_orientation: gtk::Orientation::Vertical,
                                    set_spacing: 12,
                                    set_margin_all: 18,

                                    // Dashboard header: gauge + actions.
                                    gtk::Box {
                                        set_orientation: gtk::Orientation::Horizontal,
                                        set_spacing: 24,
                                        set_halign: gtk::Align::Center,

                                        #[local_ref]
                                        gauge_widget -> gtk::DrawingArea {},

                                        gtk::Box {
                                            set_orientation: gtk::Orientation::Vertical,
                                            set_spacing: 10,
                                            set_valign: gtk::Align::Center,

                                            gtk::Button {
                                                #[watch]
                                                set_label: if model.busy { "Scanning…" } else { "Scan" },
                                                add_css_class: "suggested-action",
                                                add_css_class: "pill",
                                                #[watch]
                                                set_sensitive: !model.busy,
                                                connect_clicked => Msg::Scan,
                                            },
                                            gtk::Button {
                                                #[watch]
                                                set_label: &format!("Clean {}", humansize::format_size(model.result.selected_size(), humansize::DECIMAL)),
                                                add_css_class: "destructive-action",
                                                add_css_class: "pill",
                                                #[watch]
                                                set_sensitive: !model.busy && model.result.selected_size() > 0,
                                                connect_clicked => Msg::ConfirmClean,
                                            },
                                            gtk::Button {
                                                set_label: "Cancel",
                                                add_css_class: "pill",
                                                #[watch]
                                                set_visible: model.busy,
                                                connect_clicked => Msg::Cancel,
                                            },
                                        },
                                    },

                                    gtk::Box {
                                        set_orientation: gtk::Orientation::Horizontal,
                                        set_spacing: 6,
                                        gtk::Button {
                                            set_label: "Select all",
                                            add_css_class: "flat",
                                            connect_clicked => Msg::SelectAll(true),
                                        },
                                        gtk::Button {
                                            set_label: "Deselect all",
                                            add_css_class: "flat",
                                            connect_clicked => Msg::SelectAll(false),
                                        },
                                        gtk::Label {
                                            set_hexpand: true,
                                            set_halign: gtk::Align::End,
                                            #[watch]
                                            set_label: &model.status,
                                            add_css_class: "dim-label",
                                        },
                                    },

                                    gtk::ScrolledWindow {
                                        set_vexpand: true,
                                        #[local_ref]
                                        rows_list -> gtk::ListBox {
                                            set_selection_mode: gtk::SelectionMode::None,
                                            add_css_class: "boxed-list",
                                            set_valign: gtk::Align::Start,
                                        },
                                    },
                                },

                                // ---- System Monitor view ----
                                add_named[Some("monitor")] = &gtk::Box {
                                    set_orientation: gtk::Orientation::Vertical,
                                    set_margin_all: 24,
                                    set_spacing: 12,
                                    gtk::Label {
                                        add_css_class: "title-2",
                                        set_halign: gtk::Align::Start,
                                        set_label: "System Monitor",
                                    },
                                    gtk::Label {
                                        set_halign: gtk::Align::Start,
                                        add_css_class: "monospace",
                                        #[watch]
                                        set_label: &model.monitor_text,
                                    },
                                },

                                // Children exist by now, so switching is safe.
                                #[watch]
                                set_visible_child_name: if model.view == View::Module(Module::SystemMonitor) { "monitor" } else { "scan" },
                            },
                        },
                    },
                },
            },
        }
    }

    fn init(
        _init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let gauge = Gauge::new();
        let rows = FactoryVecDeque::builder()
            .launch(gtk::ListBox::default())
            .forward(sender.input_sender(), |out| match out {
                ItemRowOutput::Toggled(idx, v) => Msg::RowToggled(idx, v),
            });

        let model = App {
            view: View::SmartScan,
            busy: false,
            title: "Smart Scan".into(),
            subtitle: "Reclaim space with one click".into(),
            gauge,
            rows,
            result: ScanResult::default(),
            status: String::new(),
            monitor: Monitor::new(),
            monitor_text: "Sampling…".into(),
            cancel: CancelToken::new(),
        };

        let gauge_widget = &model.gauge.widget;
        let rows_list = model.rows.widget();
        let widgets = view_output!();

        // Drive the System Monitor at ~1 Hz via a GLib timer (main-loop friendly).
        let tick_sender = sender.input_sender().clone();
        gtk::glib::timeout_add_seconds_local(1, move || {
            tick_sender.send(Msg::Tick).ok();
            gtk::glib::ControlFlow::Continue
        });

        model.gauge.set(0.0, "—", "press Scan");
        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>, root: &Self::Root) {
        match msg {
            Msg::Select(view) => {
                self.view = view;
                let (title, subtitle) = match view {
                    View::SmartScan => (
                        "Smart Scan".to_string(),
                        "Reclaim space with one click".into(),
                    ),
                    View::Module(m) => (m.title().to_string(), module_subtitle(m)),
                };
                self.title = title;
                self.subtitle = subtitle;
                // Auto-scan space modules on entry (not the monitor).
                if !matches!(view, View::Module(Module::SystemMonitor)) {
                    sender.input(Msg::Scan);
                }
            }
            Msg::Scan => {
                if self.busy {
                    return;
                }
                self.busy = true;
                self.status = "Scanning…".into();
                self.gauge.set_busy("scanning");
                self.cancel = CancelToken::new();
                let view = self.view;
                let cancel = self.cancel.clone();
                sender.oneshot_command(async move {
                    let res = relm4::tokio::task::spawn_blocking(move || {
                        let engine = Engine::new();
                        match view {
                            View::SmartScan => engine.smart_scan(&cancel, None),
                            View::Module(m) => engine.scan_module(m, &cancel, None),
                        }
                    })
                    .await
                    .unwrap_or_default();
                    CmdOut::ScanDone(res)
                });
            }
            Msg::RowToggled(idx, v) => {
                let i = idx.current_index();
                if let Some(item) = self.result.items.get_mut(i) {
                    item.selected = v;
                }
                self.refresh_gauge();
            }
            Msg::SelectAll(v) => {
                for item in &mut self.result.items {
                    item.selected = v;
                }
                // Rebuild rows to reflect new selection state.
                self.populate_rows();
                self.refresh_gauge();
            }
            Msg::Cancel => {
                self.cancel.cancel();
                self.status = "Cancelling…".into();
            }
            Msg::ConfirmClean => {
                if self.busy || self.result.selected_size() == 0 {
                    return;
                }
                present_clean_dialog(root, &self.result.items, sender.input_sender().clone());
            }
            Msg::Clean => {
                if self.busy {
                    return;
                }
                self.busy = true;
                self.status = "Cleaning…".into();
                let items = self.result.items.clone();
                self.cancel = CancelToken::new();
                let cancel = self.cancel.clone();
                sender.oneshot_command(async move {
                    let (report, extra) =
                        relm4::tokio::task::spawn_blocking(move || run_clean(items, &cancel))
                            .await
                            .unwrap_or_default();
                    CmdOut::CleanDone(report, extra)
                });
            }
            Msg::Tick => {
                if self.view == View::Module(Module::SystemMonitor) {
                    let s = self.monitor.sample();
                    self.monitor_text = format_monitor(&s);
                }
            }
        }
    }

    fn update_cmd(
        &mut self,
        msg: Self::CommandOutput,
        _sender: ComponentSender<Self>,
        _root: &Self::Root,
    ) {
        match msg {
            CmdOut::ScanDone(res) => {
                self.busy = false;
                self.result = res;
                self.populate_rows();
                self.refresh_gauge();
                self.status = format!("{} found", self.result.human_total());
            }
            CmdOut::CleanDone(report, extra) => {
                self.busy = false;
                let mut msg = format!("Freed {}", report.human_freed());
                if !extra.is_empty() {
                    msg.push_str(&format!(" · {extra}"));
                }
                if !report.failures.is_empty() {
                    msg.push_str(&format!(" · {} skipped", report.failures.len()));
                }
                self.status = msg;
                // Re-scan to reflect the new state.
                _sender.input(Msg::Scan);
            }
        }
    }
}

impl App {
    fn populate_rows(&mut self) {
        let mut guard = self.rows.guard();
        guard.clear();
        for item in &self.result.items {
            guard.push_back(item.clone());
        }
    }

    fn refresh_gauge(&self) {
        let total = self.result.total_size();
        let selected = self.result.selected_size();
        let frac = if total == 0 {
            0.0
        } else {
            selected as f64 / total as f64
        };
        let value = humansize::format_size(selected, humansize::DECIMAL);
        self.gauge.set(frac, value, "selected to clean");
    }
}

/// Run user-space + privileged cleanup via the shared engine helper.
fn run_clean(
    items: Vec<cml_core::types::ScanItem>,
    cancel: &CancelToken,
) -> (cml_core::types::CleanReport, String) {
    let home = dirs::home_dir().unwrap_or_default();
    let cfg = cml_core::config::Config::load();
    let safelist = cml_core::safety::Safelist::new(home, cfg.exclusions);
    clean::clean_selected(&items, &safelist, cancel, None)
}

fn module_subtitle(m: Module) -> String {
    match m {
        Module::SystemJunk => "Caches, thumbnails, Trash and logs".into(),
        Module::PackageCleanup => "APT cache, orphans, old kernels, journal".into(),
        Module::LargeAndOldFiles => "Big files in your home folder (sent to Trash)".into(),
        Module::Uninstaller => "Remove installed applications".into(),
        Module::SystemMonitor => "Live CPU, memory and disk usage".into(),
    }
}

fn format_monitor(s: &cml_core::scan::monitor::SystemSample) -> String {
    let mut out = String::new();
    out.push_str(&format!("CPU:    {:>5.1}%\n", s.cpu * 100.0));
    out.push_str(&format!(
        "Memory: {} / {}  ({:.0}%)\n",
        humansize::format_size(s.mem_used, humansize::DECIMAL),
        humansize::format_size(s.mem_total, humansize::DECIMAL),
        s.mem_fraction() * 100.0
    ));
    out.push_str(&format!(
        "Swap:   {} / {}\n\n",
        humansize::format_size(s.swap_used, humansize::DECIMAL),
        humansize::format_size(s.swap_total, humansize::DECIMAL),
    ));
    out.push_str("Disks:\n");
    for d in &s.disks {
        out.push_str(&format!(
            "  {:<16} {} free of {}  ({:.0}% used)\n",
            d.mount,
            humansize::format_size(d.available, humansize::DECIMAL),
            humansize::format_size(d.total, humansize::DECIMAL),
            d.used_fraction() * 100.0
        ));
    }
    out
}

fn present_clean_dialog(
    root: &adw::ApplicationWindow,
    items: &[ScanItem],
    input: relm4::Sender<Msg>,
) {
    let selected: Vec<&ScanItem> = items.iter().filter(|i| i.selected).collect();
    if selected.is_empty() {
        return;
    }

    let selected_bytes: u64 = selected.iter().map(|i| i.size).sum();
    let permanent = selected
        .iter()
        .filter(|i| i.delete_mode == DeleteMode::Permanent)
        .count();
    let trash = selected
        .iter()
        .filter(|i| i.delete_mode == DeleteMode::Trash)
        .count();
    let privileged = selected
        .iter()
        .filter(|i| i.path.to_string_lossy().starts_with("priv://"))
        .count();
    let risky = selected
        .iter()
        .filter(|i| i.safety == Safety::Risky)
        .count();

    let mut lines = vec![
        format!(
            "{} selected across {} item(s).",
            humansize::format_size(selected_bytes, humansize::DECIMAL),
            selected.len()
        ),
        format!("{permanent} item(s) will be deleted permanently."),
    ];
    if trash > 0 {
        lines.push(format!("{trash} user file(s) will be moved to Trash."));
    }
    if privileged > 0 {
        lines.push(format!(
            "{privileged} privileged action(s) may ask for your password."
        ));
    }
    if risky > 0 {
        lines.push(format!("{risky} risky item(s) are selected."));
    }

    let dialog = adw::AlertDialog::builder()
        .heading("Clean selected items?")
        .body(lines.join("\n"))
        .build();
    dialog.add_response("cancel", "Cancel");
    dialog.add_response("clean", "Clean");
    dialog.set_default_response(Some("cancel"));
    dialog.set_close_response("cancel");
    dialog.set_response_appearance("clean", adw::ResponseAppearance::Destructive);
    dialog.choose(root, None::<&gtk::gio::Cancellable>, move |response| {
        if response == "clean" {
            input.send(Msg::Clean).ok();
        }
    });
}

/// Build a sidebar row (icon + label) as a plain box; the parent ListBox wraps
/// it in a selectable row.
fn row_button(label: &str, icon: &str) -> gtk::Box {
    let b = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    b.set_margin_top(8);
    b.set_margin_bottom(8);
    b.set_margin_start(6);
    b.set_margin_end(6);
    let img = gtk::Image::from_icon_name(icon);
    let lbl = gtk::Label::new(Some(label));
    lbl.set_halign(gtk::Align::Start);
    b.append(&img);
    b.append(&lbl);
    b
}
