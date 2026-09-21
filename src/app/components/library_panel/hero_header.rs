//! The Wide Hero header (task 5.5, design D5): three arms — Landscape
//! (16:9 artwork full content width above title/meta), Portrait (2:3) and
//! Square (1:1) (title/meta left, artwork right) — behind one title/meta
//! painter that colours meta row *n* with `HERO_META_ROLES[n % 3]` and owns
//! truncation and wrapping. The Landscape arm lays the title/meta entries
//! out in a two-column grid (title top-left, entries alternating columns
//! row by row, right column right-aligned) so short metadata uses the
//! full content width; Portrait/Square keep the stacked rows. The arm
//! comes from the artwork policy's shape,
//! re-armed to the projected image's decoded aspect once it resolves
//! (`HeroArtwork::painted_shape`), never from a destination. The artwork box is
//! sized by the paint-free [`hero_artwork_box`], the one layout site the
//! shell projection (task 5.10) shares with the painter.

use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;
use unicode_width::UnicodeWidthStr;

use crate::app::palette;
use crate::app::render::{paint_wide_hero_text, render_artwork_placeholder, WrappedHeroLine};
use crate::app::ui_util::trunc_str;

use super::content::{HeroContent, HeroFacts, HeroHeader};
use super::overview_box;
use super::overview_box::OverviewPaint;

/// Blank rows between the artwork box and the text block (the shared
/// `wide_hero_slots` convention: metadata starts at `img_area.bottom() + 1`).
const ARTWORK_TEXT_GAP_ROWS: u16 = 1;

/// Vertical room a present Workspace keeps below the header (design D5: the
/// artwork shrinks before a Workspace viewport would drop). Two padding rows
/// plus a few visible list rows.
const WORKSPACE_MIN_ROWS: u16 = 6;

/// Maximum artwork height for the Wide hero box, shared by Landscape, Portrait,
/// and Square artwork.
const HERO_ARTWORK_MAX_ROWS: u16 = 25;

/// Maximum artwork height for the non-landscape (Portrait/Square) arms. Their
/// boxes are sized from the available height, so they reach the cap directly
/// and a taller block reads as oversized beside the title/meta text.
const HERO_NON_LANDSCAPE_ARTWORK_MAX_ROWS: u16 = 20;

/// Panes whose *terminal* is this short (or fewer rows) use the compact caps
/// below so the header leaves room for the text block, overview, and
/// Workspace beneath it. Measured on the terminal, not the pane, so panel
/// chrome does not change the decision.
pub(in crate::app) const HERO_SHORT_PANE_MAX_HEIGHT: u16 = 50;

/// The compact-cap decision, shared by the artwork box, the overview box,
/// and the shell's image projection: the threshold is the terminal height.
pub(in crate::app) fn short_pane(terminal_height: u16) -> bool {
    terminal_height <= HERO_SHORT_PANE_MAX_HEIGHT
}

/// Compact caps for short panes: 15 rows for Landscape and non-landscape arms
/// alike.
const HERO_SHORT_LANDSCAPE_MAX_ROWS: u16 = 15;
const HERO_SHORT_NON_LANDSCAPE_MAX_ROWS: u16 = 15;

/// Minimum columns the title/meta block keeps beside a right-aligned artwork
/// box (Portrait/Square arms).
const HERO_MIN_TEXT_COLS: u16 = 16;

