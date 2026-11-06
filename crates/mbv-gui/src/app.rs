//! The GUI Client window: one eframe app over the Client core (design D7 and
//! D8 of `add-gui-front-end`).
//!
//! The frame does exactly two things beyond painting: it runs the shared tick
//! at its start, and it asks eframe to repaint at the TUI Client's idle
//! cadence so timers and worker results still land when no waker fires. The
//! waker installed on the core repaints this window from the threads that
//! deliver Owner events.

use std::time::Instant;

use eframe::egui;
use mbv::{CLIENT_IDLE_CADENCE, ClientCore};
use mbv_theme::{Surface, surface_colors};

use crate::attach;
use crate::fonts;
use crate::tick_host::GuiTickHost;

/// The attached Client: the `App` the shared tick's surface phases read. It
/// exists once the Owner link attaches.
struct Session {
    host: GuiTickHost,
}

pub(crate) struct GuiClient {
    session: Option<Session>,
    /// Why the window holds no Client session: a failed Owner find, start,
    /// or attach, including the shared refusal texts. Painted in the window.
    notice: Option<String>,
}

impl GuiClient {
    pub(crate) fn new(cc: &eframe::CreationContext<'_>) -> Self {
        fonts::install(&cc.egui_ctx);
        match open_session(cc) {
            Ok(session) => Self {
                session: Some(session),
                notice: None,
            },
            Err(notice) => Self {
                session: None,
                notice: Some(notice),
            },
        }
    }

    /// The no-session window's surface: one themed panel with the attach
    /// notice. A failed Owner find, start, or attach is the only thing this
    /// window can say before the Client session exists.
    fn paint_notice(&self, ui: &mut egui::Ui) {
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(theme_fill()))
            .show(ui, |ui| {
                if let Some(notice) = &self.notice {
                    ui.painter().text(
                        ui.max_rect().left_top() + egui::vec2(16.0, 16.0),
                        egui::Align2::LEFT_TOP,
                        notice,
                        egui::FontId::proportional(13.0),
                        role_color(mbv_theme::Role::StatusError),
                    );
                }
            });
    }
}

/// Attach to the Owner and install the window's waker on the Client core.
/// The waker is `egui::Context::request_repaint` (design D7): the Owner
/// snapshot relays wake a sleeping window when input lands. The core holds
/// nothing but the wake slot it shares with the `App`, and the tick is a
/// core-associated call, so the value itself need not outlive the install.
fn open_session(cc: &eframe::CreationContext<'_>) -> Result<Session, String> {
    let app = attach::attach()?;
    let core = ClientCore::from_app(&app);
    let ctx = cc.egui_ctx.clone();
    core.install_waker({
        let waker_ctx = ctx.clone();
        move || waker_ctx.request_repaint()
    });
    Ok(Session {
        host: GuiTickHost::new(app, ctx),
    })
}

impl eframe::App for GuiClient {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // The shared tick runs at the start of every frame (design D7): the
        // App-side drains and expiries, then this Client's input phase inside
        // them. eframe also calls `logic` with no `ui` while the window is
        // hidden, so the tick runs then too. The input phase takes its events
        // (`GuiTickHost::tick_input`), so those passes do not route them twice.
        if let Some(session) = &mut self.session {
            let outcome = ClientCore::tick(&mut session.host, Instant::now());
            if outcome.quit_requested {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            // The surface sync runs before the paint, once per frame: the
            // GUI Client's analog of the TUI Client's sync pass (design D5).
            session.host.sync_surfaces();
        }
        // The idle cadence the TUI Client polls at: a repaint at least this
        // often keeps timers and worker results landing with no waker.
        ctx.request_repaint_after(CLIENT_IDLE_CADENCE);
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        if let Some(session) = &mut self.session {
            session.host.paint(ui);
        } else {
            self.paint_notice(ui);
        }
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        theme_fill().to_normalized_gamma_f32()
    }
}

/// The fill the window paints behind every surface: the Queue column's
/// resting fill, which is what the first slice covers.
fn theme_fill() -> egui::Color32 {
    to_egui(surface_colors(Surface::QueueColumn, false).fill)
}

/// The neutral colour value converted to the toolkit colour (task 1.3).
fn to_egui(rgb: mbv_theme::Rgb) -> egui::Color32 {
    egui::Color32::from_rgb(rgb.r, rgb.g, rgb.b)
}

fn role_color(role: mbv_theme::Role) -> egui::Color32 {
    to_egui(role.color())
}

#[cfg(test)]
mod tests;
