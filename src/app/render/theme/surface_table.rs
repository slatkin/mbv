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
//! (a) **Fixed-fill sites** (focused == resting): the library column gutter
//!     (`render/components/chrome.rs:61`, `SURFACE_BACKDROP` in every frame on
//!     main) and the wide Music browser container
//!     (`render/components/music_wide.rs:549`); the wide hero split gap
//!     (`components/wide_hero_boundary.rs:121`); the selected-row punch-through
//!     (`render/components/widgets.rs:151`, `list_rows.rs:341,354,383,469`,
//!     `media_list/wide.rs:202,268`); the context menu's selected row
//!     (`render/components/context_menu.rs:38`); the recess rows
//!     (`chrome_player.rs:122,158,239`, `artwork_placeholder.rs:9`,
//!     `card.rs:111`); every chrome band (`chrome_status.rs`,
//!     `queue.rs`, `widgets.rs`, `hero.rs`, `home.rs:402`, `chrome.rs`,
//!     `chrome_tabs.rs:43`); the popup frame and its dim backdrop
//!     (`modal_frame.rs:50`, `backdrop.rs:16-28`).
//! (b) **`SelectedRowSurface` policy family** (`components/media_list/mod.rs:198`):
//!     `ListBackdrop` resolves `list_selected_row_bg()` (the `SelectedRow`
//!     row), `OwningSurface` resolves the containing surface's focus pair
//!     (`media_list/wide.rs:202-203`). The `OwningSurface` callers declare
//!     `SelectedRowOnQueueColumn` (`render/components/queue.rs:36`) or
//!     `SelectedRowOnLibraryPane` (`tv_wide.rs:573-576`,
//!     `music_wide.rs:537-538`).
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
//! One appearance main has that this single-bool resolver cannot carry is the
//! Queue-only playback strip (`shell_draw.rs:286,297,316`): with no right
//! column on screen, the panel body and its recess rows paint
//! `SURFACE_CHROME` in both bool states. That is a mode-driven appearance, not
//! a focus-driven one, and it is deliberately recorded rather than encoded:
//! the migration unit that owns `shell_draw` dispositions it (D1 forbids a
//! second input here).

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
        // Main never follows focus here: the shell paints the right column's
        // whole gutter as the app backdrop in every frame
        // (`render/components/chrome.rs:61`) and the wide Music browser
        // container does the same (`render/components/music_wide.rs:549`), so
        // the row is fixed at `SURFACE_BACKDROP`.
        Surface::LibraryColumn => Row {
            level: Level::ColumnPane,
            focus: FocusSource::Fixed,
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
        // `CONTEXT.md` defines the Hero pane as the `#333c43` resting fill of
        // Wide hero's right pane, so this is the level default, not a
        // deviation. The call site is the `ReadOnly` / `Workspace(held)` match
        // at `render/arrangements/wide_hero.rs:302-303`.
        Surface::HeroPane => Row {
            level: Level::ColumnPane,
            focus: FocusSource::LibraryColumn,
            soft: false,
            resting: SURFACE_RESTING,
        },
        // The library punch-through: the backdrop beneath the list panel
        // shows through, never the panel's focus green. Evidence:
        // `render/components/widgets.rs:151`, `list_rows.rs:341,354,383,469`,
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
        Surface::LibraryPanel => Row {
            level: Level::ContentBody,
            focus: FocusSource::LibraryColumn,
            soft: false,
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
        // `shell_playback.rs:47-51`, the pre-sync default at
        // `components/playback.rs:55`, and the Queue-only strip
        // (`shell_draw.rs:286,297,316` — see the module note).
        Surface::PlaybackPanel => Row {
            level: Level::ContentBody,
            focus: FocusSource::QueueColumn,
            soft: false,
            resting: SURFACE_RESTING,
        },
        // The expanded (F1-F4) sidebar body paints the resting content value
        // (`render/components/chrome.rs:166-170`).
        Surface::SidebarBody => Row {
            level: Level::ContentBody,
            focus: FocusSource::Fixed,
            soft: false,
            resting: SURFACE_RESTING,
        },
        // The non-hero sidebar shell is its own appearance
        // (`render/components/chrome.rs:169`).
        Surface::NonHeroSidebarBody => Row {
            level: Level::ContentBody,
            focus: FocusSource::Fixed,
            soft: false,
            resting: SURFACE_SIDEBAR,
        },
        // The queue card's now-playing content is the queue column's content,
        // so it resolves the content-body pair with the queue column's focus
        // even though it sits inside the card
        // (`render/components/card.rs:245-248`).
        Surface::QueueCardVisualizer => Row {
            level: Level::ContentBody,
            focus: FocusSource::QueueColumn,
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
        // Main paints this row whenever the panel is four rows high
        // (`render/components/chrome_player.rs:122,158`), in every mode; only
        // its "On Now" title is Mini-only (`:127`).
        Surface::PlaybackBottomRow => Row {
            level: Level::Recess,
            focus: FocusSource::Fixed,
            soft: false,
            resting: SURFACE_BACKDROP,
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
        // Main uses the `BORDER_UNFOCUSED` rule role as this fill
        // (`render/components/card.rs:111`, `album_art.rs:183`,
        // `detail_series_view.rs:125`, `home_hero_emby.rs:121,272,286`); the
        // row records today's role, not a name from a later retirement.
        Surface::ArtworkLoadingPlaceholder => Row {
            level: Level::Recess,
            focus: FocusSource::Fixed,
            soft: false,
            resting: BORDER_UNFOCUSED,
        },
        // --- chrome band ---
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
        // Home's pill-bar spacer band paints the backdrop today
        // (`render/components/home.rs:402`).
        Surface::PillRowGap => Row {
            level: Level::ChromeBand,
            focus: FocusSource::Fixed,
            soft: false,
            resting: SURFACE_BACKDROP,
        },
        Surface::SidebarBand => Row {
            level: Level::ChromeBand,
            focus: FocusSource::Fixed,
            soft: false,
            resting: SURFACE_CHROME,
        },
        Surface::NonHeroSidebarBand => Row {
            level: Level::ChromeBand,
            focus: FocusSource::Fixed,
            soft: false,
            resting: SURFACE_ITEM_FOCUSED,
        },
        Surface::TabBar => Row {
            level: Level::ChromeBand,
            focus: FocusSource::Fixed,
            soft: false,
            resting: SURFACE_CHROME,
        },
        // --- popup ---
        // Every modal caller passes `SURFACE_FOCUSED` as its frame background
        // (`render/components/modal_frame.rs:50` and its ten callers, e.g.
        // `confirm_modal.rs:29`, `selection_modal.rs:65,96`).
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

    /// The border colour this surface frames itself with, when it paints one.
    ///
    /// Only the Wide hero library/rail panel paints a surface border: its
    /// `▔`/`▁` frame rows in `render_selected_block_borders`' `FocusedRail`
    /// arm, whose glyph colour is `PROGRESS_TRACK` and whose background is the
    /// panel's own fill (`render/arrangements/wide_hero.rs:422`,
    /// `render/components/widgets.rs:190-192`). Every other surface's frame
    /// glyphs are foreground text, not a surface border, so they say `None`
    /// rather than inventing one.
    pub(super) const fn border(self) -> Option<Color> {
        match self {
            Surface::LibraryPanel => Some(PROGRESS_TRACK),
            _ => None,
        }
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
        "main paints the library column's gutter and the wide Music browser \
         container as the app backdrop (`SURFACE_BACKDROP`) in every frame; \
         neither follows panel focus",
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
        "the queue panel's resting half has always painted the app backdrop",
    ),
    (
        Surface::MainContentBox,
        "the pane content box's resting half has always painted the app backdrop",
    ),
    (
        Surface::NonHeroSidebarBody,
        "the non-hero sidebar shell paints `SURFACE_SIDEBAR`, not the resting \
         content value",
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
