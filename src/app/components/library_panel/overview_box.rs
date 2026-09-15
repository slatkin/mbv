//! Wide Hero Main content box: overview text and the Movie credits table.
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::widgets::{Block, Paragraph};

use crate::app::render::components::widgets::render_right_scrollbar_inside;
use ratatui::Frame;
use unicode_width::UnicodeWidthStr;

use crate::app::components::mouse::hit::HitRegions;
use crate::app::palette;
use crate::app::render::{paint_wide_hero_text, WrappedHeroLine, PANE_PAD_X, PANE_PAD_Y};

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
    // The overview and separator are pinned; only the table consumes the
    // scrollable viewport.  The two fixed rows are the separator and its
    // following blank gap.
    let pinned_rows = text_rows
        + if has_credits_gap {
            OVERVIEW_CREDITS_GAP_ROWS
        } else {
            0
        };
    let inner_rows = pinned_rows + credit_rows;
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
    let table_viewport = (inner.height as usize).saturating_sub(pinned_rows);
    let max_offset = credit_rows.saturating_sub(table_viewport);
    let offset = scroll_offset.min(max_offset);
    if let Some(text) = overview {
        let wrapped = textwrap::wrap(text, (inner.width as usize).saturating_sub(1).max(1));
        let lines: Vec<WrappedHeroLine<'_>> = wrapped
            .iter()
            .map(|line| WrappedHeroLine {
                text: line.as_ref(),
                style: Style::default().fg(palette::TEXT_EMPHASIS),
            })
            .collect();
        // Keep pinned overview content inside the box when the pane is too
        // short to show the whole overview (and its table, if present).
        let visible_rows = (lines.len() as u16).min(inner.height);
        if visible_rows > 0 {
            paint_wide_hero_text(
                f,
                Rect {
                    y: inner.y,
                    height: visible_rows,
                    ..inner
                },
                &lines,
            );
        }
    }
    if has_credits_gap {
        // The separator is directly below the overview, with one blank row
        // below it; both remain pinned while the table scrolls.
        let y = inner.y + text_rows as u16;
        if y < inner.bottom() {
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
    }
    if let Some(credits) = credits {
        let y = inner.y + pinned_rows as u16;
        if y < inner.bottom() {
            paint_credits_from(
                f,
                Rect {
                    y,
                    height: inner.bottom().saturating_sub(y),
                    ..inner
                },
                credits,
                offset,
            );
        }
    }
    if max_offset > 0 && table_viewport > 0 {
        render_right_scrollbar_inside(
            f,
            panel,
            credit_rows,
            table_viewport,
            offset,
            palette::TEXT_METADATA,
        );
    }
    Some(OverviewPaint {
        bottom: panel.bottom(),
        rect: panel,
        content_length: credit_rows,
        viewport: table_viewport,
    })
}

