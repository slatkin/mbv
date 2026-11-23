//! The GUI Client's test support: the `App` a wrapper parks while it moves the
//! live one into a `GuiCore` for a single call, so a test seeds a `GuiCore`
//! through the same wrapper the core thread uses.

/// The parked `App` a wrapper moves out of position: the stub construction,
/// which the wrapper drops as soon as the real `App` comes back.
#[must_use]
pub fn handoff_app() -> mbv::App {
    mbv::stub_app_with_queue(0)
}
