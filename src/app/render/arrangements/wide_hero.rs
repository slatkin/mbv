//! Wide hero arrangement: geometry, borders, recessed content blocks,
//! and wrapped-text rendering for the two-pane layout shared by Music,
//! Home, and future screens (design.md decisions 4–6).

use super::padded_rect;
use crate::app::palette;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};
use ratatui::Frame;
use textwrap::wrap;

/// Minimum outer content-area height for the Wide hero arrangement's
/// two-pane split.
pub(in crate::app::render) const WIDE_HERO_MIN_AREA_HEIGHT: u16 = 6;
/// Minimum width either Wide hero pane may shrink to (decision 5's
/// minimum pane width).
pub(in crate::app) const WIDE_HERO_MIN_PANE_WIDTH: u16 = 40;
/// Empty columns separating the Wide hero arrangement's two panes. The
/// draggable boundary IS this gap; the split's override clamp reserves it.
pub(in crate::app) const WIDE_HERO_PANE_GAP: u16 = 2;
/// Height of the pill row at the top of the Wide hero arrangement's left
/// (list) pane.
const WIDE_HERO_PILLS_ROW_HEIGHT: u16 = 1;
/// Blank rows below the pill row before the list starts.
const WIDE_HERO_PILLS_GAP_ROWS: u16 = 1;
/// Height of the full-width Selector band the Wide Library panel reserves
/// above both of its panes: the pill row plus its spacer row. One definition
/// of the band height (design D7); `arrangements/library.rs` carves it.
pub(in crate::app) const WIDE_HERO_PILL_BAND_HEIGHT: u16 =
    WIDE_HERO_PILLS_ROW_HEIGHT + WIDE_HERO_PILLS_GAP_ROWS;

/// Symmetric interior padding shared by every Wide hero surface's panes
/// (hero content, list panel, recessed boxes). One definition; surfaces that
/// previously carried their own `PANE_PAD_X`/`PANE_PAD_Y` (or `HOME_HERO_PAD_*`)
/// copy now import these.
pub(in crate::app) const PANE_PAD_X: u16 = 2;
pub(in crate::app) const PANE_PAD_Y: u16 = 1;

/// Resolves the only shared responsive decision for hero-bearing browsers and
/// returns the pane geometry when the wide presentation fits. Callers provide
/// content; they do not own a breakpoint or a height threshold.
///
/// This primitive owns the one-row status-bar reserve: both returned panes
/// already exclude the terminal's bottom status row, so every Wide hero
/// screen bottoms out exactly one row above it. Callers must not re-derive
/// that reserve (no extra `saturating_sub(1)` on the panes, no `bottom_pad`
/// on `wide_hero_browser_pane`) — doing so double-subtracts and shifts the
/// screen a second row.
///
/// Geometry is returned by semantic role: `hero` is the larger (~60%)
/// hero/workspace pane on the left, `browser` is the ~40% list pane on the
/// right.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::app) struct WideHeroPanes {
    pub browser: Rect,
    pub hero: Rect,
}

/// Whether `area` fits the Wide hero two-pane presentation (the shared
/// breakpoint predicate). Callers evaluate it on the **uncarved** panel area:
/// the Wide Library panel then carves its full-width Selector band off and
/// splits the reduced content area, so the decision cannot shift by the band
/// height (design D1).
pub(in crate::app) fn wide_hero_fits(area: Rect) -> bool {
    area.width >= crate::app::TWO_COLUMN_THRESHOLD
        && area.height.saturating_sub(1) >= WIDE_HERO_MIN_AREA_HEIGHT
}

/// Produces the Wide hero pane geometry for `content_area` by semantic role.
///
/// Split-only, deliberately: the caller has already decided the breakpoint
/// with [`wide_hero_fits`] on the uncarved area and may be passing the
/// band-reduced content area here, so re-running the fits check would reject
/// short-but-valid content areas and strand a stale frame (design D1/D2).
pub(in crate::app::render) fn wide_hero_presentation(
    content_area: Rect,
    override_width: Option<u16>,
) -> WideHeroPanes {
    // The panes tile `content_area` exactly. The status bar's own band is
    // already excluded by `chrome_geometry` before the Library panel sees its
    // placement, so reserving a second row here only left a stray blank row
    // under the panel's bottom spacer.
    let (browser, hero) = wide_hero_split(content_area, override_width);
    WideHeroPanes { browser, hero }
}

