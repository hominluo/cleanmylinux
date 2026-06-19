//! A CleanMyMac-style circular gauge drawn with Cairo on a `gtk::DrawingArea`.
//!
//! The gauge shows a progress arc plus a big centered value and caption. State
//! is held in a shared cell so the owning component can update it and call
//! [`Gauge::redraw`].

use gtk::cairo::Context;
use gtk::prelude::*;
use std::cell::RefCell;
use std::f64::consts::PI;
use std::rc::Rc;

#[derive(Clone, Default)]
struct GaugeState {
    /// 0.0..=1.0 fill fraction of the arc.
    fraction: f64,
    /// Big centered text, e.g. "20.8 GB".
    value: String,
    /// Caption under the value, e.g. "reclaimable".
    caption: String,
    /// Whether to render in "scanning" (indeterminate-ish) style.
    busy: bool,
}

/// Owns a `DrawingArea` and the shared state powering its draw function.
pub struct Gauge {
    pub widget: gtk::DrawingArea,
    state: Rc<RefCell<GaugeState>>,
}

impl Gauge {
    pub fn new() -> Self {
        let state = Rc::new(RefCell::new(GaugeState {
            caption: "ready".into(),
            value: "—".into(),
            ..Default::default()
        }));
        let area = gtk::DrawingArea::builder()
            .content_width(240)
            .content_height(240)
            .hexpand(false)
            .vexpand(false)
            .build();

        let draw_state = state.clone();
        area.set_draw_func(move |area, cr, w, h| {
            draw(&draw_state.borrow(), area, cr, w, h);
        });

        Self {
            widget: area,
            state,
        }
    }

    pub fn set(&self, fraction: f64, value: impl Into<String>, caption: impl Into<String>) {
        let mut s = self.state.borrow_mut();
        s.fraction = fraction.clamp(0.0, 1.0);
        s.value = value.into();
        s.caption = caption.into();
        s.busy = false;
        drop(s);
        self.widget.queue_draw();
    }

    pub fn set_busy(&self, caption: impl Into<String>) {
        let mut s = self.state.borrow_mut();
        s.busy = true;
        s.value = "…".into();
        s.caption = caption.into();
        drop(s);
        self.widget.queue_draw();
    }

}

impl Default for Gauge {
    fn default() -> Self {
        Self::new()
    }
}

fn draw(s: &GaugeState, area: &gtk::DrawingArea, cr: &Context, w: i32, h: i32) {
    let w = w as f64;
    let h = h as f64;
    let cx = w / 2.0;
    let cy = h / 2.0;
    let radius = (w.min(h) / 2.0) - 16.0;
    let line_w = 16.0_f64;

    // Resolve accent + dim colors from the widget's style context so we follow
    // the system theme (light/dark + accent).
    let accent = area
        .style_context()
        .lookup_color("accent_color")
        .unwrap_or_else(|| gtk::gdk::RGBA::new(0.20, 0.52, 0.89, 1.0));
    let track = area
        .style_context()
        .lookup_color("card_shade_color")
        .unwrap_or_else(|| gtk::gdk::RGBA::new(0.5, 0.5, 0.5, 0.2));

    // Track (full ring).
    cr.set_line_width(line_w);
    cr.set_source_rgba(track.red() as f64, track.green() as f64, track.blue() as f64, track.alpha() as f64);
    cr.arc(cx, cy, radius, 0.0, 2.0 * PI);
    let _ = cr.stroke();

    // Progress arc, starting at top (−90°).
    let start = -PI / 2.0;
    let frac = if s.busy { 0.25 } else { s.fraction };
    let end = start + frac * 2.0 * PI;
    cr.set_line_width(line_w);
    cr.set_line_cap(gtk::cairo::LineCap::Round);
    cr.set_source_rgba(accent.red() as f64, accent.green() as f64, accent.blue() as f64, accent.alpha() as f64);
    cr.arc(cx, cy, radius, start, end);
    let _ = cr.stroke();

    // Centered value text.
    let fg = area
        .style_context()
        .lookup_color("window_fg_color")
        .unwrap_or_else(|| gtk::gdk::RGBA::new(0.1, 0.1, 0.1, 1.0));
    cr.set_source_rgba(fg.red() as f64, fg.green() as f64, fg.blue() as f64, fg.alpha() as f64);

    cr.select_font_face("Cantarell", gtk::cairo::FontSlant::Normal, gtk::cairo::FontWeight::Bold);
    cr.set_font_size(34.0);
    if let Ok(ext) = cr.text_extents(&s.value) {
        cr.move_to(cx - ext.width() / 2.0 - ext.x_bearing(), cy);
        let _ = cr.show_text(&s.value);
    }

    cr.set_font_size(13.0);
    cr.select_font_face("Cantarell", gtk::cairo::FontSlant::Normal, gtk::cairo::FontWeight::Normal);
    if let Ok(ext) = cr.text_extents(&s.caption) {
        cr.move_to(cx - ext.width() / 2.0 - ext.x_bearing(), cy + 24.0);
        let _ = cr.show_text(&s.caption);
    }
}
