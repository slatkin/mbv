//! The `mbv-gui` library: the native window Client's core thread (design D3
//! of `add-tauri-gui-client`) and the bridge between the window's front end
//! and the Rust Client core (design D4).

pub mod art;
pub mod attach;
pub mod bridge;
pub mod core_thread;
mod leaves;
mod painted_choices;
pub mod sections;

/// The Blitz window half (D3 of `replace-tauri-with-blitz`): the Dioxus tree,
/// the launch copy, and the window channel. Gated on the `blitz` feature for
/// the port; the switch (D18) drops the gate.
#[cfg(feature = "blitz")]
pub mod window;
