//! Resolves a `Surface` plus the frame's focus state to its `SurfaceColors`.
//!
//! This is the table's only resolver (design D2/D3): a production paint site
//! names a `Surface` and hands in the `FocusState` it was given. The focused
//! side is level-driven; the resting side is the row's own value (see
//! `surface_table::RESTING_DEVIATIONS`).

use super::surface::{FocusSource, Surface};
use super::surface_table::{row, RESTING_DEVIATIONS};
use super::*;
use crate::app::layout::FocusState;
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

/// The fill a surface takes in the Queue-only mode, where the right column —
/// the playback strip's normal home — is not on screen. This is the one
/// mode-driven appearance the table carries; every other row depends only on
/// panel focus. The value is today's: `shell_draw.rs` paints the Queue-only
/// strip with the chrome band in every state.
const fn queue_only_fill(surface: Surface) -> Option<Color> {
    match surface {
        Surface::PlaybackPanel | Surface::PlaybackRecess => Some(SURFACE_CHROME),
        _ => None,
    }
}

/// Resolves a surface's fill and border from the frame's one focus state
/// (design D2/D3).
///
/// This is the table's full entry point: every production paint site that
/// holds the frame's [`FocusState`] calls it. A shared painter handed only
/// its own column's focus (a mounted component's `Attribute::Focus`) calls
/// [`surface_colors_for_column_focus`] instead.
pub(in crate::app) fn surface_colors(surface: Surface, focus: &FocusState) -> SurfaceColors {
    if !focus.right_visible() {
        if let Some(fill) = queue_only_fill(surface) {
            return SurfaceColors {
                fill,
                border: surface.border(),
            };
        }
    }
    let focused = match row(surface).focus {
        FocusSource::Fixed => false,
        FocusSource::QueueColumn => focus.queue_column_focused(),
        FocusSource::LibraryColumn => focus.library_column_focused(),
    };
    build(surface, focused)
}

/// The colours a surface takes when the caller holds only that surface's own
/// column focus, collapsed to a bool.
///
/// The bool is *this surface's own column panel focus* — never a cursor or a
/// sub-panel bit; a `Fixed` row ignores it. The permitted callers are the
/// shared painters (`render/components/**`, `render/arrangements/**`) handed a
/// collapsed column bit, because a mounted component receives focus only as
/// `Attribute::Focus` (`design D1`). The row still decides the value, so the
/// colour is resolved in one place.
///
/// The one exclusion is the mode-driven playback strip
/// ([`Surface::PlaybackPanel`] / [`Surface::PlaybackRecess`] in Queue-only),
/// whose painters hold the shell's [`FocusState`] and must call
/// [`surface_colors`].
pub(in crate::app) fn surface_colors_for_column_focus(
    surface: Surface,
    own_column_focused: bool,
) -> SurfaceColors {
    build(surface, own_column_focused)
}

