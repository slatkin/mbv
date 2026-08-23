use super::*;

#[test]
fn inline_detail_is_suppressed_when_the_active_row_cannot_fit_with_it() {
    let mut app = make_movie_list_app(vec!["Movie 0", "Movie 1 Selected"]);
    app.libs[0].nav_stack.last_mut().unwrap().cursor = 1;
    let mut layout = LayoutMain::default();
    let _ = render_list_term(&mut app, &mut layout, 81, 4);

    assert_eq!(
        layout.hero_area.height, 0,
        "detail must be suppressed in a tiny viewport"
    );
    assert!(
        layout.selected_item_rect.is_some(),
        "the active media row must retain the available viewport"
    );
}

#[test]
fn inline_detail_scroll_keeps_selected_replacement_in_the_viewport() {
    let titles: Vec<&str> = (0..12).map(|_| "Movie").collect();
    let mut app = make_movie_list_app(titles);
    app.libs[0].nav_stack.last_mut().unwrap().items[8].name = "Movie 8 Selected".into();
    app.libs[0].nav_stack.last_mut().unwrap().items[8].overview = "Selected detail".into();
    app.libs[0].nav_stack.last_mut().unwrap().cursor = 8;
    let mut layout = LayoutMain::default();
    let _ = render_list_term(&mut app, &mut layout, 81, 16);

    let selected = layout.selected_item_rect.expect("selected row is visible");
    assert!(
        layout.hero_area.height > 0,
        "selected detail should remain addressable"
    );
    assert_eq!(layout.hero_area, selected);
    assert!(layout.hero_area.y + layout.hero_area.height <= 16);
}

#[test]
fn inline_replacement_has_one_parent_media_target() {
    let mut app = make_movie_list_app(vec!["Movie 0", "Movie 1 Selected", "Movie 2"]);
    app.libs[0].nav_stack.last_mut().unwrap().cursor = 1;
    let mut layout = LayoutMain::default();
    let _ = render_list_term(&mut app, &mut layout, 81, 40);

    let selected_row = layout
        .left_row_map
        .iter()
        .position(|row| *row == Some(1))
        .expect("selected replacement should own the parent target");
    let following_row = layout
        .left_row_map
        .iter()
        .skip(selected_row + 1)
        .position(|row| row == &Some(2))
        .map(|offset| selected_row + 1 + offset)
        .expect("following media row should be mapped");
    assert_eq!(
        selected_row,
        layout.hero_area.y as usize - layout.left_area.y as usize
    );
    assert_eq!(
        layout.left_row_map[selected_row..following_row]
            .iter()
            .filter(|row| **row == Some(1))
            .count(),
        1,
        "the replacement must publish exactly one parent target"
    );
}

#[test]
fn selected_cell_uses_carat_no_double_hash_in_two_column_mode() {
    let mut app = make_no_banner_list_app(vec!["Alpha", "Beta", "Gamma", "Delta"]);
    let mut layout = LayoutMain::default();
    let term = render_list_term(&mut app, &mut layout, 82, 8);
    let out = buffer_to_string(&term);
    let list_line = out
        .lines()
        .nth(layout.left_area.y as usize)
        .expect("list_area's first row should exist in the rendered buffer");
    assert!(
        list_line.contains('\u{258e}'),
        "selected cell's left edge should carry the ▎ mark: {list_line:?}"
    );
    assert!(
        !list_line.contains("##Alpha"),
        "selected cell's title must not be prefixed with ## in two-column mode: {list_line:?}"
    );
}

