//! The closed surface identity set (change `unify-surface-colour-neutral`,
//! design D1/D2/D7).
//!
//! [`Surface`] and its [`Surface::ALL`] list are generated together by
//! `declare_surfaces!`, so a new variant cannot be added without appearing in
//! `ALL`. [`Level`] is D2's nesting level; [`FocusSource`] records which
//! panel-focus signal the archived inventory found driving a row's focused
//! appearance; [`Row`] carries everything a row's colour depends on. The rows
//! live in `surface_table` and the one resolver in `surface_resolve`.
//!
//! Only [`Surface`], [`Surface::ALL`] and the resolver's `surface_colors` are
//! crate-visible. The level and row accessors are private to the theme, so a
//! screen cannot resolve a *level* or a row instead of naming its surface and
//! calling the resolver.
//!
//! The identity names are the archived `unify-surface-colour` change's,
//! verbatim (D7): they are the reviewed vocabulary. Every row *value* is
//! re-derived against this tree — main's behaviour — rather than taken from
//! the declined branch (D2/D6), so the rows differ from the archived table
//! wherever the declined branch moved a surface (the library column, the row
//! above the pill bar) and never where main did not.

#![cfg_attr(not(test), allow(dead_code))]

use super::*;
use ratatui::style::Color;

/// The nesting level of a rendered surface (design D2).
///
/// A level is what an edit is expressed against: one change to a level's
/// focused appearance reaches every row of that level.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub(super) enum Level {
    /// The surface a content body sits on: the column gutters the shell
    /// paints and the hero pane fill.
    ColumnPane,
    /// A focusable content region that holds content: a panel's body
    /// (Library/Queue), a screen's main content box, a sidebar's body, a
    /// popup's list.
    ContentBody,
    /// A non-focusable inset inside a content body (the now-playing panel's
    /// own rows and status pills, the visualizer background).
    Recess,
    /// Non-focusable structural chrome that never follows panel focus (the
    /// tab bar, the status bar, a column's header/status rows, the pill row
    /// and the spacer band below it, a sidebar's header/footer).
    ChromeBand,
    /// An overlay frame and its dim backdrop, which never follow a panel.
    Popup,
}

impl Level {
    /// The fill a focus-driven row of this level takes while its column (or
    /// pane) holds focus.
    ///
    /// `ColumnPane` and `ContentBody` deliberately share the value today, and
    /// `Recess` shares it too because its focus-driven row (the now-playing
    /// recess, and the TV/Music pane content boxes) reads as content.
    /// `ChromeBand` and `Popup` have no focus-driven row in main; their value
    /// records the level's intent for one.
    pub(super) const fn focused_fill(self) -> Color {
        match self {
            Level::ColumnPane
            | Level::ContentBody
            | Level::Recess
            | Level::Popup
            | Level::ChromeBand => SURFACE_FOCUSED,
        }
    }

    /// The resting fill a row of this level defaults to, when the level has
    /// one.
    ///
    /// Only the two content-bearing levels have a single declared resting
    /// appearance; recesses, chrome bands and popups paint their own value
    /// today (design D2: "as each surface paints today"), so they have no
    /// default to deviate from.
    pub(super) const fn resting_default(self) -> Option<Color> {
        match self {
            Level::ColumnPane | Level::ContentBody => Some(SURFACE_RESTING),
            Level::Recess | Level::ChromeBand | Level::Popup => None,
        }
    }
}

/// Which panel-focus signal the archived inventory found driving a row's
/// focused appearance.
///
/// The one resolver takes the *call site's own* bool (design D1), so this is
/// documentation of where that bool comes from, not a second input: a
/// production site names its surface and passes the focus input it already
/// computes. Only [`FocusSource::Fixed`] changes the resolved colour — a fixed
/// row pins focused == resting (design D3(a)) — while `QueueColumn` and
/// `LibraryColumn` tell the migration which column's bit the site passes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum FocusSource {
    Fixed,
    QueueColumn,
    LibraryColumn,
}

/// One row of the table: everything a surface's colour depends on.
#[derive(Clone, Copy, Debug)]
pub(super) struct Row {
    pub(super) level: Level,
    pub(super) focus: FocusSource,
    /// Takes the soft content-body variant (`SOFT_CONTENT_BODY_BG`,
    /// `#48584e`) while focused, instead of the level's focused fill.
    pub(super) soft: bool,
    /// The value the row paints while resting: main's value for that surface.
    pub(super) resting: Color,
}

/// Declares the closed `Surface` set and the enumerable [`Surface::ALL`] list
/// from one variant list, so a new variant cannot be added without appearing
/// in `ALL` (and `row`/`border` fail to compile until it is placed).
macro_rules! declare_surfaces {
    ($($variant:ident),+ $(,)?) => {
        /// A rendered-surface identity: a structural position in the layout,
        /// not a screen. The same position in a different screen or provider
        /// is the same identity; what the surface holds is content.
        #[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
        pub(in crate::app) enum Surface {
            $($variant),+
        }

        impl Surface {
            /// Every declared surface identity, in declaration order.
            pub(in crate::app) const ALL: &'static [Surface] = &[$(Surface::$variant),+];
        }
    };
}

declare_surfaces!(
    // --- column/pane: the surface a content body sits on ---
    QueueColumn,
    LibraryColumn,
    WideSplitGutter,
    HeroPane,
    SelectedRow,
    SelectedRowOnQueueColumn,
    SelectedRowOnLibraryPane,
    ContextMenuSelectedRow,
    // --- content body: a focusable content region ---
    LibraryPanel,
    QueuePanel,
    MainContentBox,
    InlineHero,
    PlaybackPanel,
    // The Queue-only playback strip: a mode-driven chrome-band appearance, not
    // a focus-driven one (see `surface_table`'s row doc).
    QueueOnlyPlaybackPanel,
    SidebarBody,
    NonHeroSidebarBody,
    // The queue card's now-playing content: an inset by geography, but it
    // resolves the content-body pair (design D2 row shape).
    QueueCardVisualizer,
    // --- recess: a non-focusable inset inside a content body ---
    PlaybackRecess,
    PlaybackBottomRow,
    PlaybackStatusPill,
    ArtworkPlaceholder,
    ArtworkLoadingPlaceholder,
    // --- chrome band: non-focusable structural chrome ---
    StatusBar,
    StatusBarPill,
    QueuePanelBand,
    PillRow,
    PillChip,
    PillChipSelected,
    // The queue's selected scope pill: its aqua is the Direct-remote
    // indicator (`CONTEXT.md`, "Direct remote control"), not the library
    // pill's selected blue, so it is its own row rather than a variant of
    // `PillChipSelected`.
    QueueScopePillSelected,
    PillRowGap,
    SidebarBand,
    NonHeroSidebarBand,
    TabBar,
    // --- popup: an overlay frame and its dim backdrop ---
    PopupFrame,
    PopupDimBackdrop,
);