/// The row's fill and border given whether the surface's own column holds
/// panel focus. A `Fixed` row paints its resting value in every frame.
fn build(surface: Surface, own_column_focused: bool) -> SurfaceColors {
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
    let focused = own_column_focused && row.focus != FocusSource::Fixed;
    let mut colors = if focused {
        SurfaceColors::fill(if row.soft {
            SURFACE_ACCENT_SOFT
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
    use super::super::surface::Level;
    use super::*;
    use std::collections::HashSet;

    fn queue_focused() -> FocusState {
        FocusState::queue_focused_for_test()
    }

    fn library_focused() -> FocusState {
        FocusState::library_focused_for_test()
    }

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
            for state in [FocusState::default(), queue_focused(), library_focused()] {
                let colors = surface_colors(surface, &state);
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
    /// between two rows fail.
    fn pinned_fills(surface: Surface) -> (Color, Color) {
        match surface {
            Surface::QueueColumn => (SURFACE_FOCUSED, SURFACE_RESTING),
            Surface::LibraryColumn => (SURFACE_FOCUSED, SURFACE_BACKDROP),
            Surface::WideSplitGutter => (SURFACE_BACKDROP, SURFACE_BACKDROP),
            Surface::HeroPane => (SURFACE_FOCUSED, SURFACE_RESTING),
            Surface::SelectedRow => (SURFACE_BACKDROP, SURFACE_BACKDROP),
            Surface::SelectedRowOnQueueColumn => (SURFACE_FOCUSED, SURFACE_RESTING),
            Surface::SelectedRowOnLibraryPane => (SURFACE_FOCUSED, SURFACE_RESTING),
            Surface::ContextMenuSelectedRow => (ACCENT_ACTIVE, ACCENT_ACTIVE),
            Surface::LibraryPanel => (SURFACE_FOCUSED, SURFACE_RESTING),
            Surface::QueuePanel => (SURFACE_ACCENT_SOFT, SURFACE_BACKDROP),
            Surface::MainContentBox => (SURFACE_ACCENT_SOFT, SURFACE_BACKDROP),
            Surface::InlineHero => (SURFACE_FOCUSED, SURFACE_RESTING),
            Surface::PlaybackPanel => (SURFACE_FOCUSED, SURFACE_RESTING),
            Surface::SidebarBody => (SURFACE_RESTING, SURFACE_RESTING),
            Surface::NonHeroSidebarBody => (SURFACE_SIDEBAR, SURFACE_SIDEBAR),
            Surface::QueueCardVisualizer => (SURFACE_FOCUSED, SURFACE_RESTING),
            Surface::PlaybackRecess => (SURFACE_FOCUSED, SURFACE_RESTING),
            Surface::PlaybackBottomRow => (SURFACE_BACKDROP, SURFACE_BACKDROP),
            Surface::PlaybackStatusPill => (SURFACE_BACKDROP, SURFACE_BACKDROP),
            Surface::ArtworkPlaceholder => {
                (SURFACE_ARTWORK_PLACEHOLDER, SURFACE_ARTWORK_PLACEHOLDER)
            }
            Surface::ArtworkLoadingPlaceholder => (BORDER_UNFOCUSED, BORDER_UNFOCUSED),
            Surface::StatusBar => (SURFACE_CHROME, SURFACE_CHROME),
            Surface::StatusBarPill => (SURFACE_STATUS_PILL, SURFACE_STATUS_PILL),
            Surface::QueuePanelBand => (SURFACE_CHROME, SURFACE_CHROME),
            Surface::PillRow => (PILL_ROW_BG, PILL_ROW_BG),
            Surface::PillChip => (PILL_BG, PILL_BG),
            Surface::PillChipSelected => (PILL_SELECTED_BG, PILL_SELECTED_BG),
            Surface::PillRowGap => (SURFACE_BACKDROP, SURFACE_BACKDROP),
            Surface::SidebarBand => (SURFACE_CHROME, SURFACE_CHROME),
            Surface::NonHeroSidebarBand => (SURFACE_ITEM_FOCUSED, SURFACE_ITEM_FOCUSED),
            Surface::TabBar => (SURFACE_CHROME, SURFACE_CHROME),
            Surface::PopupFrame => (SURFACE_FOCUSED, SURFACE_FOCUSED),
            Surface::PopupDimBackdrop => (DIM_BACKDROP, DIM_BACKDROP),
        }
    }

    /// Every row resolves exactly its pinned focused fill (while its column
    /// holds focus) and its pinned resting fill (while it does not), and a
    /// fixed row resolves one value in both states.
    #[test]
    fn every_row_pins_its_focused_and_resting_fill() {
        for &surface in Surface::ALL {
            let (focused, resting) = pinned_fills(surface);
            let source = row(surface).focus;
            match source {
                FocusSource::Fixed => {
                    assert_eq!(
                        surface_colors(surface, &queue_focused()).fill,
                        resting,
                        "{surface:?} is fixed and must not follow focus"
                    );
                    assert_eq!(
                        surface_colors(surface, &library_focused()).fill,
                        resting,
                        "{surface:?} is fixed and must not follow focus"
                    );
                }
                FocusSource::QueueColumn => {
                    assert_eq!(
                        surface_colors(surface, &queue_focused()).fill,
                        focused,
                        "{surface:?} focused fill"
                    );
                    assert_eq!(
                        surface_colors(surface, &library_focused()).fill,
                        resting,
                        "{surface:?} resting fill"
                    );
                }
                FocusSource::LibraryColumn => {
                    assert_eq!(
                        surface_colors(surface, &library_focused()).fill,
                        focused,
                        "{surface:?} focused fill"
                    );
                    assert_eq!(
                        surface_colors(surface, &queue_focused()).fill,
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
            let focused = match r.focus {
                FocusSource::QueueColumn => surface_colors(surface, &queue_focused()).fill,
                FocusSource::LibraryColumn => surface_colors(surface, &library_focused()).fill,
                FocusSource::Fixed => unreachable!("filtered above"),
            };
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

        // The column and a content body differ in their resting value.
        let column = surface_colors(Surface::LibraryColumn, &queue_focused()).fill;
        let content = surface_colors(Surface::LibraryPanel, &queue_focused()).fill;
        assert_ne!(
            column, content,
            "the column and content-body resting values"
        );
        assert_eq!(column, SURFACE_BACKDROP);
        assert_eq!(content, SURFACE_RESTING);
    }

    /// The soft content body is the table's one declared variant: it takes the
    /// retired `SURFACE_ACCENT_SOFT` value while focused and differs from the
    /// default focused content body, so no soft surface is repainted.
    #[test]
    fn soft_content_body_matches_the_retired_role() {
        let soft_rows: Vec<Surface> = Surface::ALL
            .iter()
            .copied()
            .filter(|&s| row(s).soft)
            .collect();
        assert!(!soft_rows.is_empty(), "the table declares a soft variant");
        for surface in soft_rows {
            let state = match row(surface).focus {
                FocusSource::QueueColumn => queue_focused(),
                FocusSource::LibraryColumn => library_focused(),
                FocusSource::Fixed => unreachable!("a soft row is focus-driven"),
            };
            assert_eq!(
                surface_colors(surface, &state).fill,
                SURFACE_ACCENT_SOFT,
                "{surface:?} focused soft fill"
            );
        }
        // The soft variant is not the default focused content body.
        assert_ne!(SURFACE_ACCENT_SOFT, SURFACE_FOCUSED);
        assert_eq!(row(Surface::QueuePanel).resting, SURFACE_BACKDROP);
    }

    /// The Queue-only playback strip is the table's one mode-driven
    /// appearance: with no right column, the now-playing panel and its
    /// content rows paint the chrome band in both focus states.
    #[test]
    fn queue_only_playback_strip_paints_the_chrome_band() {
        let queue_only_focused = FocusState::queue_only_for_test(false);
        let queue_only_library = FocusState::queue_only_for_test(true);
        for surface in [Surface::PlaybackPanel, Surface::PlaybackRecess] {
            for state in [&queue_only_focused, &queue_only_library] {
                assert_eq!(
                    surface_colors(surface, state).fill,
                    SURFACE_CHROME,
                    "{surface:?} in Queue-only"
                );
            }
            assert_eq!(
                surface_colors(surface, &queue_focused()).fill,
                SURFACE_FOCUSED,
                "{surface:?} with the right column visible"
            );
        }
    }
}
