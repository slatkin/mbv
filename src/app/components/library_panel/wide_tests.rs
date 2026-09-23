use super::*;
use crate::app::components::library_panel::{
    LibraryKey, LibraryKind, LibraryPanel, LibraryPanelContent, ListSlot,
};
use crate::app::components::tv_content::TvContent;
use crate::app::render::{LibraryListRenderCtx, TvWideRenderCtx};
use ratatui::layout::Rect;

use crate::app::components::inline_search::{InlineSearch, SearchPool};
use crate::app::components::library_panel::content::{
    ArtworkShape, HeroArtwork, HeroContent, HeroFacts, PanelList, SelectorRow, Workspace,
    WorkspaceHeader,
};
use crate::app::render::arrangements::library::{
    wide_library_panes, wide_library_panes_with_selector,
};
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
    fn clamp_viewport(&mut self, _viewport_height: usize) {}

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
        duration_row: None,
        progress_row: None,
        links: Vec::new(),
        artwork: HeroArtwork {
            shape: ArtworkShape::Landscape,
            source: None,
            decoration: None,
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
                80, // tall terminal: the short-pane caps must not skew the skeleton tests
                true,
            );
        })
        .unwrap();
    (
        terminal.backend().buffer().clone(),
        geometry.expect("wide skeleton painted"),
        hits,
    )
}

#[rstest::rstest]
#[case::latest(mbv_core::config::TvContentMode::Latest)]
#[case::upcoming(mbv_core::config::TvContentMode::Upcoming)]
fn tv_flat_modes_reclaim_the_wide_hero_pane(#[case] mode: mbv_core::config::TvContentMode) {
    let mut item = crate::app::tests::make_item("Episode One", "Episode");
    item.series_name = "Example Series".into();
    let list = LibraryListRenderCtx::from_items(vec![item], 0);
    let mut context = TvWideRenderCtx::new(list, None, None, 0, None, false);
    context.set_tv_content_mode(Some(mode));

    let mut owner = TvContent::new();
    owner.set_is_wide(true);
    owner.set_content(context);
    let key = LibraryKey::Service {
        service: mbv_core::config::ServiceKind::Emby,
        library_id: "tv".into(),
        kind: LibraryKind::TvShows,
    };
    let mut panel = LibraryPanel::new();
    panel.insert_owner(key.clone(), Box::new(owner));
    panel.set_active(Some(key));
    let mut terminal = Terminal::new(TestBackend::new(AREA.width, AREA.height)).unwrap();
    terminal
        .draw(|f| tuirealm::component::Component::view(&mut panel, f, AREA))
        .unwrap();

    let geometry = panel.test_wide_geometry().expect("Wide panel geometry");
    let panes = wide_library_panes_with_selector(AREA, PANE_PAD_X, PANE_PAD_Y, None, false, false)
        .expect("Wide arrangement without Selector row");
    assert_eq!(geometry.browser, panes.content_area);
    assert_eq!(geometry.hero.width, 0);
    assert_eq!(geometry.list_panel.right(), AREA.right());
    assert!(text_in(
        terminal.backend().buffer(),
        geometry.list_area,
        "Episode One"
    ));
}

fn browser_pane(area: Rect) -> crate::app::render::arrangements::wide_hero::WideHeroBrowserPane {
    use crate::app::render::arrangements::wide_hero::WideHeroBrowserPane;
    let panes = wide_library_panes(area, PANE_PAD_X, PANE_PAD_Y, None, true).expect("wide area");
    WideHeroBrowserPane {
        pills_area: panes.pills_area,
        spacer_area: panes.spacer_area,
        list_panel: panes.browser_panel,
    }
}

