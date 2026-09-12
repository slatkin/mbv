//! Narrow TV series-list inputs for the canonical inline media-browser
//! composition.
//!
//! `BrowserComponent` retains this transitional input bundle for the
//! unregistered destinations. TV no longer uses it: its Narrow surface is
//! painted by the Library panel skeleton from the same `HeroContent` as Wide.

/// Shell-resolved extras the narrow composer needs beyond the mirrored
/// [`LibraryListRenderCtx`]. Built by `App::narrow_browse_extras`.
#[derive(Default)]
pub(in crate::app) struct NarrowBrowseExtras {
    /// Render the letter-range pill row above the list.
    pub(in crate::app) show_letter_pills: bool,
    /// `collection_type` is `movies`/`tvshows`: the inline-hero rows are
    /// always reserved rather than dropped when they don't fit the flow.
    pub(in crate::app) use_shared_replacement_plan: bool,
    /// No hero item is selected but the surface is a hero-capable collection
    /// at its top browse level: keep the fixed placeholder panel reserved.
    pub(in crate::app) hero_placeholder: bool,
    /// Narrow TV season grid (`is_viewing_season_grid`): force a single-column
    /// stride instead of the width-derived column count (legacy `list.rs`).
    pub(in crate::app) season_grid: bool,
}
