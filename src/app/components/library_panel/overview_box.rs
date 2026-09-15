//! Wide Hero Main content box: overview text and the Movie credits table.
use std::num::NonZeroU16;

use ratatui::buffer::CellDiffOption;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::widgets::{Block, Paragraph};

use crate::app::render::components::widgets::render_right_scrollbar_inside;
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
    scroll_offset: usize,
) -> Option<(u16, (Rect, usize, usize))> {
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
    let inner_rows = text_rows
        + if overview.is_some() && credits.is_some() {
            2
        } else {
            0
        }
        + credit_rows;
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
    let max_offset = inner_rows.saturating_sub(inner.height as usize);
    let offset = scroll_offset.min(max_offset);
    let mut logical_row = 0usize;
    if let Some(text) = overview {
        let wrapped = textwrap::wrap(text, (inner.width as usize).saturating_sub(1).max(1));
        let lines: Vec<WrappedHeroLine<'_>> = wrapped[offset.min(text_rows)..]
            .iter()
            .map(|line| WrappedHeroLine {
                text: line.as_ref(),
                style: Style::default().fg(palette::TEXT_EMPHASIS),
            })
            .collect();
        let visible = lines.len().min(inner.height as usize);
        if visible > 0 {
            paint_wide_hero_text(
                f,
                Rect {
                    y: inner.y,
                    height: visible as u16,
                    ..inner
                },
                &lines[..visible],
            );
        }
        logical_row += text_rows;
    }
    if overview.is_some() && credits.is_some() {
        // Keep one blank row below the overview, then paint the separator.
        logical_row += 1;
        if logical_row >= offset && logical_row - offset < inner.height as usize {
            let y = inner.y + (logical_row - offset) as u16;
            let separator = "▁".repeat(inner.width as usize);
            f.render_widget(
                Paragraph::new(separator)
                    .style(Style::default().fg(palette::HERO_OVERVIEW_SEPARATOR)),
                Rect {
                    x: inner.x,
                    y,
                    width: inner.width,
                    height: 1,
                },
            );
        }
        logical_row += 1;
    }
    if let Some(credits) = credits {
        let start = logical_row;
        if start < inner_rows {
            let skip = offset.saturating_sub(start);
            let y = inner.y + start.saturating_sub(offset) as u16;
            if y < inner.bottom() {
                paint_credits_from(
                    f,
                    Rect {
                        y,
                        height: inner.bottom().saturating_sub(y),
                        ..inner
                    },
                    &credits[skip.min(credits.len())..],
                    skip,
                );
            }
        }
    }
    if max_offset > 0 {
        render_right_scrollbar_inside(
            f,
            panel,
            inner_rows,
            inner.height as usize,
            offset,
            palette::TEXT_METADATA,
        );
    }
    Some((panel.bottom(), (panel, inner_rows, inner.height as usize)))
}

fn paint_credits(f: &mut Frame, area: Rect, credits: &[HeroCredit]) {
    paint_credits_from(f, area, credits, 0);
}

