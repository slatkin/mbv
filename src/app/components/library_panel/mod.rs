//! The Library panel (design D2/D3): content types, the Wide and Narrow
//! skeletons' slot Render Components, the object-safe [`PanelList`] surface,
//! and the mounted [`panel::LibraryPanel`] AppComponent. The panel is the one
//! paint path and event boundary for library screens: destinations supply
//! only typed [`LibraryPanelContent`], and every row, pane, fill, border, gap
//! and placeholder position is painted here.
//!
//! Tasks 5.1–5.6 built the content types and the Wide skeleton; 5.7 adds the
//! Narrow skeleton and the inline hero; 5.8 formalizes `PanelList` over the
//! shared media-list carrier; 5.9 mounts the panel as `ComponentId::Library`.
//! Destinations convert one per slice (5.11+); until then the panel paints
//! only owners that have migrated.

pub mod content;
pub mod hero;
pub mod hero_header;
pub mod narrow;
pub mod overview_box;
pub mod owner;
pub mod panel;
pub mod panel_list;
pub mod slots;
pub mod wide;

#[allow(unused_imports)]
pub(in crate::app) use content::{
    ArtworkShape, ArtworkSource, HeroArtwork, HeroContent, HeroCredit, HeroFacts, HeroHeader,
    HeroImageState, HeroLink, LibraryPanelContent, ListControls, ListSlot, PanelHeroImagePaint,
    PanelList, SelectorRow, Workspace,
};
#[allow(unused_imports)]
pub(in crate::app) use hero::{
    emby_artwork_policy, hero_content_abs_book, hero_content_abs_episode, hero_content_abs_show,
    hero_content_emby, hero_content_feed, hero_content_music_album, hero_content_queue,
    HeroContentData,
};
#[allow(unused_imports)]
pub(in crate::app) use narrow::{inline_hero_plan, render_narrow_skeleton, NarrowSkeletonGeometry};
#[allow(unused_imports)]
pub(in crate::app) use overview_box::sanitize_url;
pub(in crate::app) use owner::{LibraryContentOwner, LibrarySlotEvent};
#[allow(unused_imports)]
pub use owner::{LibraryKey, LibraryKind};
#[allow(unused_imports)]
pub(in crate::app) use panel::LibraryPanel;
#[allow(unused_imports)]
pub(in crate::app) use slots::{paint_list_controls_row, paint_pill_row_gap, paint_selector_row};
#[allow(unused_imports)]
pub(in crate::app) use wide::{render_wide_skeleton, SkeletonHits, WideSkeletonGeometry};
