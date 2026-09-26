//! The Wide Hero title/meta painter: one painter for all three arms over
//! the [`super::artwork_box`] geometry — the Landscape two-column grid plus
//! the stacked rows, meta-row role cycling, truncation/wrapping, and the
//! overview box below the header.

use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;
use unicode_width::UnicodeWidthStr;

use crate::app::palette;
use crate::app::render::{paint_wide_hero_text, render_artwork_placeholder, WrappedHeroLine};
use crate::app::ui_util::trunc_str;

use super::super::content::{HeroContent, HeroFacts, HeroHeader};
use super::super::overview_box;
use super::super::overview_box::OverviewPaint;
use super::artwork_box::{hero_artwork_box, landscape_grid_columns, ARTWORK_TEXT_GAP_ROWS};

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
        super::super::content::HeroImageState::Ready { .. }
    );
    if artwork.width > 0 && artwork.height > 0 && !image_ready {
        // Placeholder at full box size while loading or imageless (spec: the
        // artwork box keeps its size and shows the shared placeholder).
        render_artwork_placeholder(f, artwork);
    }

    let text_area = match header.arm() {
        super::super::content::HeroHeaderArm::Landscape => Rect {
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
    let grid = (header.arm() == super::super::content::HeroHeaderArm::Landscape)
        .then(|| landscape_grid_columns(text_area.width))
        .flatten();
    let mut next_row =
        paint_title_and_meta(f, text_area, &content.facts, hovered_link, link_hits, grid);
    // The header's painted bottom edge includes a right-side artwork box the
    // text block may not reach.
    if header.arm() != super::super::content::HeroHeaderArm::Landscape {
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
        .map_or(next_row, |overview| overview.bottom);
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