fn paint_credits_from(f: &mut Frame, area: Rect, credits: &[HeroCredit], row_offset: usize) {
    let name_width = credits
        .iter()
        .map(|c| UnicodeWidthStr::width(c.name.as_str()))
        .max()
        .unwrap_or(0) as u16;
    // Keep enough room for the role column even when one name is unusually long.
    const MIN_ROLE_WIDTH: u16 = 8;
    let name_width = name_width.min(area.width.saturating_sub(MIN_ROLE_WIDTH));
    let role_start = area.x.saturating_add(name_width).saturating_add(2);
    for (i, credit) in credits.iter().enumerate() {
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
            Paragraph::new(credit.name.as_str()).style(Style::default().fg(palette::TEXT_EMPHASIS)),
            Rect {
                x: area.x,
                y,
                width: name_width.min(area.width),
                height: 1,
            },
        );
        if role_start < area.right() {
            let available_width = area.right().saturating_sub(role_start) as usize;
            let role = truncate_ellipsis(&credit.role, available_width);
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
    use crate::app::components::library_panel::content::{
        ArtworkShape, HeroArtwork, HeroLink, PanelList, Workspace,
    };
    use crate::app::components::library_panel::hero_header::{
        hero_artwork_box, paint_hero_pane_content,
    };
    use crate::app::palette;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    /// A stub `PanelList` so a Workspace can be present for the overview box.
    struct NoopList;
    impl PanelList for NoopList {
        fn set_presentation(
            &mut self,
            _presentation: crate::app::components::media_list::Presentation,
            _viewport_height: usize,
        ) {
        }

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
            links: Vec::new(),
            artwork: HeroArtwork {
                shape,
                source: None,
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
        terminal
            .draw(|f| {
                paint_hero_pane_content(f, area, pane, false, 0);
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

    fn text_fg(
        buf: &ratatui::buffer::Buffer,
        area: Rect,
        needle: &str,
    ) -> Option<ratatui::style::Color> {
        for y in area.top()..area.bottom() {
            let mut line = String::new();
            let mut cells = Vec::new();
            for x in area.left()..area.right() {
                line.push_str(buf[(x, y)].symbol());
                cells.push(buf[(x, y)].style().fg);
            }
            if let Some(offset) = line.find(needle) {
                return cells.get(offset).copied().flatten();
            }
        }
        None
    }

    #[test]
    fn overview_box_presents_only_with_overview_text() {
        let box_fill = palette::surface_colors(palette::Surface::MainContentBox, false).fill;
        let pane = content(ArtworkShape::Landscape);
        let artwork = hero_artwork_box(AREA, &pane.facts, pane.workspace.is_some());
        let below = Rect {
            y: artwork.bottom() + 3,
            height: AREA.bottom().saturating_sub(artwork.bottom() + 3),
            ..AREA
        };
        // Without overview: no Main content box surface below the header.
        let buf = draw_pane(AREA.width, AREA.height, &pane);
        assert!(
            (below.top()..below.bottom()).all(|y| buf[(AREA.x + 2, y)].bg != box_fill),
            "no overview box without overview text"
        );
        // With overview text: the Main content box renders below the header
        // and its text paints inside.
        let with = HeroContent {
            facts: facts(ArtworkShape::Landscape),
            overview: Some("A very long overview.".into()),
            credits: None,
            workspace: None,
        };
        let buf = draw_pane(AREA.width, AREA.height, &with);
        assert!(
            (below.top()..below.bottom()).any(|y| buf[(AREA.x + 2, y)].bg == box_fill),
            "overview box painted below the header"
        );
        assert!(text_in(&buf, below, "A very long overview."));
        // Soft white at all times: the overview is body content, so the
        // hero pane's focus (from the Workspace) never dims it.
        assert_eq!(
            text_fg(&buf, below, "A very long overview."),
            Some(palette::TEXT_EMPHASIS),
            "unfocused overview text"
        );
        let mut workspace_list = NoopList;
        let focused = HeroContent {
            facts: facts(ArtworkShape::Landscape),
            overview: Some("A very long overview.".into()),
            credits: None,
            workspace: Some(Workspace {
                selector: None,
                list: &mut workspace_list,
                focused: true,
            }),
        };
        let buf = draw_pane(AREA.width, AREA.height, &focused);
        assert_eq!(
            text_fg(&buf, below, "A very long overview."),
            Some(palette::TEXT_EMPHASIS),
            "focused overview text"
        );
    }

    #[test]
    fn overview_box_keeps_a_blank_hero_pane_row_above_it() {
        let box_fill = palette::surface_colors(palette::Surface::MainContentBox, false).fill;
        let pane_fill = palette::surface_colors(palette::Surface::HeroPane, false).fill;
        assert_ne!(box_fill, pane_fill, "seam needs distinct fills");
        let with = HeroContent {
            facts: facts(ArtworkShape::Landscape),
            overview: Some("A very long overview.".into()),
            credits: None,
            workspace: None,
        };
        let buf = draw_pane(AREA.width, AREA.height, &with);
        // Start below the artwork box: the imageless placeholder shares the
        // box's resting fill, so a full-pane scan would catch row 0.
        let artwork = hero_artwork_box(AREA, &with.facts, with.workspace.is_some());
        let box_top =
            (artwork.bottom()..AREA.bottom()).find(|y| buf[(AREA.x + 2, *y)].bg == box_fill);
        assert!(box_top.is_some(), "overview box paints");
        let box_top = box_top.unwrap();
        assert!(box_top > AREA.top() + 1, "header text paints above the box");
        let gap = box_top - 1;
        assert_ne!(buf[(AREA.x + 2, gap)].bg, box_fill);
        assert_eq!(buf[(AREA.x + 2, gap)].bg, pane_fill);
        assert!(!text_in(
            &buf,
            Rect {
                y: gap,
                height: 1,
                ..AREA
            },
            "A very long overview."
        ));
    }

    #[test]
    fn overview_box_fills_the_pane_when_no_workspace_follows_it() {
        let box_fill = palette::surface_colors(palette::Surface::MainContentBox, false).fill;
        let with = HeroContent {
            facts: facts(ArtworkShape::Landscape),
            overview: Some("A very long overview.".into()),
            credits: None,
            workspace: None,
        };
        let buf = draw_pane(AREA.width, AREA.height, &with);
        // The bottom-most recessed box reaches the content area's last row:
        // the hero panel's own bottom padding row sits outside `area`.
        assert_eq!(buf[(AREA.x + 2, AREA.bottom() - 1)].bg, box_fill);

        // With a Workspace below it the overview is no longer the bottom-most
        // box, so it stays content-sized.
        let mut workspace_list = NoopList;
        let with_workspace = HeroContent {
            facts: facts(ArtworkShape::Landscape),
            overview: Some("A very long overview.".into()),
            credits: None,
            workspace: Some(Workspace {
                selector: None,
                list: &mut workspace_list,
                focused: false,
            }),
        };
        let buf = draw_pane(AREA.width, AREA.height, &with_workspace);
        assert_ne!(buf[(AREA.x + 2, AREA.bottom() - 1)].bg, box_fill);
    }

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
    fn overview_credits_have_blank_row_separator_and_zebra_stripes() {
        let content = HeroContent {
            facts: facts(ArtworkShape::Landscape),
            overview: Some("Overview".into()),
            credits: Some(vec![
                HeroCredit {
                    name: "One".into(),
                    role: "Actor".into(),
                },
                HeroCredit {
                    name: "Two".into(),
                    role: "Writer".into(),
                },
                HeroCredit {
                    name: "Three".into(),
                    role: "Composer".into(),
                },
            ]),
            workspace: None,
        };
        let mut terminal = Terminal::new(TestBackend::new(32, 12)).unwrap();
        terminal
            .draw(|f| {
                paint_overview_box(f, Rect::new(0, 0, 32, 12), 0, &content, 0);
            })
            .unwrap();
        let buffer = terminal.backend().buffer();
        let separator = (0..12).find(|y| buffer[(2, *y)].symbol() == "▁").unwrap();
        assert_eq!(buffer[(2, separator - 1)].symbol(), " ");
        assert_eq!(buffer[(2, separator)].fg, palette::HERO_OVERVIEW_SEPARATOR);
        assert_eq!(buffer[(3, separator)].symbol(), "▁");
        let stripe = palette::HERO_CREDITS_STRIPE;
        assert_ne!(buffer[(2, separator + 1)].bg, stripe);
        assert_eq!(buffer[(2, separator + 2)].bg, stripe);
        assert_ne!(buffer[(2, separator + 3)].bg, stripe);
    }

    #[test]
    fn credits_zebra_stripe_follows_table_offset() {
        let credits = [
            HeroCredit {
                name: "One".into(),
                role: "Actor".into(),
            },
            HeroCredit {
                name: "Two".into(),
                role: "Writer".into(),
            },
        ];
        let mut terminal = Terminal::new(TestBackend::new(24, 1)).unwrap();
        terminal
            .draw(|f| paint_credits_from(f, Rect::new(0, 0, 24, 1), &credits[1..], 1))
            .unwrap();
        assert_eq!(
            terminal.backend().buffer()[(0, 0)].bg,
            palette::HERO_CREDITS_STRIPE
        );
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
        assert_eq!(&row[19..24], "Actor", "role ends at the box edge: {row:?}");
        assert_eq!(&row[16..18], "  ", "name/role gap is retained: {row:?}");
    }

    #[test]
    fn credits_roles_use_the_full_remaining_width_and_right_align() {
        let mut terminal =
            ratatui::Terminal::new(ratatui::backend::TestBackend::new(36, 2)).unwrap();
        terminal
            .draw(|f| {
                paint_credits(
                    f,
                    Rect::new(2, 0, 30, 2),
                    &[
                        HeroCredit {
                            name: "Ann".into(),
                            role: "Lead".into(),
                        },
                        HeroCredit {
                            name: "Alexandra".into(),
                            role: "Cinematographer".into(),
                        },
                    ],
                );
            })
            .unwrap();
        let buf = terminal.backend().buffer();
        let first = (2..32).map(|x| buf[(x, 0)].symbol()).collect::<String>();
        let second = (2..32).map(|x| buf[(x, 1)].symbol()).collect::<String>();
        assert_eq!(
            &first[26..30],
            "Lead",
            "short role is right-aligned: {first:?}"
        );
        assert_eq!(
            &second[15..30],
            "Cinematographer",
            "long role uses the remaining width: {second:?}"
        );
        assert_eq!(
            &first[11..26],
            "               ",
            "gap spans to the edge: {first:?}"
        );
    }

    #[test]
    fn credits_truncate_roles_at_the_right_edge() {
        let mut terminal =
            ratatui::Terminal::new(ratatui::backend::TestBackend::new(24, 1)).unwrap();
        terminal
            .draw(|f| {
                paint_credits(
                    f,
                    Rect::new(0, 0, 24, 1),
                    &[HeroCredit {
                        name: "A very long credit name".into(),
                        role: "Director of Photography".into(),
                    }],
                );
            })
            .unwrap();
        let row = (0..24)
            .map(|x| terminal.backend().buffer()[(x, 0)].symbol())
            .collect::<String>();
        let role = row.chars().skip(18).collect::<String>();
        assert_eq!(
            role, "Direc…",
            "truncated role ends at the box edge: {row:?}"
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
