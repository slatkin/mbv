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

/// Omits both the Selector row and its gap, giving all of the area to content.
pub(in crate::app) fn no_selector_areas(area: Rect) -> PillBarAreas {
    PillBarAreas {
        pills_area: Rect::new(area.x, area.y, area.width, 0),
        spacer_area: Rect::new(area.x, area.y, area.width, 0),
        content_area: area,
    }
}

pub(in crate::app) fn wide_hero_browser_pane_with_selector(
    right_panel: Rect,
    right_area: Rect,
    has_selector: bool,
) -> WideHeroBrowserPane {
    let area = Rect {
        x: right_area.x,
        y: right_panel.y,
        width: right_area.width,
        height: right_panel.height,
    };
    let areas = if has_selector {
        pill_bar_areas(area)
    } else {
        no_selector_areas(area)
    };
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
    let browser_w = crate::app::state::list_pane_width::normalize_list_pane_width(
        override_width,
        content_area.width,
    )
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