fn paint_credits(f: &mut Frame, area: Rect, credits: &[HeroCredit]) {
    paint_credits_from(f, area, credits, 0);
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
                    if let Some(cell) = f.buffer_mut().cell_mut((area.x + offset as u16, y)) {
                        if hovered_link == Some(link_index) {
                            cell.set_style(
                                cell.style()
                                    .fg(palette::TEXT_METADATA)
                                    .add_modifier(ratatui::style::Modifier::UNDERLINED),
                            );
                        }
                        link_hits.push(
                            Rect::new(area.x + offset as u16, y, label_width as u16, 1),
                            link_index,
                        );
                    }
                }
                offset += label_width + 1;
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
                paint_hero_pane_content(f, area, pane, 0, None, &mut HitRegions::new());
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
    fn sanitizer_rejects_control_bytes_in_urls_and_labels() {
        assert_eq!(sanitize_url("https://example.test/\n"), None);
        assert_eq!(sanitize_url("https://example.test/\u{0085}"), None);
        assert_eq!(sanitize_label("IMDb\n"), None);
        assert_eq!(sanitize_label("IMDb\u{009b}"), None);
    }

    #[test]
    fn hovered_link_uses_foam_underline() {
        let mut terminal =
            ratatui::Terminal::new(ratatui::backend::TestBackend::new(30, 2)).unwrap();
        let facts = HeroFacts {
            title: "Title".into(),
            meta_rows: vec!["IMDb".into()],
            links: vec![HeroLink {
                name: "IMDb".into(),
                url: "https://imdb.test".into(),
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
                    Rect::new(0, 0, 30, 2),
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
                overlay_links(
                    f,
                    Rect::new(0, 0, 30, 2),
                    &facts,
                    Some(0),
                    &mut HitRegions::new(),
                );
            })
            .unwrap();
        let buffer = terminal.backend().buffer();
        assert_eq!(buffer[(0, 1)].style().fg, Some(palette::TEXT_METADATA));
        assert!(buffer[(0, 1)]
            .style()
            .add_modifier
            .contains(ratatui::style::Modifier::UNDERLINED));
        assert!(!buffer[(5, 1)]
            .style()
            .add_modifier
            .contains(ratatui::style::Modifier::UNDERLINED));

        terminal
            .draw(|f| {
                paint_wide_hero_text(
                    f,
                    Rect::new(0, 0, 30, 2),
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
                overlay_links(
                    f,
                    Rect::new(0, 0, 30, 2),
                    &facts,
                    None,
                    &mut HitRegions::new(),
                );
            })
            .unwrap();
        assert!(!terminal.backend().buffer()[(5, 1)]
            .symbol()
            .contains("\x1b[4;"));
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
        assert_eq!(buffer[(2, separator)].fg, palette::HERO_OVERVIEW_SEPARATOR);
        assert_eq!(buffer[(2, separator + 1)].symbol(), " ");
        let stripe = palette::HERO_CREDITS_STRIPE;
        assert_ne!(buffer[(2, separator + 2)].bg, stripe);
        assert_eq!(buffer[(2, separator + 3)].bg, stripe);
        assert_ne!(buffer[(2, separator + 4)].bg, stripe);
        assert_eq!(buffer[(2, separator + 2)].fg, palette::HERO_CREDITS_NAME);
        assert_eq!(buffer[(2, separator + 3)].fg, palette::HERO_CREDITS_NAME);
    }

    #[test]
    fn long_overview_is_clipped_to_inner_box_when_pane_is_short() {
        let content = HeroContent {
            facts: facts(ArtworkShape::Landscape),
            overview: Some("one two three four five six seven eight nine ten eleven twelve".into()),
            credits: Some(vec![HeroCredit {
                name: "Person".into(),
                role: "Actor".into(),
            }]),
            workspace: Some(Workspace {
                selector: None,
                list: &mut NoopList,
                focused: false,
            }),
        };
        let mut terminal = Terminal::new(TestBackend::new(32, 6)).unwrap();
        let mut metrics = None;
        terminal
            .draw(|f| metrics = paint_overview_box(f, Rect::new(0, 0, 32, 6), 0, &content, 0))
            .unwrap();
        let buffer = terminal.backend().buffer();
        let inner = Rect::new(2, 2, 28, 3);
        assert_eq!(metrics.unwrap().viewport, 0);
        assert!(
            (inner.bottom()..6).all(|y| {
                (inner.left()..inner.right()).all(|x| buffer[(x, y)].symbol() != "▁")
                    && (inner.left()..inner.right()).all(|x| buffer[(x, y)].symbol() != "Person")
            }),
            "separator and table never paint below inner bottom"
        );
    }

    #[test]
    fn pinned_overview_and_separator_stay_fixed_while_credits_scroll() {
        let credits = (0..12)
            .map(|i| HeroCredit {
                name: format!("Person {i}"),
                role: format!("Role {i}"),
            })
            .collect::<Vec<_>>();
        let content = HeroContent {
            facts: facts(ArtworkShape::Landscape),
            overview: Some("Overview text".into()),
            credits: Some(credits),
            workspace: Some(Workspace {
                selector: None,
                list: &mut NoopList,
                focused: false,
            }),
        };
        let draw = |offset| {
            let mut terminal = Terminal::new(TestBackend::new(32, 14)).unwrap();
            let mut paint = None;
            terminal
                .draw(|f| {
                    paint = paint_overview_box(f, Rect::new(0, 0, 32, 14), 0, &content, offset)
                })
                .unwrap();
            (terminal.backend().buffer().clone(), paint.unwrap())
        };
        let (at_start, metrics) = draw(0);
        let (at_end, _) = draw(metrics.content_length.saturating_sub(metrics.viewport));
        let separator_y = 3;
        assert_eq!(metrics.viewport, 8);
        for y in 2..=separator_y {
            for x in 0..31 {
                assert_eq!(at_start[(x, y)].symbol(), at_end[(x, y)].symbol());
            }
        }
        let row = |buffer: &ratatui::buffer::Buffer, y| {
            (0..32).map(|x| buffer[(x, y)].symbol()).collect::<String>()
        };
        assert_ne!(row(&at_start, 5), row(&at_end, 5));
    }

    #[test]
    fn credits_scrollbar_thumb_tracks_table_offset() {
        let credits = (0..12)
            .map(|i| HeroCredit {
                name: format!("Person {i}"),
                role: "Actor".into(),
            })
            .collect::<Vec<_>>();
        let content = HeroContent {
            facts: facts(ArtworkShape::Landscape),
            overview: Some("Overview".into()),
            credits: Some(credits),
            workspace: Some(Workspace {
                selector: None,
                list: &mut NoopList,
                focused: false,
            }),
        };
        let draw = |offset| {
            let mut terminal = Terminal::new(TestBackend::new(32, 14)).unwrap();
            terminal
                .draw(|f| {
                    paint_overview_box(f, Rect::new(0, 0, 32, 14), 0, &content, offset);
                })
                .unwrap();
            (2..14)
                .filter(|y| terminal.backend().buffer()[(31, *y)].symbol() != " ")
                .collect::<Vec<_>>()
        };
        let at_start = draw(0);
        let at_end = draw(12);
        assert_ne!(at_start, at_end);
        assert!(at_start.iter().copied().max() < at_end.iter().copied().max());
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
            .draw(|f| paint_credits_from(f, Rect::new(0, 0, 24, 1), &credits, 1))
            .unwrap();
        assert_eq!(
            terminal.backend().buffer()[(0, 0)].bg,
            palette::HERO_CREDITS_STRIPE
        );
    }

    #[test]
    fn credits_column_geometry_uses_rows_before_scroll_offset() {
        let credits = [
            HeroCredit {
                name: "The Longest Name".into(),
                role: "Director".into(),
            },
            HeroCredit {
                name: "X".into(),
                role: "Actor".into(),
            },
        ];
        let draw = |offset| {
            let mut terminal = Terminal::new(TestBackend::new(32, 1)).unwrap();
            terminal
                .draw(|f| paint_credits_from(f, Rect::new(0, 0, 32, 1), &credits, offset))
                .unwrap();
            (0..32)
                .map(|x| terminal.backend().buffer()[(x, 0)].symbol())
                .collect::<String>()
        };
        let at_start = draw(0);
        let after_scroll = draw(1);
        assert_eq!(&at_start[24..32], "Director");
        assert_eq!(&after_scroll[27..32], "Actor");
        assert_eq!(&after_scroll[16..18], "  ");
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
            .draw(|f| {
                overlay_links(
                    f,
                    Rect::new(0, 0, 20, 1),
                    &facts,
                    None,
                    &mut HitRegions::new(),
                )
            })
            .unwrap();
        assert_eq!(terminal.backend().buffer()[(0, 0)].symbol(), " ");
    }
}
