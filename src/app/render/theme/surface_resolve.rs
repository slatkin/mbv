//! Resolves a `Surface` plus the call site's own focus bit to its
//! `SurfaceColors`.
//!
//! This is the table's only resolver (design D1/D3): a production paint site
//! names a `Surface` and hands in the focus input it already computes. The
//! focused side is level-driven; the resting side is the row's own value (see
//! `surface_table::RESTING_DEVIATIONS`).
//!
//! The table governs colour *identity*, not the bit's provenance: the same
//! bool may be a mounted component's focus, a pane's `held` bit, a panel focus
//! bool, or (for the few cursor-driven sites) the site's cursor predicate,
//! exactly as the site computes it today. A `Fixed` row ignores the bool and
//! pins focused == resting (design D3(a)); no row branches on anything else.

#![cfg_attr(not(test), allow(dead_code))]

use super::surface::{FocusSource, Surface};
use super::surface_table::{row, RESTING_DEVIATIONS};
use super::*;
use ratatui::style::Color;

/// The resolved colour of one rendered surface for one frame.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::app) struct SurfaceColors {
    pub(in crate::app) fill: Color,
    pub(in crate::app) border: Option<Color>,
}

impl SurfaceColors {
    const fn fill(fill: Color) -> Self {
        Self { fill, border: None }
    }
}

