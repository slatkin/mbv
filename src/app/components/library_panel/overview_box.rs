//! Wide Hero Main content box: overview text and the Movie credits table.
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};

use crate::app::render::components::widgets::render_right_scrollbar_inside;
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
    let text_rows = overview
        .map(|text| {
            textwrap::wrap(
                text,
                (area.width.saturating_sub(PANE_PAD_X * 2) as usize)
                    .saturating_sub(1)
                    .max(1),
            )
            .len()
        })
        .unwrap_or(0);
    let credit_rows = credits.map(|rows| rows.len()).unwrap_or(0);
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
    let natural = (inner_rows.max(1) as u16).saturating_add(PANE_PAD_Y * 2);
    let box_height = if content.workspace.is_some() {
        natural.min(room)
    } else {
        room
    };
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
    // One virtual row space for the whole flow: the overview occupies rows
    // `0..text_rows`, then the separator and its blank row, then the table.
    // `visible_row` maps a flow row to the screen row the current offset
    // puts it on, or `None` while it is scrolled out of the viewport.
    let visible_row = |flow_row: usize| -> Option<u16> {
        let screen_row = u16::try_from(flow_row.checked_sub(offset)?).ok()?;
        let y = inner.y.saturating_add(screen_row);
        (y < inner.bottom()).then_some(y)
    };
    if let Some(text) = overview {
        // One row per wrapped line, empty lines included, so the flow's row
        // positions stay exact while it scrolls.
        let wrapped = textwrap::wrap(text, (inner.width as usize).saturating_sub(1).max(1));
        for (index, line) in wrapped.iter().enumerate() {
            let Some(y) = visible_row(index) else {
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
    if has_credits_gap {
        // The separator follows the overview in the same flow; the blank row
        // below it is the flow's next row.
        if let Some(y) = visible_row(text_rows) {
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
    if let Some(credits) = credits {
        let credits_start = text_rows + gap_rows;
        let first_visible = offset.saturating_sub(credits_start).min(credit_rows);
        if first_visible < credit_rows {
            if let Some(y) = visible_row(credits_start + first_visible) {
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
    if max_offset > 0 && viewport > 0 {
        render_right_scrollbar_inside(
            f,
            panel,
            inner_rows,
            viewport,
            offset,
            palette::TEXT_METADATA,
        );
    }
    Some(OverviewPaint {
        bottom: panel.bottom(),
        rect: panel,
        content_length: inner_rows,
        viewport,
    })
}

fn paint_credits_from(f: &mut Frame, area: Rect, credits: &[HeroCredit], row_offset: usize) {
    // Column geometry is a property of the whole table, not the visible page.
    let name_width = credits
        .iter()
        .map(|c| UnicodeWidthStr::width(c.name.as_str()))
        .max()
        .unwrap_or(0) as u16;
    // Keep enough room for the role column even when one name is unusually long.
    const MIN_ROLE_WIDTH: u16 = 8;
    let name_width = name_width.min(area.width.saturating_sub(MIN_ROLE_WIDTH));
    let role_start = area.x.saturating_add(name_width).saturating_add(2);
    for (i, credit) in credits.iter().skip(row_offset).enumerate() {
        let y = area.y.saturating_add(i as u16);
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
            let rendered_role_width = UnicodeWidthStr::width(role.as_str()) as u16;
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
    text.chars().any(crate::app::text_safety::is_control_char)
}

pub(in crate::app) fn sanitize_url(url: &str) -> Option<&str> {
    if url.is_empty() || contains_control(url) {
        return None;
    }
    let scheme = url.split_once(":")?.0;
    if scheme.eq_ignore_ascii_case("http") || scheme.eq_ignore_ascii_case("https") {
        Some(url)
    } else {
        None
    }
}

fn sanitize_label(label: &str) -> Option<&str> {
    (!contains_control(label)).then_some(label)
}

pub(in crate::app) fn overlay_links(
    f: &mut Frame,
    area: Rect,
    facts: &HeroFacts,
    hovered_link: Option<usize>,
    link_hits: &mut HitRegions<usize>,
) {
    link_hits.clear();
    let joined = facts
        .links
        .iter()
        .map(|l| l.name.as_str())
        .collect::<Vec<_>>()
        .join("|");
    if joined.is_empty() {
        return;
    }
    let Some(index) = facts.meta_rows.iter().position(|row| row == &joined) else {
        return;
    };
    let wrap = (area.width as usize).saturating_sub(1).max(1);
    let mut y = area.y;
    for line in std::iter::once(&facts.title)
        .chain(facts.meta_rows.iter())
        .take(index + 2)
    {
        let lines = textwrap::wrap(line, wrap);
        if line == &joined {
            if lines.len() != 1 {
                return;
            }
            let mut offset = 0usize;
            for (link_index, link) in facts.links.iter().enumerate() {
                let label_width = UnicodeWidthStr::width(link.name.as_str());
                if label_width > 0
                    && y < area.bottom()
                    && offset + label_width <= area.width as usize
                    && sanitize_url(&link.url).is_some()
                    && sanitize_label(&link.name).is_some()
                {
                    if hovered_link == Some(link_index) {
                        for x in offset..offset + label_width {
                            if let Some(cell) = f.buffer_mut().cell_mut((area.x + x as u16, y)) {
                                cell.set_style(
                                    cell.style()
                                        .fg(palette::TEXT_METADATA)
                                        .add_modifier(ratatui::style::Modifier::UNDERLINED),
                                );
                            }
                        }
                    }
                    link_hits.push(
                        Rect::new(area.x + offset as u16, y, label_width as u16, 1),
                        link_index,
                    );
                }
                offset += label_width + 1;
            }
            return;
        }
        y = y.saturating_add(lines.len() as u16);
    }
}

#[cfg(test)]
#[path = "overview_box_layout_tests.rs"]
mod overview_box_layout_tests;

#[cfg(test)]
#[path = "overview_box_credits_tests.rs"]
mod overview_box_credits_tests;
