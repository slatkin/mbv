//! The surface table's rows: each `Surface`'s level, focus source, soft
//! variant and resting value, plus the declared resting deviations.
//!
//! Kept apart from the identity set (`surface.rs`) and the resolver
//! (`surface_resolve.rs`) so the table is readable in one place and the enum
//! stays complete by construction.

use super::surface::{FocusSource, Level, Row, Surface};
use super::*;

/// The row for one surface: its level, which column's focus (if any) drives
/// its focused appearance, whether it takes the soft content-body variant, and
/// the value it paints while resting.
pub(super) const fn row(surface: Surface) -> Row {
    match surface {
        // --- column/pane ---
        //
        // The queue column's gutter and the queue boundary strip resolve the
        // queue column's focus; its resting value is the level default. The
        // library column's resting value is the app backdrop the shell always
        // painted (see `RESTING_DEVIATIONS`).
        Surface::QueueColumn => Row {
            level: Level::ColumnPane,
            focus: FocusSource::QueueColumn,
            soft: false,
            resting: SURFACE_RESTING,
        },
        Surface::LibraryColumn => Row {
            level: Level::ColumnPane,
            focus: FocusSource::LibraryColumn,
            soft: false,
            resting: SURFACE_BACKDROP,
        },
        // The wide hero split gap paints the backdrop in every frame today.
        Surface::WideSplitGutter => Row {
            level: Level::ColumnPane,
            focus: FocusSource::Fixed,
            soft: false,
            resting: SURFACE_BACKDROP,
        },
        // `CONTEXT.md` defines the Hero pane as the `#333c43` resting fill of
        // Wide hero's right pane, so this is the level default, not a
        // deviation.
        Surface::HeroPane => Row {
            level: Level::ColumnPane,
            focus: FocusSource::LibraryColumn,
            soft: false,
            resting: SURFACE_RESTING,
        },
        // The library punch-through: the backdrop beneath the list panel
        // shows through, never the panel's focus green.
        Surface::SelectedRow => Row {
            level: Level::ColumnPane,
            focus: FocusSource::Fixed,
            soft: false,
            resting: SURFACE_BACKDROP,
        },
        // The queue list's selected row is a hole in the queue column: it
        // shows the column's own fill, resolved with the queue column's focus.
        Surface::SelectedRowOnQueueColumn => Row {
            level: Level::ColumnPane,
            focus: FocusSource::QueueColumn,
            soft: false,
            resting: SURFACE_RESTING,
        },
        // TV's episode list and Music's track list are holes in the library
        // pane: they show the pane's fill, resolved with the library column's
        // focus.
        Surface::SelectedRowOnLibraryPane => Row {
            level: Level::ColumnPane,
            focus: FocusSource::LibraryColumn,
            soft: false,
            resting: SURFACE_RESTING,
        },
        // A popup's context-menu row is the one selected row at this level
        // with a different value; declared rather than silently repainted.
        Surface::ContextMenuSelectedRow => Row {
            level: Level::ColumnPane,
            focus: FocusSource::Fixed,
            soft: false,
            resting: ACCENT_ACTIVE,
        },
        // --- content body ---
        Surface::LibraryPanel => Row {
            level: Level::ContentBody,
            focus: FocusSource::LibraryColumn,
            soft: false,
            resting: SURFACE_RESTING,
        },
        Surface::QueuePanel => Row {
            level: Level::ContentBody,
            focus: FocusSource::QueueColumn,
            soft: true,
            resting: SURFACE_BACKDROP,
        },
        // A pane's content box: TV's episode box and Music's track box are the
        // soft variant today; row 4.6 makes TV's overview box and the
        // inline-hero box follow their pane's focus as the same variant.
        Surface::MainContentBox => Row {
            level: Level::ContentBody,
            focus: FocusSource::LibraryColumn,
            soft: true,
            resting: SURFACE_BACKDROP,
        },
        Surface::InlineHero => Row {
            level: Level::ContentBody,
            focus: FocusSource::LibraryColumn,
            soft: false,
            resting: SURFACE_RESTING,
        },
        Surface::PlaybackPanel => Row {
            level: Level::ContentBody,
            focus: FocusSource::QueueColumn,
            soft: false,
            resting: SURFACE_RESTING,
        },
        // The expanded (F1-F4) sidebar body paints the resting content value.
        Surface::SidebarBody => Row {
            level: Level::ContentBody,
            focus: FocusSource::Fixed,
            soft: false,
            resting: SURFACE_RESTING,
        },
        // The non-hero sidebar shell is its own appearance.
        Surface::NonHeroSidebarBody => Row {
            level: Level::ContentBody,
            focus: FocusSource::Fixed,
            soft: false,
            resting: SURFACE_SIDEBAR,
        },
        // The queue card's now-playing content is the queue column's content,
        // so it resolves the content-body pair with the queue column's focus
        // even though it sits inside the card.
        Surface::QueueCardVisualizer => Row {
            level: Level::ContentBody,
            focus: FocusSource::QueueColumn,
            soft: false,
            resting: SURFACE_RESTING,
        },
        // --- recess ---
        // The now-playing panel's own content rows follow the panel's fill.
        Surface::PlaybackRecess => Row {
            level: Level::Recess,
            focus: FocusSource::QueueColumn,
            soft: false,
            resting: SURFACE_RESTING,
        },
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
        Surface::ArtworkLoadingPlaceholder => Row {
            level: Level::Recess,
            focus: FocusSource::Fixed,
            soft: false,
            resting: primitives::ARTWORK_LOADING_PLACEHOLDER,
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
        // (`CONTEXT.md`, "Direct remote control") rather than the library
        // pill's selected blue, so it carries its own row.
        Surface::QueueScopePillSelected => Row {
            level: Level::ChromeBand,
            focus: FocusSource::Fixed,
            soft: false,
            resting: ACCENT,
        },
        // Home's pill-bar spacer band paints the backdrop today.
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
        // Every modal caller passes `SURFACE_FOCUSED` as its frame background.
        Surface::PopupFrame => Row {
            level: Level::Popup,
            focus: FocusSource::Fixed,
            soft: false,
            resting: SURFACE_FOCUSED,
        },
        // The dim backdrop paints no fill of its own: it blends every
        // existing cell halfway toward its own fill (see
        // `super::super::components::backdrop::dim_backdrop`), so its row is
        // that blend base rather than a rect fill.
        Surface::PopupDimBackdrop => Row {
            level: Level::Popup,
            focus: FocusSource::Fixed,
            soft: false,
            resting: DIM_BACKDROP,
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
    /// panel's own fill. Every other surface's frame glyphs are foreground
    /// text, not a surface border, so they say `None` rather than inventing
    /// one.
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
        "the app backdrop (`SURFACE_BACKDROP`) the shell always painted; a resting \
         panel body needs contrast against its column",
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
