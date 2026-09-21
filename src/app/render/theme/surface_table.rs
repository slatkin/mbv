//! The surface table's rows: each `Surface`'s level, focus source, soft
//! variant and resting value, plus the declared resting deviations.
//!
//! Kept apart from the identity set (`surface.rs`) and the resolver
//! (`surface_resolve.rs`) so the table is readable in one place and the enum
//! stays complete by construction.
//!
//! Every value is main's: the focused fill is what the site paints while its
//! own bit is true, the resting fill what it paints while the bit is false,
//! and a fixed site pins focused == resting (design D2). The rows below were
//! re-derived from this tree, not from the declined branch (D6). The special
//! sites of design D3 are enumerated here, with their current evidence:
//!
//! (a) **Fixed-fill sites** (focused == resting): the wide hero split gap
//!     (`components/wide_hero_boundary.rs:121`); the selected-row punch-through
//!     (`render/components/widgets.rs:136`, `list_rows.rs:341,354,383,469`,
//!     `media_list/wide.rs:202,268`); the context menu's selected row
//!     (`render/components/context_menu.rs:38`); the recess rows
//!     (`chrome_player.rs:122,158,239`, `artwork_placeholder.rs:9`,
//!     `card.rs:111`); every chrome band (`chrome_status.rs`,
//!     `queue.rs`, `widgets.rs`, `hero.rs`, `home.rs:402`, `chrome.rs`,
//!     `chrome_tabs.rs:43`); the popup frame and its dim backdrop
//!     (`modal_frame.rs:47`, `backdrop.rs:8-19`).
//! (b) **`SelectedRowSurface` policy family** (`components/media_list/mod.rs:198`):
//!     `ListBackdrop` resolves the `SelectedRow` row (the punch-through),
//!     row), `OwningSurface` resolves the containing surface's focus pair
//!     (`media_list/wide.rs:202-203`). The `OwningSurface` callers declare
//!     `SelectedRowOnQueueColumn` (`render/components/queue.rs:36`) or
//!     `SelectedRowOnLibraryPane` (`tv_wide.rs:573-576`,
//!     `music_wide.rs:537-538`).
//!     NOTE (accepted selected-row bar): the selected-row painter now paints
//!     the opaque `SELECTED_ROW_BG` bar for every selected row and
//!     `selected_row_surface_color` ignores this family, so these
//!     `SelectedRow*` rows no longer reach the screen; the family is kept for
//!     the surface-table contract and pending cleanup.
//! (c) **Cursor-driven sites** keep their predicate as the bit (D3(c)):
//!     `tv_wide.rs:254` (`ctx.focused && ctx.episode_cursor.is_some()`),
//!     `card.rs:245` (the queue column's focus), and the highlight gates of the
//!     Audiobookshelf podcast (`audiobookshelf_podcast.rs:235`,
//!     `:418`) and book (`audiobookshelf_book.rs:165,184`) screens.
//! (d) **The hero-pane match** (`render/arrangements/wide_hero.rs:302-303`):
//!     `ReadOnly` paints `SURFACE_RESTING`, `Workspace(held)` resolves the
//!     held bit. The row only carries its two values; the call site collapses
//!     the two match arms to the one bool it effectively computes (D3(d)).
//!
//! One appearance main has that the normal-mode panel pair cannot express is
//! the Queue-only playback strip (`shell_draw.rs:285,296,315`): with no right
//! column on screen, the panel body and its recess rows paint `SURFACE_CHROME`
//! in both bool states. That is a mode-driven appearance, not a focus-driven
//! one, so it is its own fixed identity (`QueueOnlyPlaybackPanel`) rather than
//! a second input to the resolver: the queue-only branch names it, and its
//! recess rects share the value through the panel context.

#![cfg_attr(not(test), allow(dead_code))]

use super::surface::{FocusSource, Level, Row, Surface};
use super::*;
use ratatui::style::Color;

