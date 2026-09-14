//! Wide Hero Main content box: overview text and the Movie credits table.
use std::num::NonZeroU16;

use ratatui::buffer::CellDiffOption;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::widgets::{Block, Paragraph};
use ratatui::Frame;
use unicode_width::UnicodeWidthStr;

use crate::app::palette;
use crate::app::render::{paint_wide_hero_text, WrappedHeroLine, PANE_PAD_X, PANE_PAD_Y};

use super::content::{HeroContent, HeroCredit, HeroFacts};

pub(in crate::app) fn paint_overview_box(
    f: &mut Frame,
    area: Rect,
    next_row: u16,
    content: &HeroContent<'_>,
) -> Option<u16> {
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
    let inner_rows = text_rows + usize::from(overview.is_some() && credits.is_some()) + credit_rows;
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
    f.render_widget(
        Block::default().style(
            Style::default().bg(palette::surface_colors(palette::Surface::HeroPane, false).fill),
        ),
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
    let mut row = inner.y;
    if let Some(text) = overview {
        paint_wide_hero_text(
            f,
            Rect { y: row, ..inner },
            &[WrappedHeroLine {
                text,
                style: Style::default().fg(palette::TEXT_EMPHASIS),
            }],
        );
        row = row.saturating_add(text_rows as u16);
    }
    if overview.is_some() && credits.is_some() {
        row = row.saturating_add(1);
    }
    if let Some(credits) = credits {
        paint_credits(f, Rect { y: row, ..inner }, credits);
    }
    Some(panel.bottom())
}

fn paint_credits(f: &mut Frame, area: Rect, credits: &[HeroCredit]) {
    let name_width = credits
        .iter()
        .map(|c| UnicodeWidthStr::width(c.name.as_str()))
        .max()
        .unwrap_or(0) as u16;
    let role_x = area.x.saturating_add(name_width).saturating_add(2);
    for (i, credit) in credits.iter().enumerate() {
        let y = area.y.saturating_add(i as u16);
        if y >= area.bottom() {
            break;
        }
        let name_width = role_x.saturating_sub(area.x).saturating_sub(2);
        f.render_widget(
            Paragraph::new(credit.name.as_str()).style(Style::default().fg(palette::TEXT_EMPHASIS)),
            Rect {
                x: area.x,
                y,
                width: name_width.min(area.width),
                height: 1,
            },
        );
        if role_x < area.right() {
            let role_width = area.right().saturating_sub(role_x) as usize;
            let role = truncate_ellipsis(&credit.role, role_width);
            f.render_widget(
                Paragraph::new(role).style(Style::default().fg(palette::TEXT_EMPHASIS)),
                Rect {
                    x: role_x,
                    y,
                    width: area.right() - role_x,
                    height: 1,
                },
            );
        }
    }
}

fn truncate_ellipsis(text: &str, width: usize) -> String {
    if UnicodeWidthStr::width(text) <= width {
        return text.to_owned();
    }
    if width == 0 {
        return String::new();
    }
    if width == 1 {
        return "…".into();
    }
    let mut out = String::new();
    let mut used = 0;
    for ch in text.chars() {
        let w = UnicodeWidthStr::width(ch.to_string().as_str());
        if used + w > width - 1 {
            break;
        }
        out.push(ch);
        used += w;
    }
    out.push('…');
    out
}

pub(in crate::app) fn sanitize_url(url: &str) -> Option<&str> {
    if url.is_empty() || url.bytes().any(|b| b.is_ascii_control()) {
        return None;
    }
    let scheme = url.split_once(":")?.0;
    if scheme.eq_ignore_ascii_case("http") || scheme.eq_ignore_ascii_case("https") {
        Some(url)
    } else {
        None
    }
}

pub(in crate::app) fn hyperlinks_supported(term_program: Option<&str>, term: Option<&str>) -> bool {
    matches!(
        term_program,
        Some("kitty" | "iTerm.app" | "WezTerm" | "Windows Terminal")
    ) || matches!(term, Some(t) if t == "foot" || t.starts_with("xterm-kitty") || t.starts_with("vte-"))
}

pub(in crate::app) fn overlay_links(f: &mut Frame, area: Rect, facts: &HeroFacts) {
    if !hyperlinks_supported(
        std::env::var("TERM_PROGRAM").ok().as_deref(),
        std::env::var("TERM").ok().as_deref(),
    ) {
        return;
    }
    let joined = facts
        .links
        .iter()
        .map(|l| l.name.as_str())
        .collect::<Vec<_>>()
        .join("  ");
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
        .take(index + 1)
    {
        let lines = textwrap::wrap(line, wrap);
        if line == &joined {
            if lines.len() != 1 {
                return;
            }
            let mut offset = 0usize;
            for link in &facts.links {
                let label_width = UnicodeWidthStr::width(link.name.as_str());
                if label_width > 0 && offset + label_width <= area.width as usize {
                    if let Some(url) = sanitize_url(&link.url) {
                        let symbol = format!("\x1b]8;;{url}\x1b\\{}\x1b]8;;\x1b\\", link.name);
                        if let Some(cell) = f.buffer_mut().cell_mut((area.x + offset as u16, y)) {
                            if let Some(width) = NonZeroU16::new(label_width as u16) {
                                cell.set_symbol(&symbol)
                                    .set_diff_option(CellDiffOption::ForcedWidth(width));
                            }
                        }
                    }
                }
                offset += label_width + 2;
            }
            return;
        }
        y = y.saturating_add(lines.len() as u16);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitizer_accepts_only_web_urls_without_controls() {
        assert_eq!(
            sanitize_url("https://example.test/a"),
            Some("https://example.test/a")
        );
        assert_eq!(sanitize_url("ftp://example.test"), None);
        assert_eq!(sanitize_url("https://example.test/\n"), None);
        assert_eq!(sanitize_url(""), None);
    }

    #[test]
    fn hyperlink_capability_defaults_to_unsupported() {
        assert!(hyperlinks_supported(Some("kitty"), None));
        assert!(hyperlinks_supported(Some("WezTerm"), None));
        assert!(hyperlinks_supported(None, Some("foot")));
        assert!(hyperlinks_supported(None, Some("vte-256color")));
        assert!(!hyperlinks_supported(None, None));
        assert!(!hyperlinks_supported(Some("xterm"), Some("xterm-256color")));
    }
}
