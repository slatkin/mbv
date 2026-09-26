use super::*;

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
    };
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
            .unwrap()
    };
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
    let offset = search.results().scroll();
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
    };

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