#[cfg(test)]
#[allow(clippy::items_after_test_module)]
mod tests {
    use super::*;

    /// The primitive owns the one-row status-row reserve (task 5.1, D7):
    /// both returned panes already exclude the terminal's bottom status row,
    /// so callers must not shrink them again.
    #[test]
    fn shared_presentation_fills_the_given_area_on_both_panes() {
        let area = Rect {
            x: 2,
            y: 4,
            width: crate::app::TWO_COLUMN_THRESHOLD,
            height: 20,
        };
        let WideHeroPanes {
            hero: left,
            browser: right,
        } = wide_hero_presentation(area, None);
        assert_eq!(left.height, area.height);
        assert_eq!(right.height, area.height);
        assert_eq!(left.bottom(), area.bottom());
        assert_eq!(right.bottom(), area.bottom());
    }

    #[test]
    fn pill_and_right_pane_geometry_saturate_short_areas() {
        let areas = pill_bar_areas(Rect {
            x: 4,
            y: 7,
            width: 10,
            height: 1,
        });
        assert_eq!(areas.pills_area.height, 1);
        assert_eq!(areas.spacer_area.height, 0);
        assert_eq!(areas.content_area.height, 0);

        let right = wide_hero_browser_pane(
            Rect {
                x: 20,
                y: 3,
                width: 10,
                height: 1,
            },
            Rect {
                x: 20,
                y: 3,
                width: 10,
                height: 1,
            },
        );
        assert_eq!(right.list_panel.height, 0);
    }
}

/// Returns `(browser_pane, hero_pane)` for the Wide hero arrangement's
/// horizontal split: a `WIDE_HERO_PANE_GAP`-column gutter between the larger
/// hero pane on the left and a ~40%-width browser (list) pane taking the
/// remainder on the right, each floored at
/// `WIDE_HERO_MIN_PANE_WIDTH`. A `Some` override replaces the ratio default
/// after being clamped to the valid range for this `content_area`; callers
/// reach it only through [`wide_hero_presentation`]/[`wide_library_panes`].
/// The clamp lives in `src/app/list_pane_width.rs`.
pub(in crate::app::render) fn wide_hero_split(
    content_area: Rect,
    override_width: Option<u16>,
) -> (Rect, Rect) {
    let browser_w =
        crate::app::list_pane_width::normalize_list_pane_width(override_width, content_area.width)
            .unwrap_or_else(|| (content_area.width as u32 * 2 / 5) as u16)
            .max(WIDE_HERO_MIN_PANE_WIDTH)
            .min(
                content_area
                    .width
                    .saturating_sub(WIDE_HERO_MIN_PANE_WIDTH)
                    .saturating_sub(WIDE_HERO_PANE_GAP),
            );
    let hero_w = content_area
        .width
        .saturating_sub(browser_w)
        .saturating_sub(WIDE_HERO_PANE_GAP);
    (
        Rect {
            x: content_area.x + hero_w + WIDE_HERO_PANE_GAP,
            y: content_area.y,
            width: browser_w,
            height: content_area.height,
        },
        Rect {
            x: content_area.x,
            y: content_area.y,
            width: hero_w,
            height: content_area.height,
        },
    )
}

#[cfg(test)]
mod split_override_tests {
    use super::*;

    fn content(width: u16) -> Rect {
        Rect {
            x: 3,
            y: 2,
            width,
            height: 20,
        }
    }

    #[test]
    fn override_moves_both_panes_with_the_gap_following() {
        let area = content(120);
        let (default_browser, default_hero) = wide_hero_split(area, None);
        let (browser, hero) = wide_hero_split(area, Some(70));
        // The list pane becomes exactly the override; the hero pane takes the
        // remainder and the shared gutter stays between them.
        assert_eq!(browser.width, 70);
        assert_eq!(browser.x, hero.right() + WIDE_HERO_PANE_GAP);
        assert_eq!(browser.width + WIDE_HERO_PANE_GAP + hero.width, area.width);
        assert!(browser.width > default_browser.width);
        assert!(hero.width < default_hero.width);
    }
}

