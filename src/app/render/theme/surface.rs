//! The closed surface table (`unify-surface-colour` design D2, landed by row
//! 4.1).
//!
//! One row per rendered-surface identity: its nesting level, which column's
//! panel focus (if any) drives its focused appearance, whether it takes the
//! declared soft content-body variant, and the value it paints while resting.
//! [`surface_colors`] is the single mapping a production paint site consults:
//! it names a [`Surface`] and hands in the frame's
//! [`FocusState`](crate::app::layout::FocusState), and the table resolves the
//! fill (plus a border, where the surface paints one).
//!
//! Level-driven focused appearance, declared resting deviations (D2): a row's
//! focused fill is its level's focused value, or the soft content-body
//! variant, so one edit to a level's focused appearance reaches every row of
//! that level. The resting fill is today's value; where it differs from the
//! level's resting default the row is on [`RESTING_DEVIATIONS`] with its
//! reason, so a value drift fails a test rather than hiding.
//!
//! Transitional: the table has no production callers until change row 4.2
//! migrates the screens onto it. The `allow(dead_code)` below keeps
//! `cargo check -p mbv` warning-free until then; row 4.2 removes it.
#![cfg_attr(not(test), allow(dead_code))]

use super::*;
use crate::app::layout::FocusState;
use ratatui::style::Color;

/// The nesting level of a rendered surface (design D2).
///
/// A level is what an edit is expressed against: one change to a level's
/// focused appearance reaches every row of that level.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub(in crate::app) enum Level {
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
    /// `Recess` shares it too because its two focus-driven rows (the
    /// now-playing recess and the queue card visualizer) read as content.
    /// `ChromeBand` and `Popup` have no focus-driven row yet; their value
    /// records the level's intent for one.
    pub(in crate::app) const fn focused_fill(self) -> Color {
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
    pub(in crate::app) const fn resting_default(self) -> Option<Color> {
        match self {
            Level::ColumnPane | Level::ContentBody => Some(SURFACE_RESTING),
            Level::Recess | Level::ChromeBand | Level::Popup => None,
        }
    }
}

/// Declares the closed `Surface` set and the enumerable [`Surface::ALL`] list
/// from one variant list, so a new variant cannot be added without appearing
/// in `ALL` (and `level`/`surface_colors` fail to compile until it is placed).
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
    SelectedRowOnQueuePanel,
    SelectedRowOnMainContentBox,
    ContextMenuSelectedRow,
    // --- content body: a focusable content region ---
    LibraryPanel,
    QueuePanel,
    MainContentBox,
    InlineHero,
    PlaybackPanel,
    SidebarBody,
    PlainSidebarBody,
    // --- recess: a non-focusable inset inside a content body ---
    PlaybackRecess,
    PlaybackBottomRow,
    PlaybackStatusPill,
    QueueCardVisualizer,
    ArtworkPlaceholder,
    ArtworkLoadingPlaceholder,
    // --- chrome band: non-focusable structural chrome ---
    StatusBar,
    StatusBarPill,
    QueuePanelBand,
    PillRow,
    PillChip,
    PillChipSelected,
    PillRowGap,
    SidebarBand,
    PlainSidebarBand,
    TabBar,
    // --- popup: an overlay frame and its dim backdrop ---
    PopupFrame,
    PopupDimBackdrop,
);

/// Which column's panel focus drives a row's focused appearance.
///
/// `Fixed` rows paint one value in every frame: they are recesses, bands and
/// popups that never follow a panel (and the selected-row punch-through,
/// whose colour is the backdrop beneath it).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FocusSource {
    Fixed,
    QueueColumn,
    LibraryColumn,
}

/// One row of the table: everything a surface's colour depends on.
#[derive(Clone, Copy, Debug)]
struct Row {
    level: Level,
    focus: FocusSource,
    /// Takes the soft content-body variant (`SURFACE_ACCENT_SOFT`) while
    /// focused, instead of the level's focused fill.
    soft: bool,
    /// The value the row paints while resting: today's value for that
    /// surface.
    resting: Color,
}

