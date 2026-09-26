//! The Library panel (design D2/D3): content types, the Wide and Narrow
//! skeletons' slot Render Components, the object-safe [`PanelList`] surface,
//! and the mounted [`panel::LibraryPanel`] `AppComponent`. The panel is the one
//! paint path and event boundary for library screens: destinations supply
//! only typed [`LibraryPanelContent`], and every row, pane, fill, border, gap
//! and placeholder position is painted here.
//!
//! Tasks 5.1–5.6 built the content types and the Wide skeleton; 5.8
//! formalizes `PanelList` over the shared media-list carrier; 5.9 mounts the
//! panel as `ComponentId::Library`.
//! Destinations convert one per slice (5.11+); until then the panel paints
//! only owners that have migrated.

pub mod content;
pub mod hero;
pub mod hero_composition;
pub mod hero_header;
pub mod narrow;
pub mod overview_box;
pub mod owner;
pub mod panel;
pub mod panel_list;
pub mod slots;
pub mod wide;

#[cfg(test)]
pub(in crate::app) use content::HeroLink;
pub(in crate::app) use content::{
    ArtworkShape, HeroArtwork, HeroContent, HeroFacts, HeroImageState, LibraryPanelContent,
    ListSlot, PanelHeroImagePaint, SelectorRow, Workspace,
};
pub(in crate::app) use hero::{hero_content_emby, HeroContentData};
pub(in crate::app) use overview_box::sanitize_url;
pub(in crate::app) use owner::{LibraryContentOwner, LibrarySlotEvent};
pub use owner::{LibraryKey, LibraryKind};
pub(in crate::app) use panel::LibraryPanel;
