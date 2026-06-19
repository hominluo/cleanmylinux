//! A factory component rendering one selectable `ScanItem` as an AdwActionRow
//! with a check button, size, and optional note.

use adw::prelude::*;
use cml_core::types::{Safety, ScanItem};
use relm4::factory::{DynamicIndex, FactoryComponent, FactorySender};
use relm4::prelude::*;

pub struct ItemRow {
    pub label: String,
    pub size_text: String,
    pub note: Option<String>,
    pub safety: Safety,
    pub selected: bool,
}

#[derive(Debug)]
pub enum ItemRowOutput {
    /// (row index, now-selected)
    Toggled(DynamicIndex, bool),
}

#[relm4::factory(pub)]
impl FactoryComponent for ItemRow {
    type Init = ScanItem;
    type Input = ();
    type Output = ItemRowOutput;
    type CommandOutput = ();
    type ParentWidget = gtk::ListBox;

    view! {
        adw::ActionRow {
            set_title: &self.label,
            set_subtitle: &subtitle(&self.size_text, &self.note),
            add_prefix = &gtk::CheckButton {
                set_active: self.selected,
                set_valign: gtk::Align::Center,
                connect_toggled[sender, index] => move |btn| {
                    sender.output(ItemRowOutput::Toggled(index.clone(), btn.is_active())).ok();
                },
            },
            add_suffix = &gtk::Label {
                set_label: &self.size_text,
                add_css_class: "dim-label",
                add_css_class: "numeric",
            },
            add_suffix = &gtk::Image {
                set_icon_name: Some(safety_icon(self.safety)),
                set_tooltip_text: Some(safety_tooltip(self.safety)),
            },
        }
    }

    fn init_model(item: Self::Init, _index: &DynamicIndex, _sender: FactorySender<Self>) -> Self {
        Self {
            label: item.label,
            size_text: humansize::format_size(item.size, humansize::DECIMAL),
            note: item.note,
            safety: item.safety,
            selected: item.selected,
        }
    }

}

fn subtitle(size: &str, note: &Option<String>) -> String {
    match note {
        Some(n) => format!("{size} · {n}"),
        None => size.to_string(),
    }
}

fn safety_icon(s: Safety) -> &'static str {
    match s {
        Safety::Safe => "emblem-ok-symbolic",
        Safety::Review => "dialog-information-symbolic",
        Safety::Risky => "dialog-warning-symbolic",
    }
}

fn safety_tooltip(s: Safety) -> &'static str {
    match s {
        Safety::Safe => "Safe to remove",
        Safety::Review => "Review before removing",
        Safety::Risky => "Risky — your data; sent to Trash",
    }
}