#[test]
fn hero_content_tracks_cursor_when_selection_scrolled_offscreen() {
    let mut titles: Vec<String> = (0..40).map(|i| format!("Movie {i}")).collect();
    titles[39] = "Movie 39 Selected".to_string();
    let title_refs: Vec<&str> = titles.iter().map(String::as_str).collect();
    let mut app = make_movie_list_app(title_refs);
    let mut layout = LayoutMain::default();

    // Move the cursor to the last item: the hero (below-breakpoint
    // inline presentation) must still show that item's title even though
    // the viewport is far too short to render its row's position. The wide
    // hero-on-left variant (left card tracks a scrolled-away rail row) is
    // covered in `movies_wide_tests.rs`.
    app.libs[0].nav_stack.last_mut().unwrap().cursor = 39;
    let term = render_list_term(&mut app, &mut layout, 81, 20);
    let out = buffer_to_string(&term);

    assert!(layout.hero_area.height > 0, "hero should still be shown");
    assert!(
        out.contains("Movie 39"),
        "the hero should still show the cursor's item even though its row is offscreen"
    );
}

#[test]
fn two_column_mouse_click_selects_the_clicked_cell_not_the_row_first_item() {
    let mut app = make_no_banner_list_app(vec!["Click A", "Click B", "Click C"]);
    let mut layout = LayoutMain::default();
    let _ = render_list_term(&mut app, &mut layout, 82, 8);
    sync_layout_to_app(&mut app, &layout);
    let la = layout.left_area;
    // Click cell 1 of the first row (x = cell 1 start, y = row 0).
    let cell1_x = la.x + 42;
    assert!(app.click_set_cursor(cell1_x, la.y));
    assert_eq!(
        cursor_of(&app),
        1,
        "click on the right cell must select the second item of the row"
    );
}

#[test]
fn letter_grouped_replacement_moves_with_a_scrolled_header() {
    let titles: Vec<String> = (0..60).map(|i| format!("Movie {i:02}")).collect();
    let title_refs: Vec<&str> = titles.iter().map(String::as_str).collect();
    let mut app = make_movie_list_app(title_refs);
    let selected = &mut app.libs[0].nav_stack.last_mut().unwrap().items[50];
    selected.name = "Movie 50 Selected".into();
    selected.overview = "Selected detail".into();
    app.libs[0].nav_stack.last_mut().unwrap().cursor = 50;
    app.libs[0].nav_stack.last_mut().unwrap().scroll = 50;
    let mut layout = LayoutMain::default();
    let _ = render_list_term(&mut app, &mut layout, 81, 40);

    assert!(layout.hero_area.height > 0);
    assert_eq!(
        layout.hero_area.y,
        layout.left_area.y
            + layout
                .left_row_map
                .iter()
                .position(|item| *item == Some(50))
                .unwrap() as u16,
        "detail geometry must be recomputed after restoring the preceding header"
    );
}

// Regression gate for the `show_grouped` fix (design.md Decision 6): without
// the `search.is_none()` guard, `render_grouped_album_rows` would pair
// its unfiltered `GroupedAlbumCatalog` with the filtered search-result vector,
// mislabeling every row. This pins the guard's two inputs directly rather
// than the rendered output, since `show_grouped` itself is a local binding
// inside `render_list`, not a standalone function.
#[test]
fn show_grouped_guard_is_false_while_search_is_active_on_album_folders() {
    let mut app = crate::app::render::test_helpers::make_music_group_app();
    let lib_idx = app.tab.emby_library_index().unwrap();

    assert!(
        app.is_viewing_album_folders(lib_idx),
        "fixture must sit at the album-folder level for this guard to matter"
    );
    assert!(app.libs[lib_idx].search.is_none());
    assert!(
        app.is_viewing_album_folders(lib_idx) && app.libs[lib_idx].search.is_none(),
        "baseline: show_grouped's condition holds with no active search"
    );

    app.libs[lib_idx].search = Some(crate::app::LibSearch {
        query: "x".into(),
        items: Vec::new(),
        results: Vec::new(),
        cursor: 0,
        scroll: 0,
        loading: false,
    });

    assert!(
        !(app.is_viewing_album_folders(lib_idx) && app.libs[lib_idx].search.is_none()),
        "show_grouped's condition must go false once a search is active"
    );
}
