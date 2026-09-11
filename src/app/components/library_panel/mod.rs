//! The Library panel (tasks 5.1–5.2, design D2/D3): content types plus the
//! Wide skeleton's slot Render Components. The panel is the one paint path
//! for library screens: destinations supply only typed
//! [`LibraryPanelContent`], and every row, pane, fill, border, gap and
//! placeholder position is painted here.
//!
//! This unit creates the module and its types and the Wide skeleton; the
//! `LibraryPanel` AppComponent itself is mounted in task 5.9, and
//! destinations convert in 5.11+. No existing destination changes here.

pub mod content;
pub mod hero;
pub mod hero_header;
pub mod slots;
pub mod wide;

#[allow(unused_imports)]
pub(in crate::app) use content::{
    ArtworkShape, ArtworkSource, HeroArtwork, HeroContent, HeroFacts, HeroHeader,
    LibraryPanelContent, ListControls, ListSlot, PanelList, SelectorRow, Workspace,
};
#[allow(unused_imports)]
pub(in crate::app) use hero::{
    emby_artwork_policy, hero_content_abs_book, hero_content_abs_episode, hero_content_abs_show,
    hero_content_emby, hero_content_feed, hero_content_queue, HeroContentData,
};
#[allow(unused_imports)]
pub(in crate::app) use slots::{paint_list_controls_row, paint_pill_row_gap, paint_selector_row};
#[allow(unused_imports)]
pub(in crate::app) use wide::{render_wide_skeleton, SkeletonHits, WideSkeletonGeometry};
