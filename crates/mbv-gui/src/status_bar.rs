//! The GUI Client's Status bar (row 6.5): the egui `Painter` over the Status
//! bar's Surface state. The bar paints the prefix-armed pill and the live
//! toast, every colour resolved through the theme's neutral `Rgb`.
//!
//! The bar holds no interaction state. The armed flag and the toast
//! presentation live in `StatusBarSurface`, which the App's projection adopts
//! each frame, and no stateful egui widget exists here (design D8). The bar
//! reports no pointer regions yet: the volume, mute, and visual-mode controls
//! are not presented in this slice.

use eframe::egui;
use mbv::StatusBarProjection;
use mbv_surfaces::status_bar::StatusBarSurface;
use mbv_theme::{Role, Surface};
use mbv_ui_model::toast::{StatusToast, ToastSeverity};

use crate::queue::{PILL_PAD, ROW_HEIGHT, role_colour, surface_fill};

/// The Status bar's height: one fixed row (`specs/gui-client`: fixed row
/// heights), as the TUI's status row is one row.
pub(crate) const STATUS_BAR_HEIGHT: f32 = ROW_HEIGHT;

/// The Status bar: its Surface state and the painter over it.
pub(crate) struct StatusBarPanel {
    surface: StatusBarSurface,
}

impl Default for StatusBarPanel {
    fn default() -> Self {
        Self::new()
    }
}

impl StatusBarPanel {
    #[must_use]
    pub(crate) fn new() -> Self {
        Self {
            surface: StatusBarSurface::new(),
        }
    }

    /// Adopt one projection push from the `App` (design D5).
    pub(crate) fn sync(&mut self, projection: StatusBarProjection) {
        self.surface.set_prefix_armed(projection.prefix_armed);
        self.surface.set_toast(projection.toast);
    }

    /// Paint one frame over `rect`. A live toast takes the whole row, and the
    /// armed pill drops while it shows, as the TUI status row does.
    pub(crate) fn paint(&self, painter: &egui::Painter, rect: egui::Rect) {
        painter.rect_filled(rect, 0.0, surface_fill(Surface::StatusBar, false));
        if let Some(toast) = self.surface.toast() {
            paint_toast(painter, rect, toast);
        } else if self.surface.prefix_armed() {
            paint_prefix_pill(painter, rect);
        }
    }

    /// The projection the Surface state holds after the last sync (test
    /// evidence).
    #[cfg(test)]
    pub(crate) fn test_projection(&self) -> StatusBarProjection {
        StatusBarProjection {
            prefix_armed: self.surface.prefix_armed(),
            toast: self.surface.toast().cloned(),
        }
    }
}

/// The live toast, centred over the whole row in its severity's role. The
/// text elides to the row's width and clips to it.
fn paint_toast(painter: &egui::Painter, rect: egui::Rect, toast: &StatusToast) {
    let colour = role_colour(severity_role(toast.severity));
    let mut job = egui::text::LayoutJob::simple_singleline(
        toast.text.clone(),
        egui::FontId::proportional(13.0),
        colour,
    );
    job.wrap = egui::text::TextWrapping::truncate_at_width(rect.width());
    let galley = painter.layout_job(job);
    let origin = egui::Align2::CENTER_CENTER
        .anchor_size(rect.center(), galley.size())
        .min;
    painter.with_clip_rect(rect).galley(origin, galley, colour);
}

/// The prefix-armed pill at the bar's left: the word in the accent role on the
/// status pill surface.
fn paint_prefix_pill(painter: &egui::Painter, rect: egui::Rect) {
    let font = egui::FontId::proportional(13.0);
    let colour = role_colour(Role::Accent);
    let text_width = painter
        .layout_no_wrap("PREFIX".to_owned(), font.clone(), colour)
        .size()
        .x;
    let pill = egui::Rect::from_min_size(
        rect.left_top(),
        egui::vec2(text_width + 2.0 * PILL_PAD, rect.height()),
    );
    painter.rect_filled(pill, 0.0, surface_fill(Surface::StatusBarPill, false));
    painter.text(
        pill.center(),
        egui::Align2::CENTER_CENTER,
        "PREFIX",
        font,
        colour,
    );
}

/// The role a toast severity paints in: the TUI's severity colour map.
fn severity_role(severity: ToastSeverity) -> Role {
    match severity {
        ToastSeverity::Neutral => Role::TextPrimary,
        ToastSeverity::Success => Role::StatusAvailable,
        ToastSeverity::Warning => Role::TextFocusAccent,
        ToastSeverity::Error => Role::StatusError,
    }
}

#[cfg(test)]
mod tests;
