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
        paint_credits(
            f,
            Rect {
                y: row,
                height: inner.bottom().saturating_sub(row),
                ..inner
            },
            credits,
        );
    }
    Some(panel.bottom())
}

fn paint_credits(f: &mut Frame, area: Rect, credits: &[HeroCredit]) {
    let name_width = credits
        .iter()
        .map(|c| UnicodeWidthStr::width(c.name.as_str()))
        .max()
        .unwrap_or(0) as u16;
    // Keep enough room for the role column even when one name is unusually long.
    const MIN_ROLE_WIDTH: u16 = 8;
    let name_width = name_width.min(area.width.saturating_sub(MIN_ROLE_WIDTH));
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

fn contains_control(text: &str) -> bool {
    text.chars()
        .any(|ch| ch.is_ascii_control() || matches!(ch, '\u{80}'..='\u{9f}'))
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

pub(in crate::app) fn hyperlinks_supported(term_program: Option<&str>, term: Option<&str>) -> bool {
    matches!(
        term_program,
        Some("kitty" | "iTerm.app" | "WezTerm" | "Windows Terminal")
    ) || matches!(term, Some(t) if t == "foot" || t.starts_with("xterm-kitty") || t.starts_with("vte-"))
}

pub(in crate::app) fn overlay_links(
    f: &mut Frame,
    area: Rect,
    facts: &HeroFacts,
    hyperlink_capable: bool,
) {
    if !hyperlink_capable {
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
        .take(index + 2)
    {
        let lines = textwrap::wrap(line, wrap);
        if line == &joined {
            if lines.len() != 1 {
                return;
            }
            let mut offset = 0usize;
            for link in &facts.links {
                let label_width = UnicodeWidthStr::width(link.name.as_str());
                if label_width > 0
                    && y < area.bottom()
                    && offset + label_width <= area.width as usize
                {
                    if let (Some(url), Some(label)) =
                        (sanitize_url(&link.url), sanitize_label(&link.name))
                    {
                        let symbol = format!("\x1b]8;;{url}\x1b\\{label}\x1b]8;;\x1b\\");
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
    use crate::app::components::library_panel::content::{HeroArtwork, HeroLink};

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

    #[test]
    fn sanitizer_rejects_control_bytes_in_urls_and_labels() {
        assert_eq!(sanitize_url("https://example.test/\n"), None);
        assert_eq!(sanitize_url("https://example.test/\u{0085}"), None);
        assert_eq!(sanitize_label("IMDb\n"), None);
        assert_eq!(sanitize_label("IMDb\u{009b}"), None);
    }

    #[test]
    fn supported_link_overlays_forced_width_escape_cell_on_links_row() {
        let mut terminal =
            ratatui::Terminal::new(ratatui::backend::TestBackend::new(30, 3)).unwrap();
        let facts = HeroFacts {
            title: "Title".into(),
            meta_rows: vec!["IMDb".into()],
            links: vec![HeroLink {
                name: "IMDb".into(),
                url: "https://example.test".into(),
            }],
            artwork: HeroArtwork {
                shape: super::super::content::ArtworkShape::Landscape,
                source: None,
                image: super::super::content::HeroImageState::None,
            },
        };
        terminal
            .draw(|f| {
                paint_wide_hero_text(
                    f,
                    Rect::new(0, 0, 30, 3),
                    &[
                        WrappedHeroLine {
                            text: "Title",
                            style: Style::default(),
                        },
                        WrappedHeroLine {
                            text: "IMDb",
                            style: Style::default(),
                        },
                    ],
                );
                overlay_links(f, Rect::new(0, 0, 30, 3), &facts, true);
            })
            .unwrap();
        let cell = &terminal.backend().buffer()[(0, 1)];
        assert!(cell.symbol().contains("\x1b]8;;https://example.test"));
        assert_eq!(
            cell.diff_option,
            CellDiffOption::ForcedWidth(NonZeroU16::new(4).unwrap())
        );
    }

    #[test]
    fn unsupported_link_is_plain_text() {
        let mut terminal =
            ratatui::Terminal::new(ratatui::backend::TestBackend::new(30, 2)).unwrap();
        let facts = HeroFacts {
            title: "Title".into(),
            meta_rows: vec!["IMDb".into()],
            links: vec![HeroLink {
                name: "IMDb".into(),
                url: "https://example.test".into(),
            }],
            artwork: HeroArtwork {
                shape: super::super::content::ArtworkShape::Landscape,
                source: None,
                image: super::super::content::HeroImageState::None,
            },
        };
        terminal
            .draw(|f| overlay_links(f, Rect::new(0, 0, 30, 2), &facts, false))
            .unwrap();
        assert_eq!(terminal.backend().buffer()[(0, 1)].symbol(), " ");
    }

    #[test]
    fn credits_clip_and_role_column_survives_long_name() {
        let mut terminal =
            ratatui::Terminal::new(ratatui::backend::TestBackend::new(24, 3)).unwrap();
        terminal
            .draw(|f| {
                paint_credits(
                    f,
                    Rect::new(0, 0, 24, 1),
                    &[HeroCredit {
                        name: "A very long credit name".into(),
                        role: "Actor".into(),
                    }],
                );
            })
            .unwrap();
        assert_eq!(terminal.backend().buffer()[(0, 1)].symbol(), " ");
        let row = (0..24)
            .map(|x| terminal.backend().buffer()[(x, 0)].symbol())
            .collect::<String>();
        assert!(
            row.contains("Actor"),
            "role column survives long name: {row:?}"
        );
    }

    #[test]
    fn link_outside_box_is_not_overlaid() {
        let mut terminal =
            ratatui::Terminal::new(ratatui::backend::TestBackend::new(20, 1)).unwrap();
        let facts = HeroFacts {
            title: "Title".into(),
            meta_rows: vec!["Link".into()],
            links: vec![HeroLink {
                name: "Link".into(),
                url: "https://example.test".into(),
            }],
            artwork: HeroArtwork {
                shape: super::super::content::ArtworkShape::Landscape,
                source: None,
                image: super::super::content::HeroImageState::None,
            },
        };
        terminal
            .draw(|f| overlay_links(f, Rect::new(0, 0, 20, 1), &facts, true))
            .unwrap();
        assert!(!terminal.backend().buffer()[(0, 0)]
            .symbol()
            .contains("\x1b]8"));
    }
}