#[test]
fn read_only_hero_renders_resting_with_selector_and_list() {
    let mut list = StubList::with_rows(vec!["Alpha", "Beta"]);
    let mut content = LibraryPanelContent {
        selector: Some(SelectorRow {
            pills: vec!["All".into()],
            markers: vec![],
            active: Some(0),
        }),
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

    // The absence of a secondary row and the list-box start are owned by
    // `selector_band_spans_both_panes_and_the_list_box_has_no_pill_reserve`.
    assert_eq!(geo.list_panel.y, browser_pane(AREA).list_panel.y);
    // The list box is bordered: the fill runs under the row flow.
    assert_eq!(
        buf[(geo.list_panel.x, geo.list_panel.y)].bg,
        palette::surface_colors(palette::Surface::LibraryPanel, false).fill
    );
}

/// Task 1.3 (D2): the Wide skeleton hands the band-reduced content area to the
/// hero painter, so the hero pane's fill starts below the full-width Selector
/// band instead of at the raw panel top.
#[test]
fn hero_pane_starts_below_the_full_width_selector_band() {
    let mut list = StubList::with_rows(vec!["Alpha"]);
    let mut content = LibraryPanelContent {
        selector: Some(SelectorRow {
            pills: vec!["All".into()],
            markers: vec![],
            active: Some(0),
        }),
        list: ListSlot::Media(&mut list),
        hero: None,
    };
    let (buf, geo, _hits) = draw_skeleton(&mut content, false);
    let panes = wide_library_panes(AREA, PANE_PAD_X, PANE_PAD_Y, None, true).expect("wide area");
    // Component-layer claim: the hero's own painted surface starts exactly at
    // the band's bottom, and the spacer row above it is not hero fill. The
    // band's full-width placement is the arrangement claim owned by
    // `library.rs::band_carve_keeps_the_breakpoint_and_pushes_both_panes_below_it`.
    assert_eq!(geo.hero.y, panes.spacer_area.bottom());
    assert_ne!(
        buf[(geo.hero.x, panes.spacer_area.y)].bg,
        palette::surface_colors(palette::Surface::HeroPane, false).fill,
        "the spacer row above the hero is not hero fill"
    );
    assert_eq!(
        buf[(geo.hero.x, geo.hero.y)].bg,
        palette::surface_colors(palette::Surface::HeroPane, false).fill
    );
}

/// Task 2.1 (D3/D5): the Wide Selector band's pill row spans the panel's full
/// width across both panes, and the Browser list box carries no internal pill
/// reserve — its fill starts at the Browser pane's own top and reaches the
/// panel border.
#[test]
fn selector_band_spans_both_panes_and_the_list_box_has_no_pill_reserve() {
    let mut list = StubList::with_rows(vec!["Alpha"]);
    let mut content = LibraryPanelContent {
        selector: Some(SelectorRow {
            pills: vec!["All".into()],
            markers: vec![],
            active: Some(0),
        }),
        list: ListSlot::Media(&mut list),
        hero: Some(HeroContent {
            facts: hero_facts("Dune"),
            overview: None,
            credits: None,
            workspace: None,
        }),
    };
    let (buf, geo, _hits) = draw_skeleton(&mut content, false);
    let panes = wide_library_panes(AREA, PANE_PAD_X, PANE_PAD_Y, None, true).expect("wide area");
    let pill_row_bg = palette::surface_colors(palette::Surface::PillRow, false).fill;

    // The pill row starts flush at the panel's left edge and its own surface
    // reaches the panel's right edge, i.e. across the hero/browser gap into
    // the Browser pane.
    assert_eq!(geo.selector_bar.x, AREA.x);
    assert_eq!(geo.selector_bar.right(), panes.pills_area.right());
    assert!(panes.browser_panel.x > panes.hero_panel.right());
    assert_eq!(
        buf[(geo.selector_bar.x, geo.selector_bar.y)].bg,
        pill_row_bg
    );
    assert_eq!(
        buf[(panes.browser_panel.x, geo.selector_bar.y)].bg,
        pill_row_bg,
        "the pill row paints across the Browser pane too"
    );
    // The pill row's surface covers the panel's left edge column.
    assert_eq!(buf[(AREA.x, geo.selector_bar.y)].bg, pill_row_bg);

    // No internal pill reserve in the Wide path (D3): the list box starts at
    // the Browser pane's own top (already below the band) and its fill
    // reaches the pane's bottom border.
    assert_eq!(geo.list_panel.y, panes.browser_panel.y);
    assert_eq!(geo.list_panel.bottom(), panes.browser_panel.bottom());
    assert_eq!(geo.list_panel.right(), AREA.right());
    let list_bg = palette::surface_colors(palette::Surface::LibraryPanel, false).fill;
    assert_eq!(buf[(geo.list_panel.x, geo.list_panel.y)].bg, list_bg);
    assert_eq!(
        buf[(geo.list_panel.x, geo.list_panel.bottom() - 1)].bg,
        list_bg,
        "the list-box fill reaches the Browser pane's border"
    );
}

// The former List-controls-row painter test was removed with the deleted
// slot; the shared skeleton test above owns the surviving no-secondary-row
// contract.
#[test]
fn workspace_selector_with_active_none_paints_no_active_pill() {
    let mut list = StubList::with_rows(vec!["Alpha"]);
    let mut workspace_list = StubList::with_rows(vec!["Ep 1"]);
    let mut content = LibraryPanelContent {
        selector: None,
        list: ListSlot::Media(&mut list),
        hero: Some(HeroContent {
            facts: hero_facts("Series"),
            overview: None,
            credits: None,
            workspace: Some(Workspace {
                header: None,
                selector: Some(SelectorRow {
                    pills: vec!["Seasons".into(), "Episodes".into()],
                    markers: vec![],
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
        assert_eq!(cell.style().fg, Some(palette::TEXT_MUTED));
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
    // The Workspace box rests at the hero boxes' Slate even while its list
    // holds focus: the unified Workspace look has no focused soft fill.
    assert_eq!(
        buf[(box_panel.x, box_panel.y)].bg,
        palette::surface_colors(palette::Surface::MainContentBox, false).fill
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

/// The Library Hero overlay's Workspace box rests at the hero boxes' own
/// Slate in both focus states, matching the Main content box beside it; the
/// Wide Workspace's focus-following soft fill is owned by
/// `focused_workspace_hero_pane_stays_resting`.
#[test]
fn overlay_workspace_box_rests_at_the_hero_box_slate() {
    let sheet = crate::app::render::components::library_hero_overlay::OVERLAY_SHEET_SURFACE;
    for focused in [true, false] {
        let mut workspace_list = StubList::with_rows(vec!["Ep 1"]);
        let mut hero = HeroContent {
            facts: hero_facts("Series"),
            overview: None,
            credits: None,
            workspace: Some(Workspace {
                header: None,
                selector: None,
                list: &mut workspace_list,
                focused,
            }),
        };
        let mut hits = SkeletonHits::default();
        let mut windows = SkeletonPillWindows::default();
        let mut geometry: Option<
            crate::app::components::library_panel::hero_composition::HeroCompositionGeometry,
        > = None;
        let mut terminal = Terminal::new(TestBackend::new(AREA.width, AREA.height)).unwrap();
        terminal
            .draw(|f| {
                geometry = Some(paint_library_hero_content(
                    f,
                    AREA,
                    &mut hero,
                    0,
                    None,
                    &mut hits.links,
                    &mut hits.workspace_selector,
                    &mut windows.workspace_selector,
                    sheet,
                    80, // tall terminal: the short-pane caps must not skew the geometry
                ));
            })
            .unwrap();
        let buf = terminal.backend().buffer().clone();
        let (box_panel, _box_content) = geometry
            .expect("hero content painted")
            .workspace
            .expect("workspace box painted");
        assert_eq!(
            buf[(box_panel.x, box_panel.y)].bg,
            palette::surface_colors(palette::Surface::MainContentBox, false).fill,
            "overlay workspace box fill, focused={focused}"
        );
    }
}

#[test]
fn browser_focused_list_carries_the_focus_green() {
    let mut list = StubList::with_rows(vec!["Alpha"]);
    let mut workspace_list = StubList::with_rows(vec!["Ep 1"]);
    let mut content = LibraryPanelContent {
        selector: None,
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
    let (buf, geometry, hits) = draw_skeleton(&mut content, true);
    // The browser list holds focus: green list box, resting hero pane and
    // resting workspace box.
    assert_eq!(
        buf[(geometry.list_panel.x, geometry.list_panel.y)].bg,
        palette::surface_colors(palette::Surface::LibraryPanel, true).fill
    );
    assert_eq!(
        geometry.selector_bar.height, 0,
        "a destination without a Selector row reserves no band"
    );
    assert!(hits.selector.regions().is_empty());
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
        list: ListSlot::Media(&mut list),
        hero: Some(HeroContent {
            facts: hero_facts("Album"),
            overview: None,
            credits: None,
            workspace: Some(Workspace {
                header: Some(WorkspaceHeader::Tracklist),
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
        title_row.starts_with(WorkspaceHeader::Tracklist.label()),
        "header row reads {title_row:?}"
    );
    assert_eq!(
        buf[(content_x, content_y)].style().fg,
        Some(palette::WORKSPACE_HEADER_FG)
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
fn music_workspace_header_uses_its_role_without_bold_modifier() {
    let mut list = StubList::with_rows(vec!["Alpha"]);
    let mut workspace_list = StubList::with_rows(vec!["Track 1"]);
    let mut content = LibraryPanelContent {
        selector: None,
        list: ListSlot::Media(&mut list),
        hero: Some(HeroContent {
            facts: hero_facts("Album"),
            overview: None,
            credits: None,
            workspace: Some(Workspace {
                header: Some(WorkspaceHeader::Tracklist),
                selector: None,
                list: &mut workspace_list,
                focused: false,
            }),
        }),
    };
    let (buf, geo, _hits) = draw_skeleton(&mut content, false);
    let (box_panel, _) = geo.workspace.expect("workspace box painted");
    let content_y = box_panel.y + PANE_PAD_Y;
    let content_x = box_panel.x + PANE_PAD_X;
    let header = &buf[(content_x, content_y)];

    assert_eq!(
        palette::WORKSPACE_HEADER_FG,
        ratatui::style::Color::Rgb(0x3a, 0x94, 0xc5)
    );
    assert_eq!(header.fg, palette::WORKSPACE_HEADER_FG);
    assert!(!header.modifier.contains(ratatui::style::Modifier::BOLD));
}

#[test]
fn ready_hero_reports_the_reserved_image_box_inside_the_hero_area() {
    let mut list = StubList::with_rows(vec!["Alpha"]);
    let mut facts = hero_facts("Dune");
    facts.artwork.image = crate::app::components::library_panel::content::HeroImageState::Ready {
        cache_key: "dune-cover".into(),
        decoded: Some((1600, 900)),
    };
    let mut content = LibraryPanelContent {
        selector: None,
        list: ListSlot::Media(&mut list),
        hero: Some(HeroContent {
            facts,
            overview: None,
            credits: None,
            workspace: None,
        }),
    };

    let (buf, geo, _hits) = draw_skeleton(&mut content, false);
    let image = geo.hero_image.expect("ready artwork reserves an image box");
    assert!(rect_contains(geo.hero_area, image.area));
    assert!(image.area.width > 0 && image.area.height > 0);
    assert_eq!(image.cache_key, "dune-cover");
    // The shared composition reserves the image area for the shell projection;
    // the header still paints its placeholder/content into that same area.
    assert_eq!(
        buf[(image.area.x, image.area.y)].bg,
        buf[(geo.hero_area.x, geo.hero_area.y)].bg
    );
}

#[test]
fn overview_box_fills_the_hero_pane_when_there_is_no_workspace() {
    let mut list = StubList::with_rows(vec!["Alpha"]);
    let mut content = LibraryPanelContent {
        selector: None,
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
            markers: vec![],
            active: Some(0),
        }),
        list: ListSlot::Search(&mut search),
        hero: Some(HeroContent {
            facts: hero_facts("Dune"),
            overview: None,
            credits: None,
            workspace: None,
        }),
    };
    let (buf, geo, _hits) = draw_skeleton(&mut content, false);
    let panes = wide_library_panes(AREA, PANE_PAD_X, PANE_PAD_Y, None, true).expect("wide area");
    let pill_row_bg = palette::surface_colors(palette::Surface::PillRow, false).fill;

    // The search box occupies the Selector row's place: query text in the
    // bar rect, and no selector pill painted there instead.
    assert!(text_in(&buf, geo.selector_bar, "SEARCH:"));
    assert!(text_in(&buf, geo.selector_bar, "Alp"));
    assert!(
        !text_in(&buf, geo.selector_bar, "\u{25e2}"),
        "no selector pill under the search box"
    );
    assert_eq!(
        buf[(panes.browser_panel.x, geo.selector_bar.y)].bg,
        pill_row_bg,
        "the pill row paints across the Browser pane too"
    );
    // The results occupy the list box through the canonical fixed-row
    // presentation; the carrier retained the row-flow rect the skeleton
    // gave it.
    assert!(text_in(&buf, geo.list_area, "Alpha"));
    assert_eq!(search.results().current_content_rect(), Some(geo.list_area));
    // The rest of the panel is unchanged: hero pane resting with its facts.
    assert_eq!(
        buf[(geo.hero.x, geo.hero.y)].bg,
        palette::surface_colors(palette::Surface::HeroPane, false).fill
    );
    assert!(text_in(&buf, geo.hero_area, "Dune"));
}

/// The canonical selected-row bar (spec: selected search results use the
/// canonical selected-row bar): focused, it spans the full row; unfocused,
/// the bar paints nowhere.
#[test]
fn search_results_paint_the_canonical_selected_row_bar() {
    let mut search = InlineSearch::new();
    search.open();
    search.set_pool(SearchPool::Items(vec![
        crate::app::tests::make_item("Alpha", "Movie"),
        crate::app::tests::make_item("Beta", "Movie"),
    ]));
    search.restore_query("a".into());
    let mut content = LibraryPanelContent {
        selector: None,
        list: ListSlot::Search(&mut search),
        hero: None,
    };
    let (focused_buf, focused_geo, _hits) = draw_skeleton(&mut content, true);
    let selected = focused_geo
        .selected
        .expect("the selected row's painted rect");
    // Full-width bar across the selected row's rect, inside the list box.
    for x in selected.left()..selected.right() {
        assert_eq!(
            focused_buf[(x, selected.y)].bg,
            palette::SELECTED_ROW_BG,
            "the selected-row bar spans the full row width"
        );
    }
    assert!(focused_geo.list_panel.x <= selected.x);
    assert!(selected.right() <= focused_geo.list_panel.right());

    let (resting_buf, _resting_geo, _hits) = draw_skeleton(&mut content, false);
    // The selected row is row 0; unfocused search results paint the bar
    // nowhere on it, so the row's own line is the discriminator.
    for x in selected.left()..selected.right() {
        assert_ne!(
            resting_buf[(x, selected.y)].bg,
            palette::SELECTED_ROW_BG,
            "unfocused search results paint the bar nowhere on the selected row"
        );
    }
}

/// Result rows carry the browser stripe like every other library list: the
/// panel's Wide policy sets the library column's fill as the stripe, so the
/// second row carries it over the box fill.
#[test]
fn search_result_rows_carry_the_browser_stripe() {
    let mut search = InlineSearch::new();
    search.open();
    search.set_pool(SearchPool::Items(vec![
        crate::app::tests::make_item("Alpha", "Movie"),
        crate::app::tests::make_item("Beta", "Movie"),
    ]));
    search.restore_query("a".into());
    let mut content = LibraryPanelContent {
        selector: None,
        list: ListSlot::Search(&mut search),
        hero: None,
    };
    let (buf, geo, _hits) = draw_skeleton(&mut content, false);
    // The ungrouped alternation opens on the box fill; the row below it
    // carries the stripe (the unfocused library column's backdrop).
    assert_eq!(
        buf[(geo.list_area.x, geo.list_area.y)].bg,
        palette::surface_colors(palette::Surface::LibraryPanel, false).fill
    );
    assert_eq!(
        buf[(geo.list_area.x, geo.list_area.y + 1)].bg,
        palette::surface_colors(palette::Surface::LibraryColumn, false).fill
    );
}

/// Zero rows paint the placeholder states (spec: loading while the corpus
/// fetch is outstanding, empty once the query is settled) and retain no
/// selected-row anchor geometry.
#[test]
fn zero_row_search_paints_the_placeholder_strings_and_no_anchor() {
    let mut search = InlineSearch::new();
    search.open();
    search.set_pool(SearchPool::Items(vec![crate::app::tests::make_item(
        "Alpha", "Movie",
    )]));
    let content = |search: &mut InlineSearch| {
        let mut content = LibraryPanelContent {
            selector: None,
            list: ListSlot::Search(search),
            hero: None,
        };
        draw_skeleton(&mut content, false)
    };
    // Loading: the corpus fetch is outstanding.
    search.set_loading(true);
    let (buf, geo, _hits) = content(&mut search);
    assert!(text_in(&buf, geo.list_area, " Loading\u{2026}"));
    assert!(geo.selected.is_none(), "zero rows retain no anchor rect");

    // Settled with no matches.
    search.set_loading(false);
    let (buf, geo, _hits) = content(&mut search);
    assert!(text_in(&buf, geo.list_area, " (empty)"));
    assert!(geo.selected.is_none());
}

/// A responsive Wide/Narrow transition keeps one session and one owner
/// (canonical-media-lists delta: presentation transition reuses the shared
/// owner): the same control's carrier retains the new geometry, the
/// selection survives, and the viewport clamps so the selection stays
/// visible — carrier-native, no row-local state copied into a second
/// control.
#[test]
fn presentation_transition_keeps_one_search_owner_across_wide_and_narrow() {
    // More rows than either painted list box can show, so both paints have
    // to resolve a clamped viewport for a selection parked past row 0.
    const ROWS: usize = 40;
    let mut search = InlineSearch::new();
    search.open();
    search.set_pool(SearchPool::Items(crate::app::tests::make_items(ROWS)));
    search.restore_query("ite".into());
    search
        .results_mut()
        .delegate_operation(crate::app::components::media_list::MediaListOperation::Last);
    let target = search
        .results()
        .selected_target()
        .cloned()
        .expect("a selected result after moving to the last row");

    // Wide: the session paints through the panel's one canonical list box.
    let wide_list_area;
    {
        let mut content = LibraryPanelContent {
            selector: None,
            list: ListSlot::Search(&mut search),
            hero: None,
        };
        let (_buf, wide_geo, _hits) = draw_skeleton(&mut content, true);
        wide_list_area = wide_geo.list_area;
    }
    assert_eq!(
        search.results().current_content_rect(),
        Some(wide_list_area)
    );
    assert_eq!(search.results().selected_target(), Some(&target));

    // Narrow: the same owner, the selection preserved, the viewport clamped
    // in place by the carrier's ordinary geometry-change rule.
    let narrow_area = Rect::new(0, 0, 60, 20);
    let mut terminal =
        Terminal::new(TestBackend::new(narrow_area.width, narrow_area.height)).unwrap();
    let mut hits = SkeletonHits::default();
    let mut windows = SkeletonPillWindows::default();
    let mut narrow_geo = None;
    {
        let mut content = LibraryPanelContent {
            selector: None,
            list: ListSlot::Search(&mut search),
            hero: None,
        };
        terminal
            .draw(|f| {
                narrow_geo = Some(
                    crate::app::components::library_panel::render_narrow_skeleton(
                        f,
                        narrow_area,
                        &mut content,
                        true,
                        None,
                        &mut hits,
                        &mut windows,
                    ),
                );
            })
            .unwrap();
    }
    let narrow_geo = narrow_geo.expect("narrow skeleton painted");
    assert_ne!(narrow_geo.list_area, wide_list_area, "the geometry changed");
    assert_eq!(
        search.results().current_content_rect(),
        Some(narrow_geo.list_area),
        "the same owner retained the narrow geometry"
    );
    assert_eq!(
        search.results().selected_target(),
        Some(&target),
        "the selection survived the transition"
    );
    let offset = search
        .results()
        .current_flow_offset()
        .expect("the narrow paint retained the row flow");
    // Exact clamp (canonical-media-lists delta: the shared owner's
    // geometry-change rule): the result flow is flat — one display row per
    // pool item — and the selection is the last row, so the narrow box's
    // painted row capacity must leave exactly `rows - viewport_height`
    // display rows above the viewport top; nothing else keeps the selection
    // visible.
    let viewport_height = narrow_geo.list_area.height as usize;
    assert!(
        ROWS > viewport_height,
        "the fixture must overflow the narrow list box"
    );
    let selected_row = search.results().rows().len() - 1;
    assert_eq!(selected_row, ROWS - 1, "the selection is the last result");
    let expected_offset = selected_row + 1 - viewport_height;
    assert_eq!(
        offset, expected_offset,
        "the clamped viewport parks the selection on its last visible row"
    );
    assert!(
        offset <= selected_row && selected_row < offset + viewport_height,
        "the clamped viewport keeps the selection visible"
    );
    // And the retained selected-row geometry sits inside that viewport.
    let selected_rect = search
        .results()
        .current_selected_row_rect()
        .expect("the selected row's retained rect");
    assert!(
        narrow_geo.list_area.left() <= selected_rect.left()
            && selected_rect.right() <= narrow_geo.list_area.right()
            && narrow_geo.list_area.top() <= selected_rect.top()
            && selected_rect.bottom() <= narrow_geo.list_area.bottom(),
        "the selected row's rect lies inside the retained narrow viewport"
    );
    assert_eq!(
        search.query(),
        "ite",
        "the session stayed open with its query"
    );
}

/// A closed search session paints no search surface anywhere in the frame
/// and retains no hit geometry (task 6.3). The session is painted open
/// first, so its carrier really holds a retained content rect — the close,
/// not a missing paint, is what clears it: the retained rect is gone, no
/// search bar is painted, a point inside the old rect resolves to nothing
/// through the carrier, and the browse list still paints the frame.
#[test]
fn closed_search_paints_no_search_surface_and_no_hit_geometry() {
    let mut search = InlineSearch::new();
    search.open();
    search.set_pool(SearchPool::Items(vec![crate::app::tests::make_item(
        "Alpha", "Movie",
    )]));
    search.restore_query("Alp".into());

    // Paint the open session through the panel's one canonical list box so
    // the carrier retains its painted hit geometry.
    let old_rect;
    {
        let mut open_content = LibraryPanelContent {
            selector: Some(SelectorRow {
                pills: vec!["All".into()],
                markers: vec![],
                active: Some(0),
            }),
            list: ListSlot::Search(&mut search),
            hero: None,
        };
        let (_buf, open_geo, _hits) = draw_skeleton(&mut open_content, false);
        old_rect = search
            .results()
            .current_content_rect()
            .expect("the painted session retained its hit geometry");
        assert_eq!(old_rect, open_geo.list_area);
    }

    search.close();
    assert!(
        search.results().current_content_rect().is_none(),
        "closing the session clears the carrier's retained rect"
    );

    // Repaint with the browse list: the closed session adds no surface.
    let mut list = StubList::with_rows(vec!["Alpha"]);
    let mut content = LibraryPanelContent {
        selector: Some(SelectorRow {
            pills: vec!["All".into()],
            markers: vec![],
            active: Some(0),
        }),
        list: ListSlot::Media(&mut list),
        hero: None,
    };
    let (buf, geo, _hits) = draw_skeleton(&mut content, false);
    let frame = Rect::new(0, 0, AREA.width, AREA.height);
    assert!(!text_in(&buf, frame, "SEARCH:"), "no search bar anywhere");
    assert!(
        text_in(&buf, geo.selector_bar, "\u{25e2}"),
        "the selector row paints its pill"
    );
    assert!(
        text_in(&buf, geo.list_area, "Alpha"),
        "the browse list paints"
    );
    let inside_old = ratatui::layout::Position {
        x: old_rect.x + old_rect.width / 2,
        y: old_rect.y + old_rect.height / 2,
    };
    assert!(!search.results().claims_current_point(inside_old));
    assert!(
        search.results().resolve_current_point(inside_old).is_none(),
        "a point inside the old rect resolves to nothing after the close"
    );
}

#[test]
fn empty_list_slot_paints_its_placeholder_in_the_list_box() {
    let mut content = LibraryPanelContent {
        selector: None,
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
                80, // tall terminal: the short-pane caps must not skew the skeleton tests
                true,
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
