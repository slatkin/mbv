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
//! * `ToCellSize`, the conversion from the terminal's font measurement to the
//!   neutral `mbv_ui_model::geom::CellSize` that pixel composition sizes with.
//! * The terminal image protocol, which speaks `ratatui-image` and `crossterm`
//!   on behalf of `mbv-images`.

mod cell;
mod protocol;
mod resize;
mod rgb;

pub use cell::ToCellSize;
pub use protocol::*;
pub use resize::{ResizeRegisterTx, ResizeResponseRx, spawn_resize_worker};
#[doc(inline)]
pub use rgb::ToRatatui;

/// The filter the terminal protocols resample with.
pub const RENDER_FILTER: ratatui_image::FilterType = ratatui_image::FilterType::Triangle;
