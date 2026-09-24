use super::*;

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
