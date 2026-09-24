//! The Wide Hero header (task 5.5, design D5): three arms — Landscape
//! (16:9 artwork full content width above title/meta), Portrait (2:3) and
//! Square (1:1) (title/meta left, artwork right) — behind one title/meta
//! painter that colours meta row *n* with `HERO_META_ROLES[n % 3]` and owns
//! truncation and wrapping. The Landscape arm lays the title/meta entries
//! out in a two-column grid (title top-left, entries alternating columns
//! row by row, right column right-aligned) so short metadata uses the
//! full content width; Portrait/Square keep the stacked rows. The arm
//! comes from the artwork policy's shape,
//! re-armed to the projected image's decoded aspect once it resolves
//! (`HeroArtwork::painted_shape`), never from a destination. The artwork box is
//! sized by the paint-free [`hero_artwork_box`], the one layout site the
//! shell projection (task 5.10) shares with the painter.

mod artwork_box;
#[cfg(test)]
mod hero_header_tests;
mod title_meta;

/// Panes whose *terminal* is this short (or fewer rows) use compact caps so
/// the header leaves room for text, overview, and Workspace beneath it. The
/// measurement is the terminal, not pane, height, so panel chrome is excluded.
pub(in crate::app) const HERO_SHORT_PANE_MAX_HEIGHT: u16 = 50;
pub(in crate::app) use artwork_box::{hero_artwork_box, short_pane};
pub(in crate::app) use title_meta::paint_hero_pane_content;
