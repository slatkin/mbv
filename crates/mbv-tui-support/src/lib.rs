//! Terminal-only support shared by the TUI crates (design D7).
//!
//! `mbv-theme` and `mbv-images` are leaf crates that keep no terminal library:
//! theme colours resolve to the neutral `mbv_theme::Rgb`, and images keep
//! decode, cache, and pixel processing. The two pieces that do need a
//! terminal toolkit live here instead, and the root package, `mbv-components`,
//! and `mbv-render` depend on this crate. Nothing in `mbv-gui` does.
//!
//! This crate holds:
//!
//! * `ToRatatui`, the conversion from `mbv_theme` colour values to `ratatui`
//!   colours, so a caller names the meaning and paints it.
//! * The terminal image protocol, which speaks `ratatui-image` and `crossterm`
//!   on behalf of `mbv-images`.

mod rgb;

#[doc(inline)]
pub use rgb::ToRatatui;
