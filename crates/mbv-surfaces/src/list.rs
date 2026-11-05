//! The shared list seam: one ordered [`RowFlow`] with the cursor, viewport,
//! and mark mechanics every list shape composes over.
//!
//! The seam is front-end-neutral Surface state (design D2): it depends on no
//! terminal library, terminal UI framework, or GUI toolkit, and its viewport
//! arithmetic counts in items.

mod cursor;
mod marks;
mod row_flow;
mod viewport;

#[doc(inline)]
pub use self::cursor::Cursored;
#[doc(inline)]
pub use self::marks::{AggregateMarkState, MarkSelection, MarkSelectionState};
#[doc(inline)]
pub use self::row_flow::{Row, RowFlow};
#[doc(inline)]
pub use self::viewport::{
    PagingPolicy, ViewportAnchor, Viewported, max_viewport_offset_for_len, scrolled_viewport_offset,
};
