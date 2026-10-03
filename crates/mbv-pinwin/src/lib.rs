//! Safe Rust wrapper over the vendored pinwin panel C ABI.
//!
//! [`Panel::start`] hands the pty master to the library, which runs the panel
//! on its own GTK thread until the returned handle drops. The public surface is
//! owned types and a result type; the raw pointers and repr(C) structs of the
//! C ABI stay inside this crate.

mod error;
mod ffi;
mod layout;
mod panel;

pub use error::PinwinError;
pub use layout::{Gutters, KeyboardMode, Layout, Side};
pub use panel::Panel;
