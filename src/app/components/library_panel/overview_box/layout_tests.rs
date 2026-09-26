use super::*;
use crate::app::components::library_panel::content::{
    ArtworkShape, HeroArtwork, HeroLink, PanelList, Workspace,
};
use crate::app::components::library_panel::hero_header::{
    hero_artwork_box, paint_hero_pane_content,
};
use crate::app::palette;
use crate::app::render::{paint_wide_hero_text, WrappedHeroLine};
use ratatui::backend::TestBackend;
use ratatui::Terminal;

/// A stub `PanelList` so a Workspace can be present for the overview box.
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
    terminal
        .draw(|f| {
            paint_hero_pane_content(
                f,
                area,
                pane,
                0,
                None,
                &mut HitRegions::new(),
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
    let artwork = hero_artwork_box(AREA, &pane.facts, pane.workspace.is_some(), AREA.height);
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
            header: None,
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
    let artwork = hero_artwork_box(AREA, &with.facts, with.workspace.is_some(), AREA.height);
    let box_top = (artwork.bottom()..AREA.bottom()).find(|y| buf[(AREA.x + 2, *y)].bg == box_fill);
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
    // A tall pane: short panes cap the overview at 5 rows, so the
    // fills-the-pane claim only holds above the short-pane threshold.
    let buf = draw_pane(AREA.width, 51, &with);
    // The bottom-most recessed box reaches the content area's last row:
    // the hero panel's own bottom padding row sits outside `area`.
    assert_eq!(buf[(AREA.x + 2, 51 - 1)].bg, box_fill);

    // With a Workspace below it the overview is no longer the bottom-most
    // box, so it stays content-sized.
    let mut workspace_list = NoopList;
    let with_workspace = HeroContent {
        facts: facts(ArtworkShape::Landscape),
        overview: Some("A very long overview.".into()),
        credits: None,
        workspace: Some(Workspace {
            header: None,
            selector: None,
            list: &mut workspace_list,
            focused: false,
        }),
    };
    let buf = draw_pane(AREA.width, 51, &with_workspace);
    assert_ne!(buf[(AREA.x + 2, 51 - 1)].bg, box_fill);
}

#[test]
fn short_pane_caps_the_overview_box_at_five_content_rows() {
    let box_fill = palette::surface_colors(palette::Surface::MainContentBox, false).fill;
    let with = HeroContent {
        facts: facts(ArtworkShape::Landscape),
        overview: Some("A very long overview.".into()),
        credits: None,
        workspace: None,
    };
    let buf = draw_pane(AREA.width, AREA.height, &with);
    let artwork = hero_artwork_box(AREA, &with.facts, false, AREA.height);
    let box_rows: Vec<u16> = (artwork.bottom()..AREA.bottom())
        .filter(|y| buf[(AREA.x + 2, *y)].bg == box_fill)
        .collect();
    assert!(!box_rows.is_empty(), "the overview box paints");
    // 5 content rows plus the box's top/bottom padding.
    assert_eq!(box_rows.len(), 5 + 2 * PANE_PAD_Y as usize);
    // The rows below the capped box were never painted by this harness (the
    // pane fill belongs to the panel painter); they must not carry the box.
    let below = *box_rows.last().unwrap() + 1;
    assert!(below < AREA.bottom(), "the cap leaves rows below the box");
    assert_ne!(buf[(AREA.x + 2, below)].bg, box_fill);
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
    let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(30, 2)).unwrap();
    let facts = HeroFacts {
        title: "Title".into(),
        meta_rows: vec!["IMDb".into()],
        duration_row: None,
        progress_row: None,
        links: vec![HeroLink {
            name: "IMDb".into(),
            url: "https://imdb.test".into(),
        }],
        artwork: HeroArtwork {
            shape: super::super::content::ArtworkShape::Landscape,
            source: None,
            decoration: None,
            image: super::super::content::HeroImageState::None,
        },
    };
    let mut link_hits = HitRegions::new();
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
            overlay_links(f, Rect::new(0, 0, 30, 2), &facts, Some(0), &mut link_hits);
        })
        .unwrap();
    let buffer = terminal.backend().buffer();
    for x in 0..4 {
        assert_eq!(buffer[(x, 1)].style().fg, Some(palette::TEXT_METADATA));
        assert!(buffer[(x, 1)]
            .style()
            .add_modifier
            .contains(ratatui::style::Modifier::UNDERLINED));
    }
    assert_eq!(link_hits.regions(), &[(Rect::new(0, 1, 4, 1), 0)]);
    assert_eq!(
        link_hits.resolve(ratatui::layout::Position { x: 2, y: 1 }),
        Some(&0)
    );
    assert_eq!(
        link_hits.resolve(ratatui::layout::Position { x: 4, y: 1 }),
        None
    );

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

/// Grid links register at the painted label run: entries
/// [Title, 2021, 2h, IMDb] put the links row third-from-title, so it
/// lands in the right column right-aligned — hits open at the text,
/// not the cell edge.
#[test]
fn grid_links_register_at_the_right_aligned_label() {
    let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(60, 4)).unwrap();
    let facts = HeroFacts {
        title: "Title".into(),
        meta_rows: vec!["2021".into(), "2h".into(), "IMDb".into()],
        duration_row: None,
        progress_row: None,
        links: vec![HeroLink {
            name: "IMDb".into(),
            url: "https://imdb.test".into(),
        }],
        artwork: HeroArtwork {
            shape: super::super::content::ArtworkShape::Landscape,
            source: None,
            decoration: None,
            image: super::super::content::HeroImageState::None,
        },
    };
    let mut link_hits = HitRegions::new();
    terminal
        .draw(|f| {
            overlay_links_grid(
                f,
                Rect::new(0, 0, 60, 4),
                30,
                &facts,
                Some(0),
                &mut link_hits,
            );
        })
        .unwrap();
    assert_eq!(link_hits.regions(), &[(Rect::new(56, 1, 4, 1), 0)]);
    let buffer = terminal.backend().buffer();
    assert!(buffer[(56, 1)]
        .style()
        .add_modifier
        .contains(ratatui::style::Modifier::UNDERLINED));
    assert_eq!(
        link_hits.resolve(ratatui::layout::Position { x: 57, y: 1 }),
        Some(&0)
    );
    assert_eq!(
        link_hits.resolve(ratatui::layout::Position { x: 55, y: 1 }),
        None
    );
}

#[test]
fn link_outside_box_is_not_overlaid() {
    let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(20, 1)).unwrap();
    let facts = HeroFacts {
        title: "Title".into(),
        meta_rows: vec!["Link".into()],
        duration_row: None,
        progress_row: None,
        links: vec![HeroLink {
            name: "Link".into(),
            url: "https://example.test".into(),
        }],
        artwork: HeroArtwork {
            shape: super::super::content::ArtworkShape::Landscape,
            source: None,
            decoration: None,
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
            );
        })
        .unwrap();
    assert_eq!(terminal.backend().buffer()[(0, 0)].symbol(), " ");
}