/// The Wide hero arrangement's right (list) pane geometry: a one-row pill
/// bar flush with the pane's top, then the list panel below it (decision
/// 6's "pill row at top of list pane"). `right_panel` is the pane's full
/// rect (its `y`/`height` anchor the pill row and the panel's bottom);
/// `right_area` is the vertically-inset pane used for the pill row's
/// x/width. The pane's own status-row reserve is owned by
/// [`wide_hero_presentation`]; callers must not re-derive it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::app) struct WideHeroBrowserPane {
    pub pills_area: Rect,
    pub spacer_area: Rect,
    pub list_panel: Rect,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::app) struct PillBarAreas {
    pub pills_area: Rect,
    pub spacer_area: Rect,
    pub content_area: Rect,
}

/// Places the shared one-row pill bar, its one-row parent-background spacer,
/// and the content below them.
pub(in crate::app) fn pill_bar_areas(area: Rect) -> PillBarAreas {
    let reserved = WIDE_HERO_PILL_BAND_HEIGHT;
    PillBarAreas {
        pills_area: Rect {
            height: WIDE_HERO_PILLS_ROW_HEIGHT.min(area.height),
            ..area
        },
        spacer_area: Rect {
            y: area.y.saturating_add(WIDE_HERO_PILLS_ROW_HEIGHT),
            height: WIDE_HERO_PILLS_GAP_ROWS.min(area.height.saturating_sub(1)),
            ..area
        },
        content_area: Rect {
            y: area.y.saturating_add(reserved),
            height: area.height.saturating_sub(reserved),
            ..area
        },
    }
}

pub(in crate::app) fn wide_hero_browser_pane(
    right_panel: Rect,
    right_area: Rect,
) -> WideHeroBrowserPane {
    let areas = pill_bar_areas(Rect {
        x: right_area.x,
        y: right_panel.y,
        width: right_area.width,
        height: right_panel.height,
    });
    WideHeroBrowserPane {
        pills_area: areas.pills_area,
        spacer_area: areas.spacer_area,
        list_panel: areas.content_area,
    }
}

/// Paints the Wide hero left pane and returns the shared content inset
/// (`PANE_PAD_X`, `PANE_PAD_Y`). One owner for fill, extent, inset, and focus
/// resolution -- callers must not resize, re-derive, or conditionally skip the
/// fill, and must not apply a destination-specific inset.
///
/// Takes `content_area` rather than a pane rect so a caller has nothing to
/// hand in but the rect the arrangement already consumes -- it cannot supply
/// a mutated hero pane rect. `wide_hero_presentation` is pure and cheap, so
/// recomputing it here costs nothing. It is split-only: `content_area` may
/// already be the band-reduced area, so this painter must not re-run the
/// Wide/Narrow breakpoint (design D1/D2).
pub(in crate::app) fn wide_hero_hero_pane(
    f: &mut Frame,
    content_area: Rect,
    focused: bool,
    override_width: Option<u16>,
) -> Rect {
    let WideHeroPanes {
        hero: hero_panel, ..
    } = wide_hero_presentation(content_area, override_width);
    let background = palette::surface_colors(palette::Surface::HeroPane, focused).fill;
    f.render_widget(
        Block::default().style(Style::default().bg(background)),
        hero_panel,
    );
    padded_rect(hero_panel, PANE_PAD_X, PANE_PAD_Y)
}

