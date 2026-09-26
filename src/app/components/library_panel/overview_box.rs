//! Wide Hero Main content box: overview text and the Movie credits table.
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};

use crate::app::render::components::widgets::render_right_scrollbar_with_viewport;
use ratatui::Frame;
use unicode_width::UnicodeWidthStr;

use crate::app::components::mouse::hit::HitRegions;
use crate::app::palette;
use crate::app::render::{PANE_PAD_X, PANE_PAD_Y};
use crate::app::ui_util::trunc_str;

use super::content::{HeroContent, HeroCredit, HeroFacts};

/// The painted Main content box and its scroll metrics, retained by the
/// Library panel for interaction against the same geometry painted here.
#[derive(Clone, Copy, Debug)]
pub(in crate::app) struct OverviewPaint {
    pub bottom: u16,
    pub rect: Rect,
    pub content_length: usize,
    pub viewport: usize,
}

const OVERVIEW_CREDITS_GAP_ROWS: usize = 2;
const MIN_ROLE_WIDTH: u16 = 8;

/// Overview content rows a short terminal shows at most (the rest of the
/// flow scrolls); applies at [`crate::app::components::library_panel::
/// hero_header::HERO_SHORT_PANE_MAX_HEIGHT`] terminal rows or fewer.
const OVERVIEW_SHORT_PANE_MAX_ROWS: u16 = 5;

fn has_overview_and_credits(overview: Option<&str>, credits: Option<&[HeroCredit]>) -> bool {
    overview.is_some() && credits.is_some()
}

pub(in crate::app) fn paint_overview_box(
    f: &mut Frame,
    area: Rect,
    next_row: u16,
    content: &HeroContent<'_>,
    scroll_offset: usize,
    pane_surface: palette::Surface,
    terminal_height: u16,
) -> Option<OverviewPaint> {
    let overview = content
        .overview
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    let credits = content.credits.as_deref().filter(|rows| !rows.is_empty());
    if overview.is_none() && credits.is_none() {
        return None;
    }
    let text_rows = overview.map_or(0, |text| {
        textwrap::wrap(
            text,
            (area.width.saturating_sub(PANE_PAD_X * 2) as usize)
                .saturating_sub(1)
                .max(1),
        )
        .len()
    });
    let credit_rows = credits.map_or(0, <[HeroCredit]>::len);
    let has_credits_gap = has_overview_and_credits(overview, credits);
    // The overview text, the separator and the table are ONE scrollable flow:
    // the caller's offset shifts the whole box content, so this is the box's
    // full content length.
    let gap_rows = if has_credits_gap {
        OVERVIEW_CREDITS_GAP_ROWS
    } else {
        0
    };
    let inner_rows = text_rows + gap_rows + credit_rows;
    let box_y = next_row.saturating_add(1);
    let room = area.bottom().saturating_sub(box_y);
    let box_height = overview_box_height(
        inner_rows,
        room,
        content.workspace.is_some(),
        terminal_height,
    );
    if box_height <= PANE_PAD_Y * 2 {
        return None;
    }
    // The blank row between the header text and the overview box belongs to
    // the pane the content is painted into, so it repaints that pane's own
    // surface: the Wide Hero pane's fill, or the Library Hero overlay's sheet.
    f.render_widget(
        Block::default()
            .style(Style::default().bg(palette::surface_colors(pane_surface, false).fill)),
        Rect {
            y: next_row,
            height: 1.min(area.bottom().saturating_sub(next_row)),
            ..area
        },
    );
    let panel = Rect {
        y: box_y,
        height: box_height,
        ..area
    };
    f.render_widget(
        Block::default().style(
            Style::default()
                .bg(palette::surface_colors(palette::Surface::MainContentBox, false).fill),
        ),
        panel,
    );
    let inner = Rect {
        x: panel.x + PANE_PAD_X,
        y: panel.y + PANE_PAD_Y,
        width: panel.width.saturating_sub(PANE_PAD_X * 2),
        height: panel.height.saturating_sub(PANE_PAD_Y * 2),
    };
    let viewport = inner.height as usize;
    let max_offset = inner_rows.saturating_sub(viewport);
    let offset = scroll_offset.min(max_offset);
    paint_overview_text(f, inner, overview, offset);
    if has_credits_gap {
        // The separator follows the overview in the same flow; the blank row
        // below it is the flow's next row.
        if let Some(y) = visible_overview_row(inner, text_rows, offset) {
            crate::app::render::components::widgets::render_block_separator(
                f,
                Rect {
                    x: inner.x,
                    y,
                    width: inner.width,
                    height: 1,
                },
            );
        }
    }
    paint_overview_credits(f, inner, credits, text_rows, gap_rows, offset);
    if max_offset > 0 && viewport > 0 {
        render_right_scrollbar_with_viewport(
            f,
            panel,
            inner_rows,
            viewport,
            offset,
            palette::SCROLLBAR,
        );
    }
    Some(OverviewPaint {
        bottom: panel.bottom(),
        rect: panel,
        content_length: inner_rows,
        viewport,
    })
}

