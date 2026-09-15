use super::*;
use crate::app::components::library_panel::{LibraryPanelContent, ListSlot};
use ratatui::layout::Rect;

use crate::app::components::inline_search::{InlineSearch, SearchPool};
use crate::app::components::library_panel::content::{
    ArtworkShape, HeroArtwork, HeroContent, HeroFacts, ListControls, PanelList, SelectorRow,
    Workspace,
};
use crate::app::render::arrangements::library::wide_library_panes;
use ratatui::backend::TestBackend;
use ratatui::widgets::Paragraph;
use ratatui::Terminal;

// Tall enough for a Landscape header (16:9 artwork box) to leave the
// title and meta rows visible below it.
const AREA: Rect = Rect::new(0, 0, 100, 30);

/// A test double over [`PanelList`]: paints its rows and retains the rect
/// it was viewed into.
struct StubList {
    rows: Vec<&'static str>,
    painted: Option<Rect>,
}

impl StubList {
    fn with_rows(rows: Vec<&'static str>) -> Self {
        Self {
            rows,
            painted: None,
        }
    }
}

impl PanelList for StubList {
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

    fn view(&mut self, f: &mut Frame, rect: Rect) {
        self.painted = Some(rect);
        for (index, row) in self.rows.iter().enumerate() {
            if index as u16 >= rect.height {
                break;
            }
            f.render_widget(
                Paragraph::new(*row),
                Rect {
                    y: rect.y + index as u16,
                    height: 1,
                    ..rect
                },
            );
        }
    }
}

fn hero_facts(title: &str) -> HeroFacts {
    HeroFacts {
        title: title.into(),
        meta_rows: vec!["2020".into()],
        links: Vec::new(),
        artwork: HeroArtwork {
            shape: ArtworkShape::Landscape,
            source: None,
            image: crate::app::components::library_panel::content::HeroImageState::None,
        },
    }
}