/// The row for one surface: its level, which column's focus (if any) drove
/// its focused appearance in main, whether it takes the soft content-body
/// variant, and the value it paints while resting.
pub(super) const fn row(surface: Surface) -> Row {
    match surface {
        // --- column/pane ---
        //
        // The queue column's gutter and the queue boundary strip resolve the
        // queue column's focus; its resting value is the level default.
        // Evidence: `render/components/chrome.rs:33`,
        // `components/queue_boundary.rs:121`.
        Surface::QueueColumn => Row {
            level: Level::ColumnPane,
            focus: FocusSource::QueueColumn,
            soft: false,
            resting: SURFACE_RESTING,
        },
        // The right column's whole gutter and body: the shell paints it in
        // `shell_library_panel.rs`'s `library_body_fill`, which passes the
        // panel's own focus bit, so the column lightens to the level's
        // `SURFACE_FOCUSED` fill while the library panel holds focus. Resting
        // it keeps the app backdrop the column has always painted (declared
        // as a deviation below).
        Surface::LibraryColumn => Row {
            level: Level::ColumnPane,
            focus: FocusSource::LibraryColumn,
            soft: false,
            resting: SURFACE_BACKDROP,
        },
        // The wide hero split gap paints the backdrop in every frame today
        // (`components/wide_hero_boundary.rs:121`).
        Surface::WideSplitGutter => Row {
            level: Level::ColumnPane,
            focus: FocusSource::Fixed,
            soft: false,
            resting: SURFACE_BACKDROP,
        },
        // The Wide hero's sheet: the same dark chrome the Library Hero
        // overlay paints when the same hero is shown over a non-Wide browser,
        // for every destination (user decision 2026-09-20). It is the sheet
        // the hero content repaints its own background with, not a
        // focus-reactive content surface, so the row is fixed: the pane no
        // longer lightens with the panel bit.
        Surface::HeroPane => Row {
            level: Level::ColumnPane,
            focus: FocusSource::Fixed,
            soft: false,
            resting: Palette::Ink.color(),
        },
        // The library punch-through: the backdrop beneath the list panel
        // shows through, never the panel's focus green. Evidence:
        // `render/components/widgets.rs:136`, `list_rows.rs:341,354,383,469`,
        // the `SelectedRowSurface::ListBackdrop` arm at
        // `media_list/wide.rs:202`, and the grid cell at `wide.rs:268`.
        Surface::SelectedRow => Row {
            level: Level::ColumnPane,
            focus: FocusSource::Fixed,
            soft: false,
            resting: SURFACE_BACKDROP,
        },
        // The queue list's selected row is a hole in the queue column: it
        // shows the column's own fill, resolved with the queue column's focus.
        // Evidence: `render/components/queue.rs:36` (the policy focused bit).
        Surface::SelectedRowOnQueueColumn => Row {
            level: Level::ColumnPane,
            focus: FocusSource::QueueColumn,
            soft: false,
            resting: SURFACE_RESTING,
        },
        // TV's episode list and Music's track list are holes in the library
        // pane: they show the pane's fill, resolved with the library column's
        // focus. Evidence: `render/components/tv_wide.rs:573-576`,
        // `music_wide.rs:537-538`.
        Surface::SelectedRowOnLibraryPane => Row {
            level: Level::ColumnPane,
            focus: FocusSource::LibraryColumn,
            soft: false,
            resting: SURFACE_RESTING,
        },
        // A popup's context-menu row is the one selected row at this level
        // with a different value; declared rather than silently repainted
        // (`render/components/context_menu.rs:38`).
        Surface::ContextMenuSelectedRow => Row {
            level: Level::ColumnPane,
            focus: FocusSource::Fixed,
            soft: false,
            resting: ACCENT_ACTIVE,
        },
        // --- content body ---
        // The wide-hero rail body and frame, and the same identity across
        // Movies, TV, Music, ABS books/podcasts and Feeds:
        // `render/arrangements/wide_hero.rs:415` (border at `:422`),
        // `components/browser/paint.rs:108`, `tv_wide.rs:312`,
        // `music_wide.rs:581`, `audiobookshelf_book.rs:197`,
        // `audiobookshelf_podcast.rs:264`, `feeds.rs:211`, `home.rs:370`.
        // Its focused fill is the soft content-body sheet; resting remains
        // the table value.
        Surface::LibraryPanel => Row {
            level: Level::ContentBody,
            focus: FocusSource::LibraryColumn,
            soft: true,
            resting: SURFACE_RESTING,
        },
        // The queue panel body (`render/components/widgets.rs:237-241`).
        Surface::QueuePanel => Row {
            level: Level::ContentBody,
            focus: FocusSource::QueueColumn,
            soft: true,
            resting: SURFACE_BACKDROP,
        },
        // A pane's content box: TV's episode listing
        // (`render/components/tv_wide.rs:515`), Music's track listing
        // (`render/arrangements/wide_hero.rs:468`, chosen at
        // `music_wide.rs:528-532`) and the generic pane inset
        // (`wide_hero.rs:467`) are the same identity. Main's focused half is
        // the soft variant; its resting half is the backdrop the generic
        // painter writes.
        Surface::MainContentBox => Row {
            level: Level::ContentBody,
            focus: FocusSource::LibraryColumn,
            soft: true,
            resting: SURFACE_BACKDROP,
        },
        // The selected row's inline detail (`render/components/hero.rs:192`).
        Surface::InlineHero => Row {
            level: Level::ContentBody,
            focus: FocusSource::LibraryColumn,
            soft: false,
            resting: SURFACE_RESTING,
        },
        // The now-playing panel body: the projection at
        // `shell_playback.rs:47-51` and the pre-sync default at
        // `components/playback.rs:55`.
        Surface::PlaybackPanel => Row {
            level: Level::ContentBody,
            focus: FocusSource::QueueColumn,
            soft: false,
            resting: SURFACE_RESTING,
        },
        // The Queue-only playback strip (`shell_draw.rs:285,296,315`): with no
        // right column on screen, the shell paints the panel body and its
        // recess rows as a chrome band in every frame — the recess rects take
        // the value through the panel context (`ctx.panel_bg`), not from their
        // own focus. The value is the chrome band's (`DARK_BG`), so a chrome
        // band edit reaches it; the fixed row pins focused == resting.
        Surface::QueueOnlyPlaybackPanel => Row {
            level: Level::ChromeBand,
            focus: FocusSource::Fixed,
            soft: false,
            resting: SURFACE_CHROME,
        },
        // The expanded (F1-F4) sidebar body paints the resting content value
        // (`render/components/chrome.rs:166-170`).
        Surface::SidebarBody => Row {
            level: Level::ContentBody,
            focus: FocusSource::Fixed,
            soft: false,
            resting: SURFACE_RESTING,
        },
        // --- recess ---
        // The now-playing panel's own content rows follow the panel's fill
        // (`render/components/chrome_player.rs:43,52,65,93,109,197,406`, via
        // `ctx.panel_bg`).
        Surface::PlaybackRecess => Row {
            level: Level::Recess,
            focus: FocusSource::QueueColumn,
            soft: false,
            resting: SURFACE_RESTING,
        },
        Surface::PlaybackStatusPill => Row {
            level: Level::Recess,
            focus: FocusSource::Fixed,
            soft: false,
            resting: SURFACE_BACKDROP,
        },
        Surface::ArtworkPlaceholder => Row {
            level: Level::Recess,
            focus: FocusSource::Fixed,
            soft: false,
            resting: SURFACE_BACKDROP,
        },
        // Main paints the artwork-loading inset's fill (the muted grey
        // value): `card.rs:111`, `album_art.rs:183`, the panel inline-hero
        // painter, `home_hero_emby.rs:121,272,286`. The row resolves through
        // its own purpose-named value (`Palette::Grey2`), deliberately not
        // the text role `TEXT_MUTED`, whose `Palette::Grey2` value it shares
        // today: a text edit can never move the fill.
        Surface::ArtworkLoadingPlaceholder => Row {
            level: Level::Recess,
            focus: FocusSource::Fixed,
            soft: false,
            resting: Palette::Grey2.color(),
        },
        // --- chrome band ---
        // The queue card's visualizer fills the same band the queue playback
        // panel paints, so it takes that band's value in both focus states
        // rather than following the queue column's: the reserved slot never
        // flashes a fill the panel around it does not have
        // (`render/components/card.rs:233-236`).
        Surface::QueueCardVisualizer => Row {
            level: Level::ChromeBand,
            focus: FocusSource::Fixed,
            soft: false,
            resting: SURFACE_CHROME,
        },
        Surface::StatusBar => Row {
            level: Level::ChromeBand,
            focus: FocusSource::Fixed,
            soft: false,
            resting: SURFACE_CHROME,
        },
        Surface::StatusBarPill => Row {
            level: Level::ChromeBand,
            focus: FocusSource::Fixed,
            soft: false,
            resting: SURFACE_CHROME,
        },
        Surface::QueuePanelBand => Row {
            level: Level::ChromeBand,
            focus: FocusSource::Fixed,
            soft: false,
            resting: SURFACE_CHROME,
        },
        Surface::PillRow => Row {
            level: Level::ChromeBand,
            focus: FocusSource::Fixed,
            soft: false,
            resting: PILL_ROW_BG,
        },
        Surface::PillChip => Row {
            level: Level::ChromeBand,
            focus: FocusSource::Fixed,
            soft: false,
            resting: PILL_BG,
        },
        Surface::PillChipSelected => Row {
            level: Level::ChromeBand,
            focus: FocusSource::Fixed,
            soft: false,
            resting: PILL_SELECTED_BG,
        },
        // The queue's selected scope pill paints the Direct-remote aqua
        // (`CONTEXT.md`, "Direct remote control"; `render/components/queue.rs:259,271`)
        // rather than the library pill's selected blue, so it carries its own
        // row.
        Surface::QueueScopePillSelected => Row {
            level: Level::ChromeBand,
            focus: FocusSource::Fixed,
            soft: false,
            resting: ACCENT,
        },
        // The Selector row's spacer band: the panel showing through, so it
        // follows the panel's own focus bit and resolves the column body's
        // value — focused the level's `SURFACE_FOCUSED` fill, resting the app
        // backdrop. The one chrome band that follows focus, because it is a
        // reserved row *inside* the panel rather than structural chrome
        // around it (ui-design-language: "the spacer row SHALL inherit the
        // parent panel background").
        Surface::PillRowGap => Row {
            level: Level::ChromeBand,
            focus: FocusSource::LibraryColumn,
            soft: false,
            resting: SURFACE_BACKDROP,
        },
        Surface::SidebarBand => Row {
            level: Level::ChromeBand,
            focus: FocusSource::Fixed,
            soft: false,
            resting: SURFACE_CHROME,
        },
        Surface::TabBar => Row {
            level: Level::ChromeBand,
            focus: FocusSource::Fixed,
            soft: false,
            resting: SURFACE_CHROME,
        },
        // --- popup ---
        // Every modal caller passes `SURFACE_FOCUSED` as its frame background
        // (`render/components/modal_frame.rs:47` and its remaining callers).
        Surface::PopupFrame => Row {
            level: Level::Popup,
            focus: FocusSource::Fixed,
            soft: false,
            resting: SURFACE_FOCUSED,
        },
        // The dim backdrop paints no fill of its own: it blends every
        // existing cell halfway toward black (see
        // `super::super::components::backdrop::dim`), so its row is that blend
        // base rather than a rect fill. Main has no role for the shade, so the
        // row names the `Color::Black` variant the painter's arm uses, not a
        // new literal.
        Surface::PopupDimBackdrop => Row {
            level: Level::Popup,
            focus: FocusSource::Fixed,
            soft: false,
            resting: Color::Black,
        },
    }
}

