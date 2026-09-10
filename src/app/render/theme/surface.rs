//! The closed surface identity set (`unify-surface-colour` design D2, row
//! 4.1).
//!
//! [`Surface`] and its [`Surface::ALL`] list are generated together by
//! `declare_surfaces!`, so a new variant cannot be added without appearing in
//! `ALL`. [`Level`] is D2's nesting level; [`FocusSource`] says whether a row
//! follows a column's panel focus; [`Row`] carries everything a row's colour
//! depends on. The rows live in `surface_table` and the one resolver in
//! `surface_resolve`.
//!
//! Only [`Surface`], [`Surface::ALL`] and the resolver's `surface_colors` /
//! `surface_colors_for_column_focus` are visible to `crate::app`. The level and
//! row accessors are private to the theme, so a screen cannot resolve a
//! *level* or a row instead of naming its surface and calling the resolver.

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
    /// recess) reads as content. `ChromeBand` and `Popup` have no focus-driven
    /// row yet; their value records the level's intent for one.
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

/// Which column's panel focus drives a row's focused appearance.
///
/// `Fixed` rows paint one value in every frame: the recesses, chrome bands and
/// popups that never follow a panel, the wide split gap, and the
/// selected-row punch-through, whose colour is the backdrop beneath it.
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
    /// Takes the soft content-body variant (`SURFACE_ACCENT_SOFT`) while
    /// focused, instead of the level's focused fill.
    pub(super) soft: bool,
    /// The value the row paints while resting: today's value for that
    /// surface.
    pub(super) resting: Color,
}

/// Declares the closed `Surface` set and the enumerable [`Surface::ALL`] list
/// from one variant list, so a new variant cannot be added without appearing
/// in `ALL` (and `row`/`border` fail to compile until it is placed).
///
/// The two `allow(dead_code)` attributes cover the identities and `ALL` whose
/// painter migrates after row 4.2; row 5.2's conformance test enumerates
/// `ALL`, so they are consumed by that test rather than by a production paint
/// site. Narrowly scoped here and removed when 5.2 lands.
macro_rules! declare_surfaces {
    ($($variant:ident),+ $(,)?) => {
        /// A rendered-surface identity: a structural position in the layout,
        /// not a screen. The same position in a different screen or provider
        /// is the same identity; what the surface holds is content.
        #[allow(dead_code)] // row 5.2's conformance test enumerates ALL
        #[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
        pub(in crate::app) enum Surface {
            $($variant),+
        }

        impl Surface {
            /// Every declared surface identity, in declaration order.
            #[allow(dead_code)] // row 5.2's conformance test is the consumer
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
    SidebarBody,
    NonHeroSidebarBody,
    // The queue card's now-playing content: an inset by geography, but it
    // resolves the content-body pair (design D2 row shape, row 4.1 review).
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