fn overview_box_height(
    inner_rows: usize,
    room: u16,
    has_workspace: bool,
    terminal_height: u16,
) -> u16 {
    let natural = u16::try_from(inner_rows.max(1))
        .unwrap_or(u16::MAX)
        .saturating_add(PANE_PAD_Y * 2);
    let box_height = if has_workspace {
        natural.min(room)
    } else {
        room
    };
    if crate::app::components::library_panel::hero_header::short_pane(terminal_height) {
        box_height.min(OVERVIEW_SHORT_PANE_MAX_ROWS + PANE_PAD_Y * 2)
    } else {
        box_height
    }
}

// One virtual row space for the whole flow: the overview occupies rows
// `0..text_rows`, then the separator and its blank row, then the table.
// A flow row outside the scrolled viewport has no screen coordinate.
fn visible_overview_row(inner: Rect, flow_row: usize, offset: usize) -> Option<u16> {
    let screen_row = u16::try_from(flow_row.checked_sub(offset)?).ok()?;
    let y = inner.y.saturating_add(screen_row);
    (y < inner.bottom()).then_some(y)
}

fn paint_overview_text(f: &mut Frame, inner: Rect, overview: Option<&str>, offset: usize) {
    let Some(text) = overview else {
        return;
    };
    // One row per wrapped line, empty lines included, so the flow's row
    // positions stay exact while it scrolls.
    let wrapped = textwrap::wrap(text, (inner.width as usize).saturating_sub(1).max(1));
    for (index, line) in wrapped.iter().enumerate() {
        let Some(y) = visible_overview_row(inner, index, offset) else {
            continue;
        };
        if line.is_empty() {
            continue;
        }
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                line.as_ref(),
                Style::default().fg(palette::TEXT_EMPHASIS),
            ))),
            Rect {
                x: inner.x,
                y,
                width: inner.width,
                height: 1,
            },
        );
    }
}

fn paint_overview_credits(
    f: &mut Frame,
    inner: Rect,
    credits: Option<&[HeroCredit]>,
    text_rows: usize,
    gap_rows: usize,
    offset: usize,
) {
    let Some(credits) = credits else {
        return;
    };
    let credits_start = text_rows + gap_rows;
    let first_visible = offset.saturating_sub(credits_start).min(credits.len());
    if first_visible < credits.len() {
        if let Some(y) = visible_overview_row(inner, credits_start + first_visible, offset) {
            paint_credits_from(
                f,
                Rect {
                    y,
                    height: inner.bottom().saturating_sub(y),
                    ..inner
                },
                credits,
                first_visible,
            );
        }
    }
}

fn paint_credits_from(f: &mut Frame, area: Rect, credits: &[HeroCredit], row_offset: usize) {
    // Column geometry is a property of the whole table, not the visible page.
    // Keep enough room for the role column even when one name is unusually long.
    let name_width = credits
        .iter()
        .map(|c| UnicodeWidthStr::width(c.name.as_str()))
        .max()
        .unwrap_or(0)
        .min(usize::from(area.width.saturating_sub(MIN_ROLE_WIDTH)));
    let name_width = u16::try_from(name_width).unwrap_or(u16::MAX);
    let role_start = area.x.saturating_add(name_width).saturating_add(2);
    for (i, credit) in credits.iter().skip(row_offset).enumerate() {
        let y = area.y.saturating_add(u16::try_from(i).unwrap_or(u16::MAX));
        if y >= area.bottom() {
            break;
        }
        if (row_offset + i) % 2 == 1 {
            f.render_widget(
                Paragraph::new("").style(Style::default().bg(palette::HERO_CREDITS_STRIPE)),
                Rect {
                    x: area.x,
                    y,
                    width: area.width,
                    height: 1,
                },
            );
        }
        let name_width = role_start.saturating_sub(area.x).saturating_sub(2);
        f.render_widget(
            Paragraph::new(credit.name.as_str())
                .style(Style::default().fg(palette::HERO_CREDITS_NAME)),
            Rect {
                x: area.x,
                y,
                width: name_width.min(area.width),
                height: 1,
            },
        );
        if role_start < area.right() {
            let available_width = area.right().saturating_sub(role_start) as usize;
            let role = trunc_str(&credit.role, available_width);
            let rendered_role_width =
                u16::try_from(UnicodeWidthStr::width(role.as_str())).unwrap_or(u16::MAX);
            let role_x = area.right().saturating_sub(rendered_role_width);
            f.render_widget(
                Paragraph::new(role).style(Style::default().fg(palette::TEXT_EMPHASIS)),
                Rect {
                    x: role_x,
                    y,
                    width: rendered_role_width,
                    height: 1,
                },
            );
        }
    }
}

fn contains_control(text: &str) -> bool {
    text.chars()
        .any(crate::app::infra::text_safety::is_control_char)
}

pub(in crate::app) fn sanitize_url(url: &str) -> Option<&str> {
    if url.is_empty() || contains_control(url) {
        return None;
    }
    let scheme = url.split_once(':')?.0;
    (scheme.eq_ignore_ascii_case("http") || scheme.eq_ignore_ascii_case("https")).then_some(url)
}

fn sanitize_label(label: &str) -> Option<&str> {
    (!contains_control(label)).then_some(label)
}