/// The paint-free artwork box for one hero pane's content area (design D5):
/// Landscape fills the content width at the top with its 16:9 height; the
/// Portrait (2:3) and Square (1:1) boxes are sized from the available height
/// and right-aligned, capped so the text block keeps room. A present
/// Workspace caps the box height first (the artwork shrinks before a
/// Workspace viewport would drop), and the Landscape box additionally
/// shrinks before the wrapped title/meta block below it is starved out of
/// the pane (the legacy wide Emby card's rule, now universal). Tall panes
/// cap all three artwork boxes at 25 rows; panes 50 rows or shorter use the
/// compact 15-row cap.
///
/// One layout site: the header painter calls this and the shell projection
/// (task 5.10) calls it with the Library panel's area, so the fetched image
/// is always encoded for the box that paints it. `terminal_height` is the
/// terminal's row count: the compact caps apply at [`HERO_SHORT_PANE_MAX_HEIGHT`]
/// terminal rows or fewer.
pub(in crate::app) fn hero_artwork_box(
    area: Rect,
    facts: &HeroFacts,
    workspace_present: bool,
    terminal_height: u16,
) -> Rect {
    let header = HeroHeader::from(facts.artwork.painted_shape());
    // Short terminals leave room for the text block, overview, and Workspace
    // beneath the header: compact 15-row caps at 50 rows or fewer.
    let short = short_pane(terminal_height);
    let landscape_cap = if short {
        HERO_SHORT_LANDSCAPE_MAX_ROWS
    } else {
        HERO_ARTWORK_MAX_ROWS
    };
    let non_landscape_cap = if short {
        HERO_SHORT_NON_LANDSCAPE_MAX_ROWS
    } else {
        HERO_NON_LANDSCAPE_ARTWORK_MAX_ROWS
    };
    // The artwork shrinks before a Workspace viewport would drop (design D5).
    let max_h = if workspace_present {
        area.height.saturating_sub(WORKSPACE_MIN_ROWS)
    } else {
        area.height
    }
    .min(landscape_cap);
    let (width, height) = match header.arm() {
        super::content::HeroHeaderArm::Landscape => {
            // 16:9 in terminal cells (cells are ~2x taller than wide).
            let h = (area.width.saturating_mul(9).saturating_add(31) / 32).max(1);
            // The artwork shrinks before the title/meta block below it is
            // starved out of the pane: the box keeps room for the wrapped
            // text rows plus the gap row between the two blocks.
            let text_rows = landscape_text_rows(area.width, facts);
            let room_for_text = area
                .height
                .saturating_sub(text_rows)
                .saturating_sub(ARTWORK_TEXT_GAP_ROWS);
            (area.width, h.min(max_h).min(room_for_text))
        }
        super::content::HeroHeaderArm::Portrait => {
            box_from_height(max_h.min(non_landscape_cap), 4, 3, area)
        }
        super::content::HeroHeaderArm::Square => {
            box_from_height(max_h.min(non_landscape_cap), 2, 1, area)
        }
    };
    Rect {
        x: area.right().saturating_sub(width).max(area.x),
        y: area.y,
        width: width.min(area.width),
        height,
    }
}

/// Right-aligned box of `numerator:denominator` display aspect, sized from
/// `max_h` and capped so the text block keeps [`HERO_MIN_TEXT_COLS`] columns.
/// Cell aspect (~2x taller than wide) is folded into the ratio: a 2:3
/// portrait box is `h*4/3` cells wide, a square `h*2`.
fn box_from_height(max_h: u16, num: u16, den: u16, area: Rect) -> (u16, u16) {
    let cap_w = area
        .width
        .saturating_sub(HERO_MIN_TEXT_COLS)
        .min(area.width);
    let w = ((max_h.saturating_mul(num).saturating_add(den - 1)) / den).min(cap_w);
    let h = (w.saturating_mul(den) / num).min(max_h);
    (w, h)
}

/// Minimum columns each landscape-grid column keeps: the text block
/// beside Portrait/Square art never goes narrower, so neither does a
/// grid column. Below twice that width the Landscape arm falls back to
/// the stacked rows.
fn landscape_grid_columns(width: u16) -> Option<(u16, u16)> {
    if width < 2 * HERO_MIN_TEXT_COLS {
        return None;
    }
    let left = width / 2;
    Some((left, width.saturating_sub(left)))
}

/// Rows the Landscape title/meta block needs: the two-column grid packs
/// two non-empty entries per row (cells truncate, never wrap), or — on a
/// text block too narrow for two columns — the same wrap
/// [`paint_wide_hero_text`] applies (`width - 1`), so the box's
/// text-starvation cap matches what the painter actually paints below it.
fn landscape_text_rows(width: u16, facts: &HeroFacts) -> u16 {
    if landscape_grid_columns(width).is_some() {
        facts.live_entries().count().div_ceil(2) as u16
    } else {
        let wrap_width = (width as usize).saturating_sub(1).max(1);
        facts
            .live_entries()
            .map(|line| textwrap::wrap(line, wrap_width).len() as u16)
            .sum()
    }
}

