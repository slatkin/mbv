//! Provider-neutral embedded media-list controls (design.md D1/D2/D3).
//!
//! [`WideMediaList`] is the fixed-row presentation over one [`MediaList`]
//! owner. Painting lives in `mbv-render::components::media_list`.
//!
//! The canonical owner is front-end-neutral Surface state and lives in
//! `mbv-surfaces` (add-gui-front-end design D2/D5); this module re-exports it
//! for the TUI adapters and keeps the TUI-side carrier, wide presentation,
//! pointer-input mapping, and letter grouping.

mod carrier;
mod grouping;
#[cfg(test)]
mod tests;
mod types;
mod wide;

#[doc(inline)]
pub use carrier::MediaListCarrier;
#[doc(inline)]
pub use grouping::letter_grouped_rows;
use mbv_render::components::media_list::WideMediaListPaintPolicy;
#[doc(inline)]
pub use mbv_surfaces::media_list::{
    MediaList, MediaListDisposition, MediaListOperation, MediaListPaintBorrow, MediaListTransition,
    RowIntent, VisualKeyOutcome, WideViewport,
};
#[doc(inline)]
pub use types::MediaListSurfaceInput;
pub(crate) use wide::render_wide_media_list;
#[doc(inline)]
pub use wide::{WideMediaList, WidePresentationState};
