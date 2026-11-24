//! The `mbv-gui` library: the native window Client's core thread (design D3
//! of the archived `add-tauri-gui-client`) and the bridge between the window
//! and the Rust Client core (design D4).

pub mod art;
pub mod attach;
pub mod bridge;
pub mod core_thread;
/// The GUI Client's own core: the `App` it owns, wrapped by composition, with
/// the GUI modules and the GUI routing entry (`crate::gui_core::GuiCore`).
pub mod gui_core;
mod leaves;
mod painted_choices;
pub mod sections;

/// Test support for the GUI Client's own tests: the `App` a wrapper parks while
/// it moves the live one into a `GuiCore` for a single call.
pub mod test_support;

/// The window half (D3 of `replace-tauri-with-blitz`): the Dioxus tree, the
/// launch copy, and the window channel. The `blitz` feature and its gate went
/// at the switch (D18).
pub mod window;