/// Paints the Hero header and — when overview text exists — the overview
/// Main content box below it (design D5). One title/meta painter for all
/// three arms, truncation and wrapping owned here; meta row *n* is coloured
/// `HERO_META_ROLES[n % 3]`. The artwork box renders the shared placeholder
/// while the projected image is not ready (task 5.10: `Ready` reserves the
/// box and the shell paints the projected protocol into it after view);
/// the returned second element is the reserved box's rect when a projected
/// image should paint there. Returns `(first unpainted row, reserved image
/// box)`, the row the skeleton places the Workspace below.
pub(in crate::app) fn paint_hero_pane_content(
    f: &mut Frame,
    area: Rect,
    content: &HeroContent<'_>,
    overview_scroll: usize,
    hovered_link: Option<usize>,
    link_hits: &mut crate::app::components::mouse::hit::HitRegions<usize>,
    surface: palette::Surface,
    terminal_height: u16,
) -> (u16, Option<Rect>, Option<OverviewPaint>) {
    let header = HeroHeader::from(content.facts.artwork.painted_shape());
    let artwork = hero_artwork_box(
        area,
        &content.facts,
        content.workspace.is_some(),
        terminal_height,
    );
    let image_ready = matches!(
        content.facts.artwork.image,
        super::content::HeroImageState::Ready { .. }
    );
    if artwork.width > 0 && artwork.height > 0 && !image_ready {
        // Placeholder at full box size while loading or imageless (spec: the
        // artwork box keeps its size and shows the shared placeholder).
        render_artwork_placeholder(f, artwork);
    }

    let text_area = match header.arm() {
        super::content::HeroHeaderArm::Landscape => Rect {
            y: artwork.bottom().saturating_add(ARTWORK_TEXT_GAP_ROWS),
            height: area
                .bottom()
                .saturating_sub(artwork.bottom().saturating_add(ARTWORK_TEXT_GAP_ROWS)),
            ..area
        },
        // Portrait/Square: title/meta left of the artwork box.
        _ => Rect {
            width: artwork.x.saturating_sub(area.x).saturating_sub(1),
            ..area
        },
    };
    // The Landscape arm packs title/meta into the two-column grid when
    // the text block fits two columns; Portrait/Square keep stacked rows.
    let grid = (header.arm() == super::content::HeroHeaderArm::Landscape)
        .then(|| landscape_grid_columns(text_area.width))
        .flatten();
    let mut next_row =
        paint_title_and_meta(f, text_area, &content.facts, hovered_link, link_hits, grid);
    // The header's painted bottom edge includes a right-side artwork box the
    // text block may not reach.
    if header.arm() != super::content::HeroHeaderArm::Landscape {
        next_row = next_row.max(artwork.bottom());
    }
    let reserved_image =
        (artwork.width > 0 && artwork.height > 0 && image_ready).then_some(artwork);

    // The overview Main content box only when overview text exists (design
    // D5); without it the Workspace moves up.
    let overview = overview_box::paint_overview_box(
        f,
        area,
        next_row,
        content,
        overview_scroll,
        surface,
        terminal_height,
    );
    let next_row = overview
        .as_ref()
        .map(|overview| overview.bottom)
        .unwrap_or(next_row);
    (next_row, reserved_image, overview)
}

/// The one title/meta painter for all three arms: the title in
/// [`palette::TEXT_HERO_TITLE`], then each meta row in
/// `HERO_META_ROLES[cycle % 3]` — except the duration row
/// (`HeroFacts::duration_row`) and the resume-percentage row
/// (`HeroFacts::progress_row`), painted the `DURATION` and `PROGRESS_PERCENT`
/// roles and skipped by the cycle — wrapped to the text block's width
/// (truncation and wrapping owned here, design D5). The Landscape arm instead
/// packs the entries
/// into [`paint_title_and_meta_grid`]'s two columns when `grid` carries
/// [`landscape_grid_columns`]' widths. Returns the first unpainted row.
fn paint_title_and_meta(
    f: &mut Frame,
    area: Rect,
    facts: &HeroFacts,
    hovered_link: Option<usize>,
    link_hits: &mut crate::app::components::mouse::hit::HitRegions<usize>,
    grid: Option<(u16, u16)>,
) -> u16 {
    let mut lines: Vec<WrappedHeroLine<'_>> = Vec::with_capacity(1 + facts.meta_rows.len());
    lines.push(WrappedHeroLine {
        text: &facts.title,
        style: ratatui::style::Style::default().fg(palette::TEXT_HERO_TITLE),
    });
    let mut cycle = 0usize;
    for (index, row) in facts.meta_rows.iter().enumerate() {
        let fg = if facts.duration_row == Some(index) {
            palette::DURATION
        } else if facts.progress_row == Some(index) {
            palette::PROGRESS_PERCENT
        } else {
            let fg = palette::HERO_META_ROLES[cycle % palette::HERO_META_ROLES.len()];
            cycle += 1;
            fg
        };
        lines.push(WrappedHeroLine {
            text: row,
            style: ratatui::style::Style::default().fg(fg),
        });
    }
    if let Some((left_w, _)) = grid {
        let next_row = paint_title_and_meta_grid(f, area, left_w, &lines);
        overview_box::overlay_links_grid(f, area, left_w, facts, hovered_link, link_hits);
        return next_row;
    }
    let next_row = paint_wide_hero_text(f, area, &lines);
    overview_box::overlay_links(f, area, facts, hovered_link, link_hits);
    next_row
}