impl Surface {
    /// The surface's nesting level, stated once (design D2; the delta spec's
    /// "Each surface entry SHALL state its nesting level").
    pub(super) const fn level(self) -> Level {
        row(self).level
    }
}

/// The rows whose resting value is not their level's resting default, with
/// the reason.
///
/// Declared next to the table so the deviation is visible and removable one at
/// a time (design D2), and so a resting-value drift fails
/// `resting_values_are_default_or_declared` rather than hiding.
pub(super) const RESTING_DEVIATIONS: &[(Surface, &str)] = &[
    (
        Surface::LibraryColumn,
        "the library column rests as the app backdrop (`SURFACE_BACKDROP`) \
         rather than the column/pane level's resting value; its focused half \
         takes `SURFACE_FOCUSED`",
    ),
    (
        Surface::WideSplitGutter,
        "the split gap between the wide hero panes is painted as the app backdrop \
         in every frame",
    ),
    (
        Surface::SelectedRow,
        "the punch-through reveals the backdrop beneath the list panel, not the \
         column's resting content value",
    ),
    (
        Surface::ContextMenuSelectedRow,
        "a popup's selected row paints `ACCENT_ACTIVE`, not the column/pane level's \
         value",
    ),
    (
        Surface::QueuePanel,
        "the queue panel's recessed resting box paints the app backdrop",
    ),
    (
        Surface::HeroPane,
        "the Wide hero's sheet paints the Library Hero overlay's dark chrome \
         (`Palette::Ink`), not the column/pane level's resting value",
    ),
    (
        Surface::MainContentBox,
        "the pane content box's resting half has always painted the app backdrop",
    ),
];