#[cfg(test)]
#[allow(clippy::items_after_test_module)]
mod wide_hero_hero_pane_tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    fn wide_area() -> Rect {
        Rect {
            x: 3,
            y: 2,
            width: crate::app::TWO_COLUMN_THRESHOLD,
            height: WIDE_HERO_MIN_AREA_HEIGHT + 5,
        }
    }

    #[test]
    fn read_only_never_resolves_to_the_focused_surface() {
        let area = wide_area();
        let mut terminal = Terminal::new(TestBackend::new(area.right(), area.bottom())).unwrap();
        let WideHeroPanes {
            hero: left_panel, ..
        } = wide_hero_presentation(area, None);
        terminal
            .draw(|f| {
                let returned = wide_hero_hero_pane(f, area, false, None);
                assert_eq!(returned, padded_rect(left_panel, PANE_PAD_X, PANE_PAD_Y));
            })
            .unwrap();
        let cell = &terminal.backend().buffer()[(left_panel.x, left_panel.y)];
        assert_eq!(
            cell.bg,
            palette::surface_colors(palette::Surface::HeroPane, false).fill
        );
        assert_ne!(cell.bg, palette::resolve_surface_focus(true));
    }

    #[test]
    fn hero_pane_paints_one_sheet_in_both_focus_states_and_returns_the_shared_inset() {
        let area = wide_area();
        let mut terminal = Terminal::new(TestBackend::new(area.right(), area.bottom())).unwrap();
        let WideHeroPanes {
            hero: left_panel, ..
        } = wide_hero_presentation(area, None);
        let expected = padded_rect(left_panel, PANE_PAD_X, PANE_PAD_Y);
        let sheet = palette::surface_colors(palette::Surface::HeroPane, false).fill;
        for focused in [true, false] {
            terminal
                .draw(|f| {
                    let returned = wide_hero_hero_pane(f, area, focused, None);
                    assert_eq!(returned.x, left_panel.x + PANE_PAD_X);
                    assert_eq!(returned.y, left_panel.y + PANE_PAD_Y);
                    assert_eq!(returned, expected);
                })
                .unwrap();
            let cell = &terminal.backend().buffer()[(left_panel.x, left_panel.y)];
            assert_eq!(
                cell.bg, sheet,
                "the hero pane is one fixed sheet, focused={focused}"
            );
            assert_ne!(cell.bg, palette::SURFACE_FOCUSED);
        }
    }

    /// The hero painter is split-only (design D1): it must paint whatever
    /// `content_area` it is given, including one that no longer passes
    /// `wide_hero_fits` once the Selector band has been carved out, so
    /// short-but-Wide panels do not strand a stale frame. The Wide/Narrow
    /// decision is `wide_hero_fits` at `wide_library_panes`/`panel_view`;
    /// `library::tests::band_carve_keeps_the_breakpoint_and_pushes_both_panes_below_it`
    /// owns that breakpoint assertion.
    #[test]
    fn hero_pane_splits_a_band_reduced_content_area_without_re_gating() {
        // Raw height 7 is the shortest Wide area; carving the two-row band
        // leaves a five-row content area that no longer fits.
        let raw = Rect {
            x: 3,
            y: 2,
            width: crate::app::TWO_COLUMN_THRESHOLD,
            height: WIDE_HERO_MIN_AREA_HEIGHT + 1,
        };
        assert!(wide_hero_fits(raw), "raw height 7 is Wide");
        let content_area = pill_bar_areas(raw).content_area;
        assert!(
            !wide_hero_fits(content_area),
            "the carved content area is below the breakpoint"
        );
        let WideHeroPanes {
            hero: hero_panel, ..
        } = wide_hero_presentation(content_area, None);
        let mut terminal = Terminal::new(TestBackend::new(raw.right(), raw.bottom())).unwrap();
        terminal
            .draw(|f| {
                assert_eq!(
                    wide_hero_hero_pane(f, content_area, false, None),
                    padded_rect(hero_panel, PANE_PAD_X, PANE_PAD_Y)
                );
            })
            .unwrap();
        assert_eq!(
            terminal.backend().buffer()[(hero_panel.x, hero_panel.y)].bg,
            palette::surface_colors(palette::Surface::HeroPane, false).fill
        );
    }
}

