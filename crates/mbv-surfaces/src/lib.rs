//! Front-end-neutral Surface state for mbv's Clients (design D2).
//!
//! The crates in here hold the interaction state that the TUI Client and the
//! GUI Client share -- the core focus stack today, Surface state per shared
//! surface as the extraction lands -- so that interaction behaviour lives
//! once. Nothing in this crate depends on a terminal library, a terminal UI
//! framework, or a GUI toolkit.

mod focus_stack;

#[doc(inline)]
pub use focus_stack::FocusStack;
