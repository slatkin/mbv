//! Front-end-neutral Surface state for mbv's Clients (design D2).
//!
//! The crates in here hold the interaction state that the TUI Client and the
//! GUI Client share -- the core focus stack today, Surface state per shared
//! surface as the extraction lands -- so that interaction behaviour lives
//! once. Nothing in this crate depends on a terminal library, a terminal UI
//! framework, or a GUI toolkit.

mod capacity;
mod focus_stack;
mod hit_map;
pub mod list;
pub mod media_list;

#[doc(inline)]
pub use capacity::ListCapacity;
#[doc(inline)]
pub use focus_stack::FocusStack;
#[doc(inline)]
pub use hit_map::{HitMap, HitRegion};