const fn row(surface: Surface) -> Row {
    match surface {
        // --- column/pane ---
        //
        // The queue column's gutter and the queue boundary strip resolve
        // `PanelFocus::Queue`; its resting value is the level default. The
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
        // The library punch-through: `list_selected_row_bg()` shows the
        // backdrop beneath the list panel, never the panel's focus green.
        Surface::SelectedRow => Row {
            level: Level::ColumnPane,
            focus: FocusSource::Fixed,
            soft: false,
            resting: SURFACE_BACKDROP,
        },
        // The queue list's selected row punches through to its owning pane,
        // which resolves `PanelFocus::Queue`.
        Surface::SelectedRowOnQueuePanel => Row {
            level: Level::ColumnPane,
            focus: FocusSource::QueueColumn,
            soft: false,
            resting: SURFACE_RESTING,
        },
        // TV's episode list and Music's track list punch through to their
        // owning pane inside the library column.
        Surface::SelectedRowOnMainContentBox => Row {
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
        // The plain (non-hero) sidebar shell is its own appearance.
        Surface::PlainSidebarBody => Row {
            level: Level::ContentBody,
            focus: FocusSource::Fixed,
            soft: false,
            resting: SURFACE_SIDEBAR,
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
        // The queue card's now-playing content resolves `PanelFocus::Queue`
        // like a content body, not like an inset inside the panel.
        Surface::QueueCardVisualizer => Row {
            level: Level::Recess,
            focus: FocusSource::QueueColumn,
            soft: false,
            resting: SURFACE_RESTING,
        },
        Surface::ArtworkPlaceholder => Row {
            level: Level::Recess,
            focus: FocusSource::Fixed,
            soft: false,
            resting: SURFACE_ARTWORK_PLACEHOLDER,
        },
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
            resting: SURFACE_STATUS_PILL,
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
        Surface::PlainSidebarBand => Row {
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
        // The dim backdrop paints no fill of its own: it halves every existing
        // cell (see `super::super::components::backdrop::dim_backdrop`), so
        // its row resolves to `Color::Reset` ("no explicit fill") rather than
        // inventing one.
        Surface::PopupDimBackdrop => Row {
            level: Level::Popup,
            focus: FocusSource::Fixed,
            soft: false,
            resting: Color::Reset,
        },
    }
}

impl Surface {
    /// The surface's nesting level, stated once (design D2; the delta spec's
    /// "Each surface entry SHALL state its nesting level").
    pub(in crate::app) const fn level(self) -> Level {
        row(self).level
    }

    /// The border colour this surface frames itself with, when it paints one.
    ///
    /// Only the Wide hero library/rail panel paints a surface border: its
    /// `▔`/`▁` frame rows in `render_selected_block_borders`' `FocusedRail`
    /// arm. Every other surface's frame glyphs are foreground text, not a
    /// surface border, so they say `None` rather than inventing one.
    pub(in crate::app) const fn border(self) -> Option<Color> {
        match self {
            Surface::LibraryPanel => Some(PROGRESS_TRACK),
            _ => None,
        }
    }
}

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
/// (design D2/D3). The focused side is level-driven; the resting side is the
/// row's own value (see [`RESTING_DEVIATIONS`]).
pub(in crate::app) fn surface_colors(surface: Surface, focus: &FocusState) -> SurfaceColors {
    if !focus.right_visible() {
        if let Some(fill) = queue_only_fill(surface) {
            return SurfaceColors {
                fill,
                border: surface.border(),
            };
        }
    }
    let row = row(surface);
    let focused = match row.focus {
        FocusSource::Fixed => false,
        FocusSource::QueueColumn => focus.queue_column_focused(),
        FocusSource::LibraryColumn => focus.library_column_focused(),
    };
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

/// The rows whose resting value is not their level's resting default, with
/// the reason.
///
/// Declared next to the table so the deviation is visible and removable one at
/// a time (design D2), and so a resting-value drift fails
/// `resting_values_are_default_or_declared` rather than hiding.
pub(in crate::app) const RESTING_DEVIATIONS: &[(Surface, &str)] = &[
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
        Surface::PlainSidebarBody,
        "the plain (non-hero) sidebar shell paints `SURFACE_SIDEBAR`, not the \
         resting content value",
    ),
];

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    /// One representative focus state per column, plus the default.
    fn focus_states() -> [FocusState; 3] {
        [
            FocusState::default(),
            FocusState::queue_focused_for_test(),
            FocusState::library_focused_for_test(),
        ]
    }

    fn queue_focused() -> FocusState {
        FocusState::queue_focused_for_test()
    }

    fn library_focused() -> FocusState {
        FocusState::library_focused_for_test()
    }

    /// A frame whose focus state reports neither column focused.
    fn not_focused() -> FocusState {
        FocusState::default()
    }

    fn focused_state(source: FocusSource) -> FocusState {
        match source {
            FocusSource::QueueColumn => queue_focused(),
            FocusSource::LibraryColumn => library_focused(),
            FocusSource::Fixed => not_focused(),
        }
    }

    fn deviation_for(surface: Surface) -> Option<&'static str> {
        RESTING_DEVIATIONS
            .iter()
            .find(|(s, _)| *s == surface)
            .map(|(_, reason)| *reason)
    }

    /// Every variant is placed, `ALL` has no duplicates, and every one
    /// resolves in every focus state. `ALL` is generated from the same macro
    /// variant list as the enum, so it is complete by construction; the
    /// exhaustive matches in `row`/`border` fail to compile if a new variant
    /// is not placed.
    #[test]
    fn all_lists_every_surface_and_every_surface_resolves() {
        let mut seen = HashSet::new();
        for &surface in Surface::ALL {
            assert!(
                seen.insert(surface),
                "duplicate surface in ALL: {surface:?}"
            );
            for state in focus_states() {
                let colors = surface_colors(surface, &state);
                assert!(
                    matches!(
                        surface.level(),
                        Level::ColumnPane
                            | Level::ContentBody
                            | Level::Recess
                            | Level::ChromeBand
                            | Level::Popup
                    ),
                    "{surface:?} has a level"
                );
                assert_eq!(
                    colors.border,
                    surface.border(),
                    "{surface:?} carries its declared border"
                );
            }
        }
        assert_eq!(seen.len(), Surface::ALL.len());
    }

    /// The focused side is level-driven: a focus-driven row takes its level's
    /// focused fill, or the declared soft content-body variant, while its
    /// column holds focus. A `Fixed` row paints the same value in both states.
    #[test]
    fn focused_fill_is_level_driven() {
        assert_eq!(
            surface_colors(Surface::QueueColumn, &not_focused()).fill,
            SURFACE_RESTING,
            "an unfocused queue column rests at the level default"
        );

        for &surface in Surface::ALL {
            let r = row(surface);
            match r.focus {
                FocusSource::Fixed => {
                    assert_eq!(
                        surface_colors(surface, &queue_focused()).fill,
                        r.resting,
                        "{surface:?} is fixed and must not follow focus"
                    );
                    assert_eq!(
                        surface_colors(surface, &library_focused()).fill,
                        r.resting,
                        "{surface:?} is fixed and must not follow focus"
                    );
                }
                source => {
                    let focused = surface_colors(surface, &focused_state(source)).fill;
                    let expected = if r.soft {
                        SURFACE_ACCENT_SOFT
                    } else {
                        r.level.focused_fill()
                    };
                    assert_eq!(focused, expected, "{surface:?} focused fill");
                }
            }
        }
    }

    /// Every row's resting value is its level's resting default or is on the
    /// declared deviation list, with a reason. A future drift of one row's
    /// resting value fails here instead of hiding.
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
            assert_eq!(
                surface_colors(surface, &focused_state(row(surface).focus)).fill,
                SURFACE_ACCENT_SOFT,
                "{surface:?} focused soft fill"
            );
        }
        // The soft variant is not the default focused content body.
        assert_ne!(SURFACE_ACCENT_SOFT, SURFACE_FOCUSED);
        assert_eq!(row(Surface::QueuePanel).resting, SURFACE_BACKDROP);
    }

    /// Surfaces of one level share that level's focused fill, and the levels
    /// differ where the map says they differ.
    #[test]
    fn levels_share_the_focused_fill_and_differ_in_resting() {
        let focused_fill =
            |surface: Surface| surface_colors(surface, &focused_state(row(surface).focus)).fill;

        // Every focus-driven column/pane row shares the level's focused fill.
        for &surface in Surface::ALL {
            if surface.level() == Level::ColumnPane && row(surface).focus != FocusSource::Fixed {
                assert_eq!(
                    focused_fill(surface),
                    Level::ColumnPane.focused_fill(),
                    "{surface:?} column/pane focused fill"
                );
            }
            if surface.level() == Level::ContentBody
                && row(surface).focus != FocusSource::Fixed
                && !row(surface).soft
            {
                assert_eq!(
                    focused_fill(surface),
                    Level::ContentBody.focused_fill(),
                    "{surface:?} content-body focused fill"
                );
            }
        }

        // The column and a content body differ in their resting value.
        let column = surface_colors(Surface::LibraryColumn, &not_focused()).fill;
        let content = surface_colors(Surface::LibraryPanel, &not_focused()).fill;
        assert_ne!(
            column, content,
            "the column and content-body resting values"
        );
        assert_eq!(column, SURFACE_BACKDROP);
        assert_eq!(content, SURFACE_RESTING);

        // The soft content body differs from the default focused content body.
        let soft = focused_fill(Surface::QueuePanel);
        let default = focused_fill(Surface::LibraryPanel);
        assert_ne!(soft, default);
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

    /// `surface_colors` stays complete: every resolved fill is a declared
    /// theme role (or the dim backdrop's explicit "no fill"), never an ad-hoc
    /// colour chosen outside `primitives`/the roles.
    #[test]
    fn every_resolved_fill_is_a_declared_role() {
        let allowed = [
            SURFACE_BACKDROP,
            SURFACE_CHROME,
            SURFACE_FOCUSED,
            SURFACE_RESTING,
            SURFACE_PLAYBACK,
            SURFACE_ACCENT_SOFT,
            SURFACE_ITEM_FOCUSED,
            SURFACE_STATUS_PILL,
            SURFACE_SIDEBAR,
            SURFACE_ARTWORK_PLACEHOLDER,
            BORDER_UNFOCUSED,
            PILL_ROW_BG,
            PILL_BG,
            PILL_SELECTED_BG,
            ACCENT_ACTIVE,
            Color::Reset,
        ];
        for &surface in Surface::ALL {
            for state in focus_states() {
                let fill = surface_colors(surface, &state).fill;
                assert!(
                    allowed.contains(&fill),
                    "{surface:?} resolved {fill:?}, which is not a declared role"
                );
            }
        }
    }
}