#[cfg(test)]
mod tests {
    use super::*;

    fn deviation_for(surface: Surface) -> Option<&'static str> {
        RESTING_DEVIATIONS
            .iter()
            .find(|(s, _)| *s == surface)
            .map(|(_, reason)| *reason)
    }

    /// Every row's resting value is its level's resting default or is on the
    /// declared deviation list, with a reason; a level with no default has
    /// nothing to deviate from. A future drift of one row's resting value
    /// fails here instead of hiding.
    #[test]
    fn resting_values_are_default_or_declared() {
        for &surface in Surface::ALL {
            let resting = row(surface).resting;
            let deviation = deviation_for(surface);
            match surface.level().resting_default() {
                Some(default) if resting != default => assert!(
                    deviation.is_some(),
                    "{surface:?} rests at {resting:?}, not the level default \
                     {default:?}, and declares no deviation"
                ),
                Some(_) => assert!(
                    deviation.is_none(),
                    "{surface:?} rests at the level default and must not declare \
                     a deviation"
                ),
                None => assert!(
                    deviation.is_none(),
                    "{surface:?}: a level with no resting default has nothing to \
                     deviate from"
                ),
            }
        }
        for (surface, reason) in RESTING_DEVIATIONS {
            assert!(
                Surface::ALL.contains(surface),
                "{surface:?} is declared but not in ALL"
            );
            assert!(!reason.is_empty(), "{surface:?} has a reason");
            assert_eq!(deviation_for(*surface), Some(*reason));
        }
    }
}