/// Landscape two-column grid over already-styled title/meta `entries`:
/// entry 0 (the title) top-left, then entries alternate columns row by
/// row; the left column is left-aligned, the right column right-aligned.
/// Cells truncate to their column width and never wrap, so a grid row
/// holds exactly two entries and empty entries leave no cell. Returns
/// the first unpainted row.
fn paint_title_and_meta_grid(
    f: &mut Frame,
    area: Rect,
    left_w: u16,
    entries: &[WrappedHeroLine<'_>],
) -> u16 {
    if area.height == 0 {
        return area.y;
    }
    let right_w = area.width.saturating_sub(left_w);
    let live: Vec<&WrappedHeroLine<'_>> = entries
        .iter()
        .filter(|entry| !entry.text.is_empty())
        .collect();
    let mut row = area.y;
    for pair in live.chunks(2) {
        if row >= area.bottom() {
            break;
        }
        let left_text = trunc_str(pair[0].text, left_w as usize);
        let mut spans = vec![
            Span::styled(left_text.clone(), pair[0].style),
            Span::raw(" ".repeat((left_w as usize).saturating_sub(left_text.width()))),
        ];
        match pair.get(1) {
            Some(right) => {
                let right_text = trunc_str(right.text, right_w as usize);
                spans.push(Span::raw(
                    " ".repeat((right_w as usize).saturating_sub(right_text.width())),
                ));
                spans.push(Span::styled(right_text, right.style));
            }
            None => spans.push(Span::raw(" ".repeat(right_w as usize))),
        }
        f.render_widget(
            Paragraph::new(Line::from(spans)),
            Rect {
                x: area.x,
                y: row,
                width: area.width,
                height: 1,
            },
        );
        row += 1;
    }
    row
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod hero_header_tests {
    use super::*;
    use crate::app::components::library_panel::content::PanelList;
    use crate::app::components::library_panel::content::{ArtworkShape, HeroArtwork, Workspace};
    use crate::app::palette;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    /// A stub `PanelList` so a Workspace can be present for the shrink rule.
    struct NoopList;
    impl PanelList for NoopList {
        fn clamp_viewport(&mut self, _viewport_height: usize) {}

        fn set_paint_policy(
            &mut self,
            _policy: crate::app::components::library_panel::content::PanelListPaintPolicy,
        ) {
        }

        fn view(&mut self, _f: &mut Frame, _rect: Rect) {}
    }

    const AREA: Rect = Rect::new(0, 0, 60, 30);

    fn facts(shape: ArtworkShape) -> HeroFacts {
        HeroFacts {
            title: "Dune".into(),
            meta_rows: vec!["2021".into()],
            duration_row: None,
            progress_row: None,
            links: Vec::new(),
            artwork: HeroArtwork {
                shape,
                source: None,
                decoration: None,
                image: crate::app::components::library_panel::content::HeroImageState::None,
            },
        }
    }

    fn content(shape: ArtworkShape) -> HeroContent<'static> {
        HeroContent {
            facts: facts(shape),
            overview: None,
            credits: None,
            workspace: None,
        }
    }

    fn draw_pane(width: u16, height: u16, pane: &HeroContent<'_>) -> ratatui::buffer::Buffer {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        let area = Rect::new(0, 0, width, height);
        let pane_area = area;
        terminal
            .draw(|f| {
                paint_hero_pane_content(
                    f,
                    pane_area,
                    pane,
                    0,
                    None,
                    &mut crate::app::components::mouse::hit::HitRegions::new(),
                    palette::Surface::HeroPane,
                    height,
                );
            })
            .unwrap();
        terminal.backend().buffer().clone()
    }

    fn text_in(buf: &ratatui::buffer::Buffer, area: Rect, needle: &str) -> bool {
        for y in area.top()..area.bottom() {
            let mut line = String::new();
            for x in area.left()..area.right() {
                line.push_str(buf[(x, y)].symbol());
            }
            if line.contains(needle) {
                return true;
            }
        }
        false
    }

    fn placeholder_fill() -> ratatui::style::Color {
        palette::surface_colors(palette::Surface::ArtworkPlaceholder, false).fill
    }

    #[test]
    fn landscape_header_paints_art_above_text() {
        let pane = content(ArtworkShape::Landscape);
        let artwork = hero_artwork_box(AREA, &pane.facts, pane.workspace.is_some(), AREA.height);
        let tall = Rect::new(0, 0, 113, 51);
        let capped = hero_artwork_box(tall, &pane.facts, false, tall.height);
        assert_eq!(capped.height, HERO_ARTWORK_MAX_ROWS);
        let tall_buf = draw_pane(tall.width, tall.height, &pane);
        assert_eq!(
            tall_buf[(capped.right() - 1, capped.bottom() - 1)].bg,
            placeholder_fill()
        );
        let buf = draw_pane(AREA.width, AREA.height, &pane);
        // The artwork box spans the full content width at the top.
        assert_eq!(artwork.x, AREA.x);
        assert_eq!(artwork.width, AREA.width);
        // The box carries the shared placeholder at its full size.
        assert_eq!(buf[(artwork.x + 1, artwork.y + 1)].bg, placeholder_fill());
        assert_eq!(
            buf[(artwork.right() - 1, artwork.bottom() - 1)].bg,
            placeholder_fill()
        );
        // The title paints below the box, never inside it.
        let below = Rect {
            y: artwork.bottom(),
            height: AREA.bottom() - artwork.bottom(),
            ..AREA
        };
        assert!(text_in(&buf, below, "Dune"));
        assert!(text_in(&buf, below, "2021"));
        let inside = (artwork.y..artwork.bottom())
            .map(|y| {
                (artwork.x..artwork.right())
                    .map(|x| buf[(x, y)].symbol().to_string())
                    .collect::<String>()
            })
            .any(|line| line.contains("Dune"));
        assert!(!inside, "no title inside the artwork box");
    }

    #[test]
    fn square_header_paints_art_right_of_text() {
        let pane = content(ArtworkShape::Square);
        let artwork = hero_artwork_box(AREA, &pane.facts, pane.workspace.is_some(), AREA.height);
        // A pane wide enough that the row cap, not the text column minimum, is
        // the binding constraint.
        let tall = Rect::new(0, 0, 100, 51);
        let capped = hero_artwork_box(tall, &pane.facts, false, tall.height);
        assert_eq!(capped.height, HERO_NON_LANDSCAPE_ARTWORK_MAX_ROWS);
        let tall_buf = draw_pane(tall.width, tall.height, &pane);
        assert_eq!(
            tall_buf[(capped.right() - 1, capped.bottom() - 1)].bg,
            placeholder_fill()
        );
        assert_eq!(
            hero_artwork_box(Rect::new(0, 0, 60, 20), &pane.facts, true, 20).height,
            14
        );
        let buf = draw_pane(AREA.width, AREA.height, &pane);
        // The artwork box is right-aligned; title and meta paint left of it.
        assert_eq!(artwork.right(), AREA.right());
        assert!(artwork.x > AREA.x + 12, "art is on the right");
        let text_area = Rect {
            width: artwork.x.saturating_sub(AREA.x + 1),
            ..AREA
        };
        assert!(text_in(&buf, text_area, "Dune"));
        assert!(text_in(&buf, text_area, "2021"));
        // The placeholder fills the whole box.
        assert_eq!(buf[(artwork.x + 1, artwork.y + 1)].bg, placeholder_fill());
        assert_eq!(
            buf[(artwork.right() - 1, artwork.bottom() - 1)].bg,
            placeholder_fill()
        );
    }

    #[test]
    fn portrait_header_paints_art_right_of_text() {
        let pane = content(ArtworkShape::Portrait);
        let artwork = hero_artwork_box(AREA, &pane.facts, pane.workspace.is_some(), AREA.height);
        let tall = Rect::new(0, 0, 60, 51);
        let capped = hero_artwork_box(tall, &pane.facts, false, tall.height);
        assert_eq!(capped.height, HERO_NON_LANDSCAPE_ARTWORK_MAX_ROWS);
        let tall_buf = draw_pane(tall.width, tall.height, &pane);
        assert_eq!(
            tall_buf[(capped.right() - 1, capped.bottom() - 1)].bg,
            placeholder_fill()
        );
        assert_eq!(
            hero_artwork_box(Rect::new(0, 0, 60, 20), &pane.facts, true, 20).height,
            14
        );
        let buf = draw_pane(AREA.width, AREA.height, &pane);
        assert_eq!(artwork.right(), AREA.right());
        let text_area = Rect {
            width: artwork.x.saturating_sub(AREA.x + 1),
            ..AREA
        };
        assert!(text_in(&buf, text_area, "Dune"));
        // The placeholder fills the whole box.
        assert_eq!(buf[(artwork.x + 1, artwork.y + 1)].bg, placeholder_fill());
    }

    /// A `Ready` projected image with `decoded` source pixels, on top of the
    /// given declared policy shape — the agreement the painter must honour.
    fn with_decoded_image(shape: ArtworkShape, decoded: (u32, u32)) -> HeroContent<'static> {
        HeroContent {
            facts: HeroFacts {
                title: "Dune".into(),
                meta_rows: vec!["2021".into()],
                duration_row: None,
                progress_row: None,
                links: Vec::new(),
                artwork: HeroArtwork {
                    shape,
                    source: None,
                    decoration: None,
                    image: crate::app::components::library_panel::content::HeroImageState::Ready {
                        cache_key: "k".into(),
                        decoded: Some(decoded),
                    },
                },
            },
            overview: None,
            credits: None,
            workspace: None,
        }
    }

    /// The image the fetch actually resolved governs the arm: a series whose
    /// policy pinned Landscape from a declared thumb but whose chain resolved
    /// to the portrait poster paints the Portrait arm — title/meta beside a
    /// right-aligned box, never stacked underneath the art.
    #[test]
    fn landscape_declared_series_with_a_portrait_poster_paints_text_beside_art() {
        let pane = with_decoded_image(ArtworkShape::Landscape, (200, 300));
        let artwork = hero_artwork_box(AREA, &pane.facts, false, AREA.height);
        // Right-aligned side-by-side box, never the full content width.
        assert_eq!(artwork.right(), AREA.right(), "art is right-aligned");
        assert!(
            artwork.width < AREA.width,
            "not the Landscape full-width box"
        );
        let buf = draw_pane(AREA.width, AREA.height, &pane);
        let beside = Rect {
            width: artwork.x.saturating_sub(AREA.x + 1),
            ..AREA
        };
        assert!(text_in(&buf, beside, "Dune"));
        assert!(text_in(&buf, beside, "2021"));
        let below = Rect {
            y: artwork.bottom(),
            height: AREA.bottom() - artwork.bottom(),
            ..AREA
        };
        assert!(!text_in(&buf, below, "Dune"), "no title stacked below");
    }

    /// Genuinely 16:9 artwork keeps the Landscape arm: full-width art above
    /// the title/meta block.
    #[test]
    fn landscape_declared_series_with_a_landscape_thumb_keeps_art_above_text() {
        let pane = with_decoded_image(ArtworkShape::Landscape, (1600, 900));
        let artwork = hero_artwork_box(AREA, &pane.facts, false, AREA.height);
        assert_eq!(artwork.x, AREA.x);
        assert_eq!(artwork.width, AREA.width);
        let buf = draw_pane(AREA.width, AREA.height, &pane);
        let below = Rect {
            y: artwork.bottom(),
            height: AREA.bottom() - artwork.bottom(),
            ..AREA
        };
        assert!(text_in(&buf, below, "Dune"));
        assert!(text_in(&buf, below, "2021"));
    }

    /// Square artwork (music) keeps the Square arm: side-by-side, unchanged.
    #[test]
    fn square_policy_with_square_art_keeps_text_beside_art() {
        let pane = with_decoded_image(ArtworkShape::Square, (500, 500));
        let artwork = hero_artwork_box(AREA, &pane.facts, false, AREA.height);
        assert_eq!(artwork.right(), AREA.right());
        assert!(
            artwork.width < AREA.width,
            "not the Landscape full-width box"
        );
        let buf = draw_pane(AREA.width, AREA.height, &pane);
        let beside = Rect {
            width: artwork.x.saturating_sub(AREA.x + 1),
            ..AREA
        };
        assert!(text_in(&buf, beside, "Dune"));
        assert!(text_in(&buf, beside, "2021"));
    }

    #[test]
    fn meta_rows_cycle_the_three_roles_across_the_grid() {
        let pane_facts = HeroFacts {
            title: "T".into(),
            meta_rows: vec!["a".into(), "b".into(), "c".into(), "d".into()],
            duration_row: None,
            progress_row: None,
            links: Vec::new(),
            artwork: HeroArtwork {
                shape: ArtworkShape::Landscape,
                source: None,
                decoration: None,
                image: crate::app::components::library_panel::content::HeroImageState::None,
            },
        };
        let pane = HeroContent {
            facts: pane_facts,
            overview: None,
            credits: None,
            workspace: None,
        };
        let artwork = hero_artwork_box(AREA, &pane.facts, pane.workspace.is_some(), AREA.height);
        let buf = draw_pane(AREA.width, AREA.height, &pane);
        // Grid entries [T, a, b, c, d]: title top-left, then alternating
        // columns — left cells keep the indent column, right cells end at
        // the area's right edge. Colours still cycle per meta row.
        let y0 = artwork.bottom() + 1;
        assert_eq!(buf[(AREA.x, y0)].symbol(), "T");
        assert_eq!(
            buf[(AREA.x, y0)].style().fg,
            Some(palette::TEXT_HERO_TITLE),
            "title colour"
        );
        let cells = [
            (AREA.right() - 1, y0, "a", palette::HERO_META_ROLES[0]),
            (AREA.x, y0 + 1, "b", palette::HERO_META_ROLES[1]),
            (AREA.right() - 1, y0 + 1, "c", palette::HERO_META_ROLES[2]),
            (AREA.x, y0 + 2, "d", palette::HERO_META_ROLES[0]),
        ];
        for (x, y, symbol, expected) in cells {
            assert_eq!(buf[(x, y)].symbol(), symbol);
            assert_eq!(
                buf[(x, y)].style().fg,
                Some(expected),
                "meta {symbol} colour"
            );
        }
        assert_eq!(
            buf[(AREA.x + AREA.width / 2, y0)].symbol(),
            " ",
            "right cell opens with alignment padding"
        );
    }

    /// The duration row paints the `DURATION` role (the sage) and the cycle
    /// skips it, so the remaining rows keep adjacent cycle colours.
    #[test]
    fn duration_row_paints_the_duration_role_and_the_cycle_skips_it() {
        let mut pane_facts = facts(ArtworkShape::Landscape);
        pane_facts.meta_rows = vec!["a".into(), "b".into(), "c".into()];
        pane_facts.duration_row = Some(1);
        let pane = HeroContent {
            facts: pane_facts,
            overview: None,
            credits: None,
            workspace: None,
        };
        let artwork = hero_artwork_box(AREA, &pane.facts, pane.workspace.is_some(), AREA.height);
        let buf = draw_pane(AREA.width, AREA.height, &pane);
        // Grid entries [Dune, a, b, c]: the duration row keeps its grid
        // cell and the cycle still skips it.
        let y0 = artwork.bottom() + 1;
        assert_eq!(
            buf[(AREA.right() - 1, y0)].style().fg,
            Some(palette::HERO_META_ROLES[0]),
            "row before the duration keeps its cycle colour"
        );
        assert_eq!(
            buf[(AREA.x, y0 + 1)].style().fg,
            Some(palette::DURATION),
            "duration row paints the DURATION role"
        );
        assert_eq!(buf[(AREA.x, y0 + 1)].symbol(), "b");
        assert_eq!(
            buf[(AREA.right() - 1, y0 + 1)].style().fg,
            Some(palette::HERO_META_ROLES[1]),
            "the cycle skips the duration row"
        );
    }

    /// The resume-percentage row paints the `PROGRESS_PERCENT` role (the
    /// orange) and the cycle skips it, so the metadata rows around it keep
    /// adjacent cycle colours.
    #[test]
    fn progress_row_paints_the_progress_role_and_the_cycle_skips_it() {
        let mut pane_facts = facts(ArtworkShape::Landscape);
        pane_facts.meta_rows = vec!["a".into(), "47%".into(), "c".into()];
        pane_facts.progress_row = Some(1);
        let pane = HeroContent {
            facts: pane_facts,
            overview: None,
            credits: None,
            workspace: None,
        };
        let artwork = hero_artwork_box(AREA, &pane.facts, pane.workspace.is_some(), AREA.height);
        let buf = draw_pane(AREA.width, AREA.height, &pane);
        // Grid entries [Dune, a, 47%, c]: the percentage row keeps its grid
        // cell and the cycle still skips it.
        let y0 = artwork.bottom() + 1;
        assert_eq!(
            buf[(AREA.right() - 1, y0)].style().fg,
            Some(palette::HERO_META_ROLES[0]),
            "row before the progress row keeps its cycle colour"
        );
        assert_eq!(buf[(AREA.x, y0 + 1)].symbol(), "4");
        assert_eq!(
            buf[(AREA.x, y0 + 1)].style().fg,
            Some(palette::PROGRESS_PERCENT),
            "the percentage row paints the progress role"
        );
        assert_eq!(
            buf[(AREA.right() - 1, y0 + 1)].style().fg,
            Some(palette::HERO_META_ROLES[1]),
            "the cycle skips the progress row"
        );
    }

    /// A text block too narrow for two columns keeps the stacked rows:
    /// entries paint top to bottom in the left column, colours unchanged.
    #[test]
    fn narrow_landscape_falls_back_to_stacked_rows() {
        let pane = content(ArtworkShape::Landscape);
        let narrow = Rect::new(0, 0, 24, 30);
        assert!(
            landscape_grid_columns(narrow.width).is_none(),
            "24 columns fit no two text columns"
        );
        let artwork =
            hero_artwork_box(narrow, &pane.facts, pane.workspace.is_some(), narrow.height);
        let buf = draw_pane(narrow.width, narrow.height, &pane);
        let y0 = artwork.bottom() + 1;
        assert_eq!(buf[(narrow.x, y0)].symbol(), "D");
        assert_eq!(buf[(narrow.x, y0 + 1)].symbol(), "2");
        assert_eq!(
            buf[(narrow.x, y0 + 1)].style().fg,
            Some(palette::HERO_META_ROLES[0]),
            "stacked fallback keeps the cycle colour"
        );
    }

    #[test]
    fn artwork_shrinks_before_a_workspace_viewport_drops() {
        let free = content(ArtworkShape::Landscape);
        let small = Rect::new(0, 0, 60, 14);
        let without_workspace =
            hero_artwork_box(small, &free.facts, free.workspace.is_some(), small.height);
        let mut workspace_list = NoopList;
        let constrained = HeroContent {
            facts: facts(ArtworkShape::Landscape),
            overview: None,
            credits: None,
            workspace: Some(Workspace {
                header: None,
                selector: None,
                list: &mut workspace_list,
                focused: false,
            }),
        };
        let with_workspace = hero_artwork_box(
            small,
            &constrained.facts,
            constrained.workspace.is_some(),
            small.height,
        );
        // The landscape box no longer fills the pane even without a
        // Workspace: it leaves room for the wrapped title/meta rows below
        // it, and a present Workspace caps it further so the Workspace
        // keeps its minimum rows.
        assert!(without_workspace.height < small.height);
        assert_eq!(
            with_workspace.height,
            small.height.saturating_sub(WORKSPACE_MIN_ROWS)
        );
    }

    #[test]
    fn landscape_artwork_shrinks_before_the_title_meta_block_is_starved() {
        // A wide, short pane: the raw 16:9 box (113 wide → 32 rows) would
        // fill the whole pane and leave the title/meta block no rows.
        let pane = content(ArtworkShape::Landscape);
        let small = Rect::new(0, 0, 113, 19);
        let artwork = hero_artwork_box(small, &pane.facts, pane.workspace.is_some(), small.height);
        assert!(
            artwork.height < small.height,
            "artwork leaves room for the text block below it"
        );
        // The title and meta paint below the capped box.
        let buf = draw_pane(small.width, small.height, &pane);
        let below = Rect {
            y: artwork.bottom(),
            height: small.bottom() - artwork.bottom(),
            ..small
        };
        assert!(text_in(&buf, below, "Dune"));
        assert!(text_in(&buf, below, "2021"));
    }

    #[test]
    fn short_pane_uses_compact_caps() {
        let landscape = content(ArtworkShape::Landscape);
        assert_eq!(
            hero_artwork_box(Rect::new(0, 0, 113, 50), &landscape.facts, false, 50).height,
            HERO_SHORT_LANDSCAPE_MAX_ROWS
        );
        let square = content(ArtworkShape::Square);
        assert_eq!(
            hero_artwork_box(Rect::new(0, 0, 100, 50), &square.facts, false, 50).height,
            HERO_SHORT_NON_LANDSCAPE_MAX_ROWS
        );
        let portrait = content(ArtworkShape::Portrait);
        assert_eq!(
            hero_artwork_box(Rect::new(0, 0, 60, 50), &portrait.facts, false, 50).height,
            HERO_SHORT_NON_LANDSCAPE_MAX_ROWS
        );
    }

    #[test]
    fn header_arm_is_derived_from_the_policy_shape_only() {
        use super::super::content::HeroHeaderArm;
        // `HeroHeader::from` is the only constructor: the arm is always the
        // policy shape's arm (no destination-selectable constructor exists —
        // the field is private to the content module).
        assert_eq!(
            HeroHeader::from(ArtworkShape::Landscape).arm(),
            HeroHeaderArm::Landscape
        );
        assert_eq!(
            HeroHeader::from(ArtworkShape::Portrait).arm(),
            HeroHeaderArm::Portrait
        );
        assert_eq!(
            HeroHeader::from(ArtworkShape::Square).arm(),
            HeroHeaderArm::Square
        );
    }
}