/// Paints the Wide hero arrangement's main content box: the `MainContentBox`
/// surface's resting fill inset within the Wide hero list pane, present on every
/// Wide hero surface with a kind-dependent payload (the episode listing on TV,
/// the track listing on Music, item description and metadata elsewhere) and one
/// shared padding value (design.md D9, matching the pane inset from D6).
/// Returns both rects so callers can use `panel` for full-bleed row backgrounds
/// and `content` for text layout.
///
/// The focused arm is gone with the deleted `WideHeroContentBoxSurface` (task
/// 5.6, design D6): the Library panel derives the Workspace box's surface from
/// its focus itself, and Music paints the same two fills directly until its
/// conversion (task 9.1).
/// Places an embedded media-list box `gap` rows below `overview_bottom` (the
/// caller's already-painted overview content's real bottom row -- not a
/// pre-reserved slot height), sized to `height` rows and clamped to fit
/// within `content`'s bottom edge. Returns `None` when there is no room
/// (same "omitted when no room" convention as the other slot arrangements).
///
/// Rect-only: no painting, no text measurement -- callers supply the
/// already-measured overview bottom and desired height. Reusable by any
/// Wide hero surface embedding a media list below its overview (TV's
/// episode list; the Library panel's Workspace, task 5.2).
pub(in crate::app) fn place_media_list_below(
    content: Rect,
    overview_bottom: u16,
    gap: u16,
    height: u16,
) -> Option<Rect> {
    let y = overview_bottom.saturating_add(gap);
    if y >= content.bottom() {
        return None;
    }
    let height = height.min(content.bottom().saturating_sub(y));
    (height > 0).then_some(Rect {
        x: content.x,
        y,
        width: content.width,
        height,
    })
}

#[cfg(test)]
#[allow(clippy::items_after_test_module)]
mod wide_hero_slots_tests {
    use super::*;

    fn content() -> Rect {
        Rect {
            x: 1,
            y: 2,
            width: 30,
            height: 20,
        }
    }

    #[test]
    fn place_media_list_below_starts_one_gap_row_after_overview_bottom() {
        let area = content();
        let overview_bottom = area.y + 5;
        let placed =
            place_media_list_below(area, overview_bottom, 1, 6).expect("room for media list");
        assert_eq!(placed.y, overview_bottom + 1);
        assert_eq!(placed.height, 6);
        assert_eq!(placed.x, area.x);
        assert_eq!(placed.width, area.width);
    }

    #[test]
    fn place_media_list_below_clamps_to_content_bottom() {
        let area = content();
        let overview_bottom = area.bottom() - 3;
        let placed =
            place_media_list_below(area, overview_bottom, 1, 6).expect("some room remains");
        assert_eq!(placed.height, 2);
        assert_eq!(placed.bottom(), area.bottom());
    }

    #[test]
    fn place_media_list_below_returns_none_without_room() {
        let area = content();
        let overview_bottom = area.bottom();
        assert!(place_media_list_below(area, overview_bottom, 1, 6).is_none());
    }
}

/// One line of the `Hero` component's Wide hero text block. Unlike
/// inline presentation's single-row, truncated [`super::hero::HeroLine`], Wide hero
/// text wraps across as many rows as it needs (design.md decision 2's
/// "Consequence": text wrapping moves into `Hero`, screens hand over
/// unwrapped strings). Style is screen-chosen (e.g. focus-derived bold),
/// matching how `HeroContent::meta_color` lets an inline browser pick its
/// own colour. `pub(in crate::app)`: the Library panel's pre-5.5 hero
/// placeholder paints through it too.
pub(in crate::app) struct WrappedHeroLine<'a> {
    pub text: &'a str,
    pub style: Style,
}

/// Paints `lines` wrapped to `area`'s width, top to bottom, stopping at
/// `area`'s bottom edge; empty line text is skipped. Returns the first
/// unpainted row.
pub(in crate::app) fn paint_wide_hero_text(
    f: &mut Frame,
    area: Rect,
    lines: &[WrappedHeroLine],
) -> u16 {
    if area.height == 0 || area.width < 3 {
        return area.y;
    }
    let mut row = area.y;
    let wrap_width = (area.width as usize).saturating_sub(1);
    for line in lines {
        if line.text.is_empty() {
            continue;
        }
        for wrapped in wrap(line.text, wrap_width.max(1)) {
            if row >= area.bottom() {
                return row;
            }
            f.render_widget(
                Paragraph::new(Line::from(Span::styled(wrapped.into_owned(), line.style))),
                Rect {
                    x: area.x,
                    y: row,
                    width: area.width,
                    height: 1,
                },
            );
            row += 1;
        }
    }
    row
}