fn underline_link_cells(f: &mut Frame, cell: Rect, offset: usize, label_width: usize) {
    for x in offset..offset + label_width {
        let Some(x) = u16::try_from(x).ok() else {
            continue;
        };
        let Some(cell) = f.buffer_mut().cell_mut((cell.x.saturating_add(x), cell.y)) else {
            continue;
        };
        cell.set_style(
            cell.style()
                .fg(palette::TEXT_METADATA)
                .add_modifier(ratatui::style::Modifier::UNDERLINED),
        );
    }
}

/// Registers hover underline and hit regions for the links row painted in
/// `cell` (a full-width stacked row or one landscape-grid cell). `bottom`
/// clips rows outside the text block. A label run wider than the cell
/// registers nothing: the painter truncates it, so full-label hits would
/// drift from the pixels.
fn paint_link_hit_row(
    f: &mut Frame,
    cell: Rect,
    bottom: u16,
    facts: &HeroFacts,
    hovered_link: Option<usize>,
    link_hits: &mut HitRegions<usize>,
) {
    let mut offset = 0usize;
    for (link_index, link) in facts.links.iter().enumerate() {
        let label_width = UnicodeWidthStr::width(link.name.as_str());
        if label_width > 0
            && cell.y < bottom
            && offset + label_width <= cell.width as usize
            && sanitize_url(&link.url).is_some()
            && sanitize_label(&link.name).is_some()
        {
            if hovered_link == Some(link_index) {
                underline_link_cells(f, cell, offset, label_width);
            }
            link_hits.push(
                Rect::new(
                    cell.x
                        .saturating_add(u16::try_from(offset).unwrap_or(u16::MAX)),
                    cell.y,
                    u16::try_from(label_width).unwrap_or(u16::MAX),
                    1,
                ),
                link_index,
            );
        }
        offset += label_width + 1;
    }
}

fn joined_links(facts: &HeroFacts) -> String {
    facts
        .links
        .iter()
        .map(|l| l.name.as_str())
        .collect::<Vec<_>>()
        .join("|")
}
pub(in crate::app) fn overlay_links(
    f: &mut Frame,
    area: Rect,
    facts: &HeroFacts,
    hovered_link: Option<usize>,
    link_hits: &mut HitRegions<usize>,
) {
    link_hits.clear();
    let joined = joined_links(facts);
    if joined.is_empty() {
        return;
    }
    let Some(index) = facts.meta_rows.iter().position(|row| row == &joined) else {
        return;
    };
    let wrap = (area.width as usize).saturating_sub(1).max(1);
    let mut y = area.y;
    for line in facts.entries().take(index + 2) {
        let lines = textwrap::wrap(line, wrap);
        if line == &joined {
            if lines.len() != 1 {
                return;
            }
            paint_link_hit_row(
                f,
                Rect::new(area.x, y, area.width, 1),
                area.bottom(),
                facts,
                hovered_link,
                link_hits,
            );
            return;
        }
        y = y.saturating_add(u16::try_from(lines.len()).unwrap_or(u16::MAX));
    }
}

/// Link hits for the landscape two-column grid (`hero_header.rs`): the
/// links row keeps its entry position among the non-empty title/meta
/// entries, so entry `pos` paints at grid row `pos / 2`, left cell for
/// even `pos` and right-aligned right cell for odd. `left_w` is the
/// grid's left column width; the right column takes the rest of `area`.
/// A links row wider than its cell registers nothing (the painter
/// truncates it), mirroring the stacked walk's bail on a wrapped row.
pub(in crate::app) fn overlay_links_grid(
    f: &mut Frame,
    area: Rect,
    left_w: u16,
    facts: &HeroFacts,
    hovered_link: Option<usize>,
    link_hits: &mut HitRegions<usize>,
) {
    link_hits.clear();
    let joined = joined_links(facts);
    if joined.is_empty() {
        return;
    }
    let Some(index) = facts.meta_rows.iter().position(|row| row == &joined) else {
        return;
    };
    let pos = facts
        .entries()
        .take(index + 2)
        .filter(|line| !line.is_empty())
        .count()
        .saturating_sub(1);
    let y = area
        .y
        .saturating_add(u16::try_from(pos / 2).unwrap_or(u16::MAX));
    let cell = if pos % 2 == 0 {
        Rect::new(area.x, y, left_w, 1)
    } else {
        Rect::new(
            area.x.saturating_add(left_w),
            y,
            area.width.saturating_sub(left_w),
            1,
        )
    };
    if joined.width() > cell.width as usize {
        return;
    }
    // The right column is right-aligned: the label run starts after the
    // cell padding, so hits open at the painted text, not the cell edge.
    let pad = if pos % 2 == 0 {
        0
    } else {
        cell.width
            .saturating_sub(u16::try_from(joined.width()).unwrap_or(u16::MAX))
    };
    let cell = Rect::new(
        cell.x.saturating_add(pad),
        cell.y,
        cell.width.saturating_sub(pad),
        1,
    );
    paint_link_hit_row(f, cell, area.bottom(), facts, hovered_link, link_hits);
}