/// Resolves a surface's fill and border from the call site's own focus bit
/// (design D1/D2/D3).
pub(in crate::app) fn surface_colors(surface: Surface, focused: bool) -> SurfaceColors {
    let row = row(surface);
    // The table is closed: `row` matches every variant, and every row's
    // resting value is its level's default or an explicitly declared
    // deviation. This holds it at runtime in debug builds.
    debug_assert!(
        surface.level().resting_default().is_none()
            || surface.level().resting_default() == Some(row.resting)
            || RESTING_DEVIATIONS
                .iter()
                .any(|(declared, _)| *declared == surface),
        "{surface:?} rests at a value that is neither its level default nor a declared deviation"
    );
    let follow_focus = focused && row.focus != FocusSource::Fixed;
    let mut colors = if follow_focus {
        SurfaceColors::fill(if row.soft {
            primitives::SOFT_CONTENT_BODY_BG
        } else {
            row.level.focused_fill()
        })
    } else {
        SurfaceColors::fill(row.resting)
    };
    colors.border = surface.border();
    colors
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::super::surface::Level;
    use super::*;

    /// Every variant is placed, `ALL` has no duplicates, and every one
    /// resolves. `ALL` is generated from the same macro variant list as the
    /// enum, so it is complete by construction; the exhaustive matches in
    /// `row`/`border` fail to compile if a new variant is not placed.
    #[test]
    fn all_lists_every_surface_and_every_surface_resolves() {
        let mut seen = HashSet::new();
        for &surface in Surface::ALL {
            assert!(
                seen.insert(surface),
                "duplicate surface in ALL: {surface:?}"
            );
            for focused in [false, true] {
                let colors = surface_colors(surface, focused);
                assert_eq!(
                    colors.border,
                    surface.border(),
                    "{surface:?} carries its declared border"
                );
            }
        }
        assert_eq!(seen.len(), Surface::ALL.len());
    }

    /// The focused and resting fill every row must resolve, pinned to the
    /// declared role constant it equals. Pinning (rather than "is some
    /// declared role") is what makes a changed row fail and a role swapped
    /// between two rows fail. A fixed row pins the same value twice.
    fn pinned_fills(surface: Surface) -> (Color, Color) {
        match surface {
            Surface::QueueColumn => (SURFACE_FOCUSED, SURFACE_RESTING),
            Surface::LibraryColumn => (SURFACE_BACKDROP, SURFACE_BACKDROP),
            Surface::WideSplitGutter => (SURFACE_BACKDROP, SURFACE_BACKDROP),
            Surface::HeroPane => (SURFACE_FOCUSED, SURFACE_RESTING),
            Surface::SelectedRow => (SURFACE_BACKDROP, SURFACE_BACKDROP),
            Surface::SelectedRowOnQueueColumn => (SURFACE_FOCUSED, SURFACE_RESTING),
            Surface::SelectedRowOnLibraryPane => (SURFACE_FOCUSED, SURFACE_RESTING),
            Surface::ContextMenuSelectedRow => (ACCENT_ACTIVE, ACCENT_ACTIVE),
            Surface::LibraryPanel => (SURFACE_FOCUSED, SURFACE_RESTING),
            Surface::QueuePanel => (primitives::SOFT_CONTENT_BODY_BG, SURFACE_BACKDROP),
            Surface::MainContentBox => (primitives::SOFT_CONTENT_BODY_BG, SURFACE_BACKDROP),
            Surface::InlineHero => (SURFACE_FOCUSED, SURFACE_RESTING),
            Surface::PlaybackPanel => (SURFACE_FOCUSED, SURFACE_RESTING),
            Surface::QueueOnlyPlaybackPanel => (SURFACE_CHROME, SURFACE_CHROME),
            Surface::SidebarBody => (SURFACE_RESTING, SURFACE_RESTING),
            Surface::NonHeroSidebarBody => (SURFACE_SIDEBAR, SURFACE_SIDEBAR),
            Surface::QueueCardVisualizer => (SURFACE_FOCUSED, SURFACE_RESTING),
            Surface::PlaybackRecess => (SURFACE_FOCUSED, SURFACE_RESTING),
            Surface::PlaybackBottomRow => (SURFACE_BACKDROP, SURFACE_BACKDROP),
            Surface::PlaybackStatusPill => (SURFACE_BACKDROP, SURFACE_BACKDROP),
            Surface::ArtworkPlaceholder => (SURFACE_BACKDROP, SURFACE_BACKDROP),
            Surface::ArtworkLoadingPlaceholder => (
                primitives::ARTWORK_LOADING_PLACEHOLDER,
                primitives::ARTWORK_LOADING_PLACEHOLDER,
            ),
            Surface::StatusBar => (SURFACE_CHROME, SURFACE_CHROME),
            Surface::StatusBarPill => (SURFACE_CHROME, SURFACE_CHROME),
            Surface::QueuePanelBand => (SURFACE_CHROME, SURFACE_CHROME),
            Surface::PillRow => (PILL_ROW_BG, PILL_ROW_BG),
            Surface::PillChip => (PILL_BG, PILL_BG),
            Surface::PillChipSelected => (PILL_SELECTED_BG, PILL_SELECTED_BG),
            Surface::QueueScopePillSelected => (ACCENT, ACCENT),
            Surface::PillRowGap => (SURFACE_BACKDROP, SURFACE_BACKDROP),
            Surface::SidebarBand => (SURFACE_CHROME, SURFACE_CHROME),
            Surface::NonHeroSidebarBand => (SURFACE_ITEM_FOCUSED, SURFACE_ITEM_FOCUSED),
            Surface::TabBar => (SURFACE_CHROME, SURFACE_CHROME),
            Surface::PopupFrame => (SURFACE_FOCUSED, SURFACE_FOCUSED),
            Surface::PopupDimBackdrop => (Color::Black, Color::Black),
        }
    }

    /// Every row resolves exactly its pinned focused fill (with the call
    /// site's bit true) and its pinned resting fill (with the bit false), and
    /// a fixed row resolves one value in both states.
    #[test]
    fn every_row_pins_its_focused_and_resting_fill() {
        for &surface in Surface::ALL {
            let (focused, resting) = pinned_fills(surface);
            match row(surface).focus {
                FocusSource::Fixed => {
                    assert_eq!(
                        surface_colors(surface, true).fill,
                        resting,
                        "{surface:?} is fixed and must not follow focus"
                    );
                    assert_eq!(
                        surface_colors(surface, false).fill,
                        resting,
                        "{surface:?} is fixed and must not follow focus"
                    );
                }
                FocusSource::QueueColumn | FocusSource::LibraryColumn => {
                    assert_eq!(
                        surface_colors(surface, true).fill,
                        focused,
                        "{surface:?} focused fill"
                    );
                    assert_eq!(
                        surface_colors(surface, false).fill,
                        resting,
                        "{surface:?} resting fill"
                    );
                }
            }
        }
    }

    /// Surfaces of one level share that level's focused fill, and the levels
    /// differ where the map says they differ.
    #[test]
    fn levels_share_the_focused_fill_and_differ_in_resting() {
        for &surface in Surface::ALL {
            let r = row(surface);
            if r.focus == FocusSource::Fixed || r.soft {
                continue;
            }
            let focused = surface_colors(surface, true).fill;
            match r.level {
                Level::ColumnPane => assert_eq!(
                    focused, SURFACE_FOCUSED,
                    "{surface:?} shares the column/pane focused fill"
                ),
                Level::ContentBody => assert_eq!(
                    focused, SURFACE_FOCUSED,
                    "{surface:?} shares the content-body focused fill"
                ),
                Level::Recess => assert_eq!(
                    focused, SURFACE_FOCUSED,
                    "{surface:?} shares the recess focused fill"
                ),
                Level::ChromeBand | Level::Popup => {
                    panic!(
                        "{surface:?}: only column/pane, content body and recess are focus-driven"
                    )
                }
            }
        }

        // A declared column deviation (the library column's backdrop) differs
        // from a content body's resting value. The two levels' declared
        // *defaults* coincide, so this pins the visible difference.
        let column = surface_colors(Surface::LibraryColumn, false).fill;
        let content = surface_colors(Surface::LibraryPanel, false).fill;
        assert_ne!(
            column, content,
            "the column and content-body resting values"
        );
        assert_eq!(column, SURFACE_BACKDROP);
        assert_eq!(content, SURFACE_RESTING);
    }

    /// The soft content body is the table's one declared variant: it takes the
    /// `#48584e` value while focused and differs from the default focused
    /// content body, so no soft surface is repainted.
    #[test]
    fn soft_content_body_matches_its_declared_variant() {
        let soft_rows: Vec<Surface> = Surface::ALL
            .iter()
            .copied()
            .filter(|&s| row(s).soft)
            .collect();
        assert!(!soft_rows.is_empty(), "the table declares a soft variant");
        for surface in soft_rows {
            assert_eq!(
                surface_colors(surface, true).fill,
                primitives::SOFT_CONTENT_BODY_BG,
                "{surface:?} focused soft fill"
            );
        }
        // The soft variant is not the default focused content body.
        assert_ne!(primitives::SOFT_CONTENT_BODY_BG, SURFACE_FOCUSED);
        assert_eq!(row(Surface::QueuePanel).resting, SURFACE_BACKDROP);
    }
}
