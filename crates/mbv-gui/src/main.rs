//! The GUI Client program (design D2, D7, and D8 of `add-gui-front-end`).
//!
//! It is a separate program from the `mbv` TUI Client: it links the `mbv`
//! library for the `App`, the Client core tick, and the Owner link, and it
//! adds the window stack the `mbv` program must not carry. The window is
//! eframe with the glow renderer; every surface is painted with the egui
//! `Painter` from Surface state, never with a stateful egui widget
//! (rows 6.4 and 6.5).

mod app;
mod attach;
mod tick_host;

/// The window title, and the Wayland/X11 application id pinwin takes the
/// tray icon from.
const APP_ID: &str = "mbv";

/// The one top-level window this program opens (design D8): Wayland and X11
/// backends, the glow renderer, and no viewport other than the root one.
fn native_options() -> eframe::NativeOptions {
    eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_app_id(APP_ID)
            .with_title(APP_ID),
        ..eframe::NativeOptions::default()
    }
}

fn main() -> eframe::Result {
    eframe::run_native(
        APP_ID,
        native_options(),
        Box::new(|cc| Ok(Box::new(app::GuiClient::new(cc)))),
    )
}