fn draw_skeleton(
    content: &mut LibraryPanelContent<'_>,
    browser_focused: bool,
) -> (ratatui::buffer::Buffer, WideSkeletonGeometry, SkeletonHits) {
    let mut terminal = Terminal::new(TestBackend::new(AREA.width, AREA.height)).unwrap();
    let mut hits = SkeletonHits::default();
    let mut windows = SkeletonPillWindows::default();
    let mut geometry = None;
    terminal
        .draw(|f| {
            geometry = render_wide_skeleton(
                f,
                AREA,
                content,
                browser_focused,
                None,
                0,
                None,
                None,
                &mut hits,
                &mut windows,
            );
        })
        .unwrap();
    (
        terminal.backend().buffer().clone(),
        geometry.expect("wide skeleton painted"),
        hits,
    )
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

fn browser_pane(area: Rect) -> crate::app::render::arrangements::wide_hero::WideHeroBrowserPane {
    let panes = wide_library_panes(area, PANE_PAD_X, PANE_PAD_Y, None).expect("wide area");
    wide_hero_browser_pane(panes.browser_panel, panes.browser_area)
}

#[test]
fn read_only_hero_renders_resting_with_selector_and_list() {
    let mut list = StubList::with_rows(vec!["Alpha", "Beta"]);
    let mut content = LibraryPanelContent {
        selector: Some(SelectorRow {
            pills: vec!["All".into()],
            active: Some(0),
        }),
        controls: None,
        list: ListSlot::Media(&mut list),
        hero: Some(HeroContent {
            facts: hero_facts("Dune"),
            overview: None,
            credits: None,
            workspace: None,
        }),
    };
    let (buf, geo, hits) = draw_skeleton(&mut content, false);

    // Hero pane: read-only, so it always renders the resting surface.
    assert_eq!(
        buf[(geo.hero.x, geo.hero.y)].bg,
        palette::surface_colors(palette::Surface::HeroPane, false).fill
    );
    assert!(geo.workspace.is_none());

    // Hero facts painted inside the hero pane's content rect.
    assert!(rect_contains(geo.hero, geo.hero_area));
    assert!(text_in(&buf, geo.hero_area, "Dune"));
    assert!(text_in(&buf, geo.hero_area, "2020"));

    // Selector row: pill bar painted, spacer below, hit retained.
    assert!(text_in(&buf, geo.selector_bar, "All"));
    let (pill_rect, pill_id) = hits.selector.regions()[0];
    assert_eq!(pill_id, 0);
    let point = ratatui::layout::Position {
        x: pill_rect.x + 1,
        y: pill_rect.y,
    };
    assert_eq!(hits.selector.resolve(point), Some(&0));
    // The spacer row directly below the bar paints the panel's gap band.
    let pane = browser_pane(AREA);
    assert_eq!(pane.spacer_area.bottom(), pane.pills_area.bottom() + 1);
    assert_eq!(
        buf[(pane.spacer_area.x, pane.spacer_area.y)].bg,
        palette::surface_colors(palette::Surface::PillRowGap, false).fill
    );

    // List rows painted inside the list box's row-flow rect, which sits
    // inside the browser pane; the stub saw exactly that rect.
    assert!(rect_contains(geo.browser, geo.list_area));
    assert_eq!(list.painted, Some(geo.list_area));
    assert!(text_in(&buf, geo.list_area, "Alpha"));
    assert!(text_in(&buf, geo.list_area, "Beta"));
    // The row flow keeps one spacer row above the list box's bottom edge,
    // matching the Workspace box's own bottom padding.
    assert_eq!(geo.list_area.bottom(), geo.list_panel.bottom() - 1);

    // No controls row content: the row is absent and the list box starts
    // exactly where the shared browser-pane primitive places it.
    assert!(geo.controls.is_none());
    assert_eq!(geo.list_panel.y, browser_pane(AREA).list_panel.y);
    // The list box is bordered: the fill runs under the row flow.
    assert_eq!(
        buf[(geo.list_panel.x, geo.list_panel.y)].bg,
        palette::surface_colors(palette::Surface::LibraryPanel, false).fill
    );
}

#[test]
fn list_controls_row_moves_the_list_box_down_one_row() {
    let mut list = StubList::with_rows(vec!["Alpha"]);
    let mut content = LibraryPanelContent {
        selector: None,
        controls: Some(ListControls {
            label: "17 items".into(),
        }),
        list: ListSlot::Media(&mut list),
        hero: None,
    };
    let (buf, geo, _hits) = draw_skeleton(&mut content, false);
    let pane = browser_pane(AREA);
    let controls = geo.controls.expect("controls row reserved");
    assert!(text_in(&buf, controls, "17 items"));
    // Relational: the controls row occupies the pane's first list row and
    // the list box starts directly below it.
    assert_eq!(controls.y, pane.list_panel.y);
    assert_eq!(geo.list_panel.y, controls.bottom());
}

#[test]
fn workspace_selector_with_active_none_paints_no_active_pill() {
    let mut list = StubList::with_rows(vec!["Alpha"]);
    let mut workspace_list = StubList::with_rows(vec!["Ep 1"]);
    let mut content = LibraryPanelContent {
        selector: None,
        controls: None,
        list: ListSlot::Media(&mut list),
        hero: Some(HeroContent {
            facts: hero_facts("Series"),
            overview: None,
            credits: None,
            workspace: Some(Workspace {
                header: None,
                selector: Some(SelectorRow {
                    pills: vec!["Seasons".into(), "Episodes".into()],
                    active: None,
                }),
                list: &mut workspace_list,
                focused: true,
            }),
        }),
    };
    let (buf, _geo, hits) = draw_skeleton(&mut content, false);

    // The workspace selector's pills paint and their hitboxes are kept;
    // `active: None` paints no active pill — no cell in any retained
    // pill rect carries the selected chip's surface, and every pill's
    // label cell paints the unselected pill foreground.
    assert_eq!(hits.workspace_selector.regions().len(), 2);
    for (rect, _) in hits.workspace_selector.regions() {
        for y in rect.top()..rect.bottom() {
            for x in rect.left()..rect.right() {
                assert_ne!(
                    buf[(x, y)].bg,
                    palette::PILL_SELECTED_BG,
                    "active: None must paint no active pill"
                );
            }
        }
        let cell = &buf[(rect.x + 1, rect.y)];
        assert_eq!(cell.style().fg, Some(palette::PILL_FG));
    }
    // A hit test inside a painted pill still resolves its index.
    let (first_rect, first_id) = hits.workspace_selector.regions()[0];
    let point = ratatui::layout::Position {
        x: first_rect.x + 1,
        y: first_rect.y,
    };
    assert_eq!(hits.workspace_selector.resolve(point), Some(&first_id));
}

#[test]
fn focused_workspace_hero_pane_stays_resting() {
    let mut list = StubList::with_rows(vec!["Alpha"]);
    let mut workspace_list = StubList::with_rows(vec!["Ep 1"]);
    let mut content = LibraryPanelContent {
        selector: None,
        controls: None,
        list: ListSlot::Media(&mut list),
        hero: Some(HeroContent {
            facts: hero_facts("Series"),
            overview: None,
            credits: None,
            workspace: Some(Workspace {
                header: None,
                selector: None,
                list: &mut workspace_list,
                focused: true,
            }),
        }),
    };
    let (buf, geo, _hits) = draw_skeleton(&mut content, false);

    // The hero pane never takes the focused surface, even while its
    // Workspace holds focus — only the list box flips with the panel.
    assert_eq!(
        buf[(geo.hero.x, geo.hero.y)].bg,
        palette::surface_colors(palette::Surface::HeroPane, false).fill
    );
    let (box_panel, box_content) = geo.workspace.expect("workspace box painted");
    // Accent-soft box surface while the workspace list holds focus.
    assert_eq!(
        buf[(box_panel.x, box_panel.y)].bg,
        palette::surface_colors(palette::Surface::MainContentBox, true).fill
    );
    // The workspace list is viewed into the box's content rect.
    assert_eq!(workspace_list.painted, Some(box_content));
    assert!(text_in(&buf, box_content, "Ep 1"));
    assert!(rect_contains(geo.hero, box_panel));

    // With the panel focused but the Workspace holding it, the browser
    // list drops its green and rests while the hero pane stays resting.
    let mut focused_list = StubList::with_rows(vec!["Alpha"]);
    let mut focused_workspace_list = StubList::with_rows(vec!["Ep 1"]);
    let mut focused_content = LibraryPanelContent {
        selector: None,
        controls: None,
        list: ListSlot::Media(&mut focused_list),
        hero: Some(HeroContent {
            facts: hero_facts("Series"),
            overview: None,
            credits: None,
            workspace: Some(Workspace {
                header: None,
                selector: None,
                list: &mut focused_workspace_list,
                focused: true,
            }),
        }),
    };
    let (buf, geometry, _hits) = draw_skeleton(&mut focused_content, true);
    assert_eq!(
        buf[(geometry.list_panel.x, geometry.list_panel.y)].bg,
        palette::surface_colors(palette::Surface::LibraryPanel, false).fill
    );
    assert_eq!(
        buf[(geometry.hero.x, geometry.hero.y)].bg,
        palette::surface_colors(palette::Surface::HeroPane, false).fill
    );
}

#[test]
fn browser_focused_list_carries_the_focus_green() {
    let mut list = StubList::with_rows(vec!["Alpha"]);
    let mut workspace_list = StubList::with_rows(vec!["Ep 1"]);
    let mut content = LibraryPanelContent {
        selector: None,
        controls: None,
        list: ListSlot::Media(&mut list),
        hero: Some(HeroContent {
            facts: hero_facts("Series"),
            overview: None,
            credits: None,
            workspace: Some(Workspace {
                header: None,
                selector: None,
                list: &mut workspace_list,
                focused: false,
            }),
        }),
    };
    let (buf, geometry, _hits) = draw_skeleton(&mut content, true);
    // The browser list holds focus: green list box, resting hero pane and
    // resting workspace box.
    assert_eq!(
        buf[(geometry.list_panel.x, geometry.list_panel.y)].bg,
        palette::surface_colors(palette::Surface::LibraryPanel, true).fill
    );
    assert_eq!(
        buf[(geometry.hero.x, geometry.hero.y)].bg,
        palette::surface_colors(palette::Surface::HeroPane, false).fill
    );
    let (box_panel, _) = geometry.workspace.expect("workspace box painted");
    assert_eq!(
        buf[(box_panel.x, box_panel.y)].bg,
        palette::surface_colors(palette::Surface::MainContentBox, false).fill
    );
}

#[test]
fn unfocused_workspace_hero_renders_resting_surfaces() {
    let mut list = StubList::with_rows(vec!["Alpha"]);
    let mut workspace_list = StubList::with_rows(vec!["Ep 1"]);
    let mut content = LibraryPanelContent {
        selector: None,
        controls: None,
        list: ListSlot::Media(&mut list),
        hero: Some(HeroContent {
            facts: hero_facts("Series"),
            overview: None,
            credits: None,
            workspace: Some(Workspace {
                header: None,
                selector: None,
                list: &mut workspace_list,
                focused: false,
            }),
        }),
    };
    let (buf, geo, _hits) = draw_skeleton(&mut content, false);
    assert_eq!(
        buf[(geo.hero.x, geo.hero.y)].bg,
        palette::surface_colors(palette::Surface::HeroPane, false).fill
    );
    let (box_panel, _) = geo.workspace.expect("workspace box painted");
    assert_eq!(
        buf[(box_panel.x, box_panel.y)].bg,
        palette::surface_colors(palette::Surface::MainContentBox, false).fill
    );
}

#[test]
fn workspace_box_sits_one_blank_row_below_the_hero_content() {
    let mut list = StubList::with_rows(vec!["Alpha"]);
    let mut workspace_list = StubList::with_rows(vec!["Ep 1"]);
    let mut content = LibraryPanelContent {
        selector: None,
        controls: None,
        list: ListSlot::Media(&mut list),
        hero: Some(HeroContent {
            facts: hero_facts("Dune"),
            overview: None,
            credits: None,
            workspace: Some(Workspace {
                header: None,
                selector: None,
                list: &mut workspace_list,
                focused: false,
            }),
        }),
    };
    let (buf, geo, _hits) = draw_skeleton(&mut content, false);
    let (box_panel, _) = geo.workspace.expect("workspace box painted");
    assert!(rect_contains(geo.hero, box_panel));

    // One blank row between the painted hero content's bottom and the
    // Workspace box (task 5.6): the row directly above the box carries
    // only the hero pane's resting fill across the pane's width.
    let fill = palette::surface_colors(palette::Surface::HeroPane, false).fill;
    let gap_row = box_panel.y - 1;
    assert!(gap_row > geo.hero.y, "gap row inside the hero pane");
    for x in geo.hero.left()..geo.hero.right() {
        assert_eq!(
            buf[(x, gap_row)].bg,
            fill,
            "row {gap_row} must stay blank between the content and the box"
        );
    }
    // The row above the gap is the painted content's bottom — the meta
    // row, compared by text, not by absolute coordinates.
    let mut content_row = String::new();
    for x in geo.hero.left()..geo.hero.right() {
        content_row.push_str(buf[(x, gap_row - 1)].symbol());
    }
    assert!(
        content_row.contains("2020"),
        "content ends directly above the gap"
    );
    // The Workspace is the hero's bottom-most recessed box, so it reaches the
    // hero pane's content bottom: the panel less its one bottom padding row.
    assert_eq!(box_panel.bottom(), geo.hero_area.bottom());
}

#[test]
fn workspace_header_paints_title_separator_and_blank_row_above_the_list() {
    let mut list = StubList::with_rows(vec!["Alpha"]);
    let mut workspace_list = StubList::with_rows(vec!["Track 1"]);
    let mut content = LibraryPanelContent {
        selector: None,
        controls: None,
        list: ListSlot::Media(&mut list),
        hero: Some(HeroContent {
            facts: hero_facts("Album"),
            overview: None,
            credits: None,
            workspace: Some(Workspace {
                header: Some("Tracks"),
                selector: None,
                list: &mut workspace_list,
                focused: false,
            }),
        }),
    };
    let (buf, geo, _hits) = draw_skeleton(&mut content, false);
    let (box_panel, list_rect) = geo.workspace.expect("workspace box painted");
    let fill = palette::surface_colors(palette::Surface::MainContentBox, false).fill;
    // The box's own content top: `geo.workspace`'s second rect is the list
    // rect below the header.
    let content_y = box_panel.y + PANE_PAD_Y;
    let content_x = box_panel.x + PANE_PAD_X;
    let content_width = box_panel.width - PANE_PAD_X * 2;

    // Header row: the title in foam, on the box's own surface.
    let title_row: String = (content_x..content_x + content_width)
        .map(|x| buf[(x, content_y)].symbol())
        .collect();
    assert!(
        title_row.starts_with("Tracks"),
        "header row reads {title_row:?}"
    );
    assert_eq!(
        buf[(content_x, content_y)].style().fg,
        Some(palette::TEXT_METADATA)
    );
    assert_eq!(buf[(content_x, content_y)].bg, fill);

    // Separator: the Movie hero's own block-character line, directly below.
    for x in content_x..content_x + content_width {
        assert_eq!(buf[(x, content_y + 1)].symbol(), "\u{2581}");
        assert_eq!(
            buf[(x, content_y + 1)].style().fg,
            Some(palette::HERO_OVERVIEW_SEPARATOR)
        );
    }

    // One blank row between the separator and the list.
    for x in content_x..content_x + content_width {
        assert_eq!(buf[(x, content_y + 2)].symbol(), " ");
        assert_eq!(buf[(x, content_y + 2)].bg, fill);
    }

    // The list starts below the three header rows and the box's bottom
    // padding is unchanged by them.
    assert_eq!(list_rect.y, content_y + 3);
    assert!(text_in(&buf, list_rect, "Track 1"));
    assert_eq!(list_rect.bottom(), box_panel.bottom() - PANE_PAD_Y);
    assert_eq!(list_rect.x, content_x);
    assert_eq!(list_rect.width, content_width);
}

#[test]
fn overview_box_fills_the_hero_pane_when_there_is_no_workspace() {
    let mut list = StubList::with_rows(vec!["Alpha"]);
    let mut content = LibraryPanelContent {
        selector: None,
        controls: None,
        list: ListSlot::Media(&mut list),
        hero: Some(HeroContent {
            facts: hero_facts("Dune"),
            overview: Some("A very long overview.".into()),
            credits: None,
            workspace: None,
        }),
    };
    let (buf, geo, _hits) = draw_skeleton(&mut content, false);
    let box_fill = palette::surface_colors(palette::Surface::MainContentBox, false).fill;
    let pane_fill = palette::surface_colors(palette::Surface::HeroPane, false).fill;
    assert!(geo.workspace.is_none());
    // The overview is the hero's bottom-most recessed box, so it reaches the
    // hero pane's content bottom (the panel less its bottom padding row).
    assert_eq!(
        buf[(geo.hero_area.x + 2, geo.hero_area.bottom() - 1)].bg,
        box_fill
    );
    // That bottom padding row itself keeps the pane's resting fill.
    assert_eq!(
        buf[(geo.hero_area.x + 2, geo.hero.bottom() - 1)].bg,
        pane_fill
    );
}

#[test]
fn active_search_takes_the_selector_row_and_the_list_box() {
    let mut search = InlineSearch::new();
    search.open();
    search.set_pool(SearchPool::Items(vec![crate::app::tests::make_item(
        "Alpha", "Movie",
    )]));
    search.restore_query("Alp".into());
    let mut content = LibraryPanelContent {
        selector: Some(SelectorRow {
            pills: vec!["All".into()],
            active: Some(0),
        }),
        controls: None,
        list: ListSlot::Search(&mut search),
        hero: Some(HeroContent {
            facts: hero_facts("Dune"),
            overview: None,
            credits: None,
            workspace: None,
        }),
    };
    let (buf, geo, _hits) = draw_skeleton(&mut content, false);

    // The search box occupies the Selector row's place: query text in the
    // bar rect, and no selector pill painted there instead.
    assert!(text_in(&buf, geo.selector_bar, "SEARCH:"));
    assert!(text_in(&buf, geo.selector_bar, "Alp"));
    assert!(
        !text_in(&buf, geo.selector_bar, "\u{25e2}"),
        "no selector pill under the search box"
    );
    // The results occupy the list box; the search session retained the
    // row-flow rect the skeleton gave it.
    assert!(text_in(&buf, geo.list_area, "Alpha"));
    assert_eq!(*search.layout(), geo.list_area);
    // The rest of the panel is unchanged: hero pane resting with its facts.
    assert_eq!(
        buf[(geo.hero.x, geo.hero.y)].bg,
        palette::surface_colors(palette::Surface::HeroPane, false).fill
    );
    assert!(text_in(&buf, geo.hero_area, "Dune"));
}

#[test]
fn empty_list_slot_paints_its_placeholder_in_the_list_box() {
    let mut content = LibraryPanelContent {
        selector: None,
        controls: None,
        list: ListSlot::Empty {
            loading: false,
            text: "Nothing here".into(),
        },
        hero: None,
    };
    let (buf, geo, _hits) = draw_skeleton(&mut content, false);
    assert!(text_in(&buf, geo.list_area, "Nothing here"));
}

#[test]
fn empty_loading_list_slot_paints_the_loading_placeholder() {
    let mut content = LibraryPanelContent {
        selector: None,
        controls: None,
        list: ListSlot::Empty {
            loading: true,
            text: String::new(),
        },
        hero: None,
    };
    let (buf, geo, _hits) = draw_skeleton(&mut content, false);
    assert!(text_in(&buf, geo.list_area, "Loading"));
}

#[test]
fn sub_breakpoint_area_paints_nothing() {
    let mut list = StubList::with_rows(vec!["Alpha"]);
    let mut content = LibraryPanelContent {
        selector: None,
        controls: None,
        list: ListSlot::Media(&mut list),
        hero: None,
    };
    let mut terminal = Terminal::new(TestBackend::new(AREA.width, AREA.height)).unwrap();
    let mut hits = SkeletonHits::default();
    let mut windows = SkeletonPillWindows::default();
    let narrow = Rect {
        width: crate::app::TWO_COLUMN_THRESHOLD - 1,
        ..AREA
    };
    let mut geometry = None;
    terminal
        .draw(|f| {
            geometry = render_wide_skeleton(
                f,
                narrow,
                &mut content,
                false,
                None,
                0,
                None,
                None,
                &mut hits,
                &mut windows,
            );
        })
        .unwrap();
    assert!(geometry.is_none());
    let buf = terminal.backend().buffer();
    // Untouched: every cell still carries the empty terminal's style,
    // compared relatively so no raw colour primitive is named here.
    let untouched = buf[(0, 0)].bg;
    for y in 0..AREA.height {
        for x in 0..AREA.width {
            assert_eq!(buf[(x, y)].bg, untouched);
        }
    }
}

fn rect_contains(outer: Rect, inner: Rect) -> bool {
    outer.left() <= inner.left()
        && outer.right() >= inner.right()
        && outer.top() <= inner.top()
        && outer.bottom() >= inner.bottom()
}
