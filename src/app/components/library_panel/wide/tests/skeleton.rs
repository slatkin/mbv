use super::*;

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
fn no_selector_reclaims_the_gap_row_for_the_browser_and_hero() {
    let mut list = StubList::with_rows(vec!["Alpha"]);
    let mut content = LibraryPanelContent {
        selector: None,
        list: ListSlot::Media(&mut list),
        hero: Some(HeroContent {
            facts: hero_facts("Dune"),
            overview: None,
            credits: None,
            workspace: None,
        }),
    };
    let (buf, geometry, _hits) = draw_skeleton(&mut content, false);
    let panes = wide_library_panes_with_selector(AREA, PANE_PAD_X, PANE_PAD_Y, None, true, false)
        .expect("wide area");

    assert_eq!(panes.spacer_area.height, 0);
    assert_eq!(geometry.browser.y, AREA.y);
    assert_eq!(geometry.hero.y, AREA.y);
    assert_eq!(geometry.browser.height, AREA.height);
    assert_eq!(geometry.hero.height, AREA.height);
    assert_eq!(list.painted, Some(geometry.list_area));
    assert!(text_in(&buf, geometry.list_area, "Alpha"));
    assert_eq!(
        buf[(geometry.browser.x, geometry.browser.y)].bg,
        palette::surface_colors(palette::Surface::LibraryPanel, false).fill
    );
    assert_eq!(
        buf[(geometry.hero.x, geometry.hero.y)].bg,
        palette::surface_colors(palette::Surface::HeroPane, false).fill
    );
}

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
