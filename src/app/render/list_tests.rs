use super::*;
use crate::app::layout::LayoutMain;
use crate::app::render::list_rows::selected_cell_rect;
use crate::app::tests::{make_app_stub, make_item};
use crate::app::{BrowseLevel, LibraryTab, TabSelection};
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;

fn buffer_to_string(term: &Terminal<TestBackend>) -> String {
    let buf = term.backend().buffer();
    let area = *buf.area();
    let mut out = String::new();
    for y in 0..area.height {
        for x in 0..area.width {
            out.push_str(buf[(x, y)].symbol());
        }
        out.push('\n');
    }
    out
}

#[test]
fn selected_cell_rect_matches_one_and_two_column_geometry() {
    let area = Rect::new(10, 4, 30, 8);
    let rows = vec![vec![0], vec![1, 2], vec![3, 4]];
    assert_eq!(
        selected_cell_rect(area, 0, &rows, 0, 1, 30, 0),
        Some(Rect::new(10, 4, 30, 1))
    );
    assert_eq!(
        selected_cell_rect(area, 2, &rows, 0, 2, 14, 2),
        Some(Rect::new(26, 5, 14, 1))
    );
    assert_eq!(selected_cell_rect(area, 4, &rows, 3, 2, 14, 2), None);
}

fn render_list_to_string(app: &mut App, layout: &mut LayoutMain) -> String {
    let backend = TestBackend::new(60, 8);
    let mut term = Terminal::new(backend).unwrap();
    term.draw(|f| {
        app.render_list(f, Rect::new(0, 0, 60, 8), true, layout);
    })
    .unwrap();
    buffer_to_string(&term)
}

fn make_movie_list_app(titles: Vec<&str>) -> App {
    let mut app = make_app_stub();
    app.tab = TabSelection::EmbyLibrary(0);

    let mut library = make_item("Movies", "CollectionFolder");
    library.id = "lib-movies".into();
    library.is_folder = true;
    library.collection_type = "movies".into();

    let items: Vec<_> = titles
        .into_iter()
        .enumerate()
        .map(|(i, title)| {
            let mut m = make_item(title, "Movie");
            m.id = format!("movie-{i}");
            if title.contains("Selected") {
                m.overview = "This is the compact movie banner overview text.".into();
                m.production_year = 2024;
                m.runtime_ticks = 90 * mbv_core::api::TICKS_PER_SECOND;
            }
            m
        })
        .collect();
    let total = items.len();

    app.libs.push(LibraryTab {
        library,
        search: None,
        nav_stack: vec![BrowseLevel {
            parent_id: "lib-movies".into(),
            title: "Movies".into(),
            items,
            total_count: total,
            cursor: 0,
            scroll: 0,
            item_types: None,
            unplayed_only: false,
            sort_by: "SortName".into(),
            sort_order: "Ascending".into(),
            loading: false,
            all_items: None,
            letter_filter: None,
            music_grouping: None,
        }],
        feed_home_video: None,

        album_track_focus: None,
        series_selection: None,
        series_season_cursor: 0,
        library_total: None,
    });

    app
}

#[test]
fn compact_banner_prefetches_nearby_movies_but_not_beyond_the_window() {
    let titles: Vec<&str> = vec![
        "Movie 0", "Movie 1", "Movie 2", "Movie 3", "Movie 4", "Movie 5",
    ];
    let mut app = make_movie_list_app(titles);
    app.image_protocol_enabled = true;

    let mut layout = LayoutMain::default();
    let _ = render_list_to_string(&mut app, &mut layout);

    let fetch_triggered = |app: &App, key: &str| {
        app.card_image_loading.contains(key) || app.card_image_states.contains_key(key)
    };

    let selected_key = compact_banner_image_cache_key("movie-0");
    assert!(
        fetch_triggered(&app, &selected_key),
        "expected the selected movie's own image fetch to still be triggered"
    );

    for i in 1..=3 {
        let key = compact_banner_image_cache_key(&format!("movie-{i}"));
        assert!(
            fetch_triggered(&app, &key),
            "expected movie-{i} to be prefetched (within the prefetch window)"
        );
    }

    let outside_key = compact_banner_image_cache_key("movie-4");
    assert!(
        !fetch_triggered(&app, &outside_key),
        "movie-4 is outside the prefetch window and should not have been fetched"
    );
}

// ── Two-column list layout (library-list-columns) ─────────────────────────

/// Renders the list at an explicit width/height and returns the
/// terminal for buffer (background) inspection.
fn render_list_term(
    app: &mut App,
    layout: &mut LayoutMain,
    width: u16,
    height: u16,
) -> Terminal<TestBackend> {
    let backend = TestBackend::new(width, height);
    let mut term = Terminal::new(backend).unwrap();
    term.draw(|f| {
        app.render_list(f, Rect::new(0, 0, width, height), true, layout);
    })
    .unwrap();
    term
}

/// Same as `make_movie_list_app`, but with a music collection type so
/// no inline movie banner / series detail is attached to the selection (the
/// compact banner only appears for leaf Movies in movies/homevideos/podcasts
/// collections, and series detail only in tvshows).
fn make_no_banner_list_app(titles: Vec<&str>) -> App {
    let mut app = make_movie_list_app(titles);
    app.libs[0].library.collection_type = "music".into();
    app
}

/// Item rows (non-empty entries of `left_item_rows`): the packed rows of a
/// two-column list, independent of banner/header filler rows.
fn item_rows(layout: &LayoutMain) -> Vec<Vec<usize>> {
    layout
        .left_item_rows
        .iter()
        .filter(|r| !r.is_empty())
        .cloned()
        .collect()
}

fn cursor_of(app: &App) -> usize {
    app.libs[0].nav_stack.last().unwrap().cursor
}

/// The render helpers write into a local `LayoutMain`, but cursor-movement
/// and mouse code read `app.layout` (the App's own layout, which only a
/// full-frame `render` swaps in). Copy the fields those paths consult so
/// the tests exercise the real production code paths.
fn sync_layout_to_app(app: &mut App, layout: &LayoutMain) {
    app.layout.main.left_area = layout.left_area;
    app.layout.main.left_item_rows = layout.left_item_rows.clone();
    app.layout.main.left_sorted_indices = layout.left_sorted_indices.clone();
    app.layout.main.left_row_map = layout.left_row_map.clone();
}

#[test]
fn two_columns_pack_items_row_major_left_to_right_before_wrapping() {
    // TV (legacy top placement two-column) still packs row-major at wide widths; the
    // dedicated Movies library moved to hero-on-left one-column (tested in
    // `movies_wide_tests.rs`). A tvshows library with Movie-type items has
    // no selected series/movie hero, exercising the plain two-column list.
    let mut app = make_movie_list_app(vec!["A", "B", "C", "D", "E", "F"]);
    app.libs[0].library.collection_type = "tvshows".into();
    let mut layout = LayoutMain::default();
    let _ = render_list_term(&mut app, &mut layout, 82, 8);
    assert_eq!(
        item_rows(&layout),
        vec![vec![0, 1], vec![2, 3], vec![4, 5]],
        "item i occupies column i % 2 of row i / 2"
    );
}

#[test]
fn letter_buckets_pack_independently_with_an_odd_sized_bucket() {
    let mut app = make_movie_list_app(vec![
        "Aardvark", "Alpha", "Apple", "Banana", "Beta", "Cherry",
    ]);
    // TV keeps legacy top placement at wide widths (unlike Movies), so letter buckets
    // still pack two per row there.
    app.libs[0].library.collection_type = "tvshows".into();
    // >= 250 switches `letter_bucket` to per-letter headers, giving an odd
    // three-item A bucket to prove ragged trailing cells.
    app.libs[0].library_total = Some(250);
    let mut layout = LayoutMain::default();
    let _ = render_list_term(&mut app, &mut layout, 82, 20);
    assert_eq!(
        item_rows(&layout),
        vec![vec![0, 1], vec![2], vec![3, 4], vec![5]],
        "each bucket starts a fresh item row; no row mixes two buckets"
    );
}

#[test]
fn two_column_cursor_deltas_wrap_rows_and_clamp_at_list_end() {
    // Tall enough viewport that the 18-row hero block leaves real list rows
    // below it, so `lib_page_size` reflects the list, not 0. TV keeps the
    // legacy top placement two-column list at wide widths (Movies moved to
    // hero-on-left one-column).
    let mut app = make_movie_list_app(vec!["M0", "M1", "M2", "M3", "M4", "M5", "M6"]);
    app.libs[0].library.collection_type = "tvshows".into();
    let mut layout = LayoutMain::default();
    let _ = render_list_term(&mut app, &mut layout, 82, 30);
    sync_layout_to_app(&mut app, &layout);
    let cur = cursor_of;

    // Left/right: ±1.
    app.move_lib_cursor(0, 1);
    assert_eq!(cur(&app), 1);
    app.move_lib_cursor(0, -1);
    assert_eq!(cur(&app), 0);
    // Row-boundary wrap: right from the last cell of a row wraps to the
    // next row's first item, left wraps back.
    app.move_lib_cursor(0, 1);
    app.move_lib_cursor(0, 1);
    assert_eq!(cur(&app), 2, "right from cell 1 wraps to the next row");
    app.move_lib_cursor(0, -1);
    assert_eq!(cur(&app), 1, "left from cell 0 wraps to the previous row");
    // Up/down: ±cols.
    app.move_lib_cursor_rows(0, -1);
    assert_eq!(cur(&app), 0);
    app.move_lib_cursor_rows(0, 1);
    assert_eq!(cur(&app), 2);
    // Down from the second-to-last row with no item directly below clamps
    // to the last item (5 -> 6).
    app.libs[0].nav_stack.last_mut().unwrap().cursor = 5;
    app.move_lib_cursor_rows(0, 1);
    assert_eq!(
        cur(&app),
        6,
        "down past the ragged end clamps to the last item"
    );
    // End-of-list clamp on right.
    app.move_lib_cursor(0, 1);
    assert_eq!(cur(&app), 6);
    // Paging: one viewport of item rows, clamped at both ends.
    app.libs[0].nav_stack.last_mut().unwrap().cursor = 0;
    let page = app.lib_page_size();
    app.move_lib_cursor_rows(0, -(page as i64));
    assert_eq!(cur(&app), 0, "page up from the top stays");
    app.move_lib_cursor_rows(0, page as i64);
    assert_eq!(
        cur(&app),
        6,
        "page down past the end clamps to the last item"
    );
}

// ── Top hero area (legacy top placement) ─────────────────────────────────────────

#[test]
fn left_area_is_set_for_an_empty_library_list() {
    let mut app = make_movie_list_app(vec![]);
    let mut layout = LayoutMain::default();
    let _ = render_list_term(&mut app, &mut layout, 82, 40);

    assert!(
        layout.left_area.height > 0,
        "left_area must be set even when the library list is empty, so clicking it can focus the panel"
    );
    assert!(
        layout.left_area.width > 0,
        "left_area must have nonzero width"
    );
}

#[test]
fn list_area_renders_the_same_per_cell_content_at_one_and_two_columns() {
    // width 81 stays under TWO_COLUMN_THRESHOLD (82) -> 1 col; 82
    // crosses it -> 2 col. Same items, same order; only the packing shape
    // should differ, not which item occupies which position.
    let titles: Vec<&str> = vec!["Movie A", "Movie B", "Movie C", "Movie D"];
    let mut app_1col = make_no_banner_list_app(titles.clone());
    let mut layout_1col = LayoutMain::default();
    let _ = render_list_term(&mut app_1col, &mut layout_1col, 81, 12);

    let mut app_2col = make_no_banner_list_app(titles);
    let mut layout_2col = LayoutMain::default();
    let _ = render_list_term(&mut app_2col, &mut layout_2col, 82, 12);

    let flat_1col: Vec<usize> = item_rows(&layout_1col).into_iter().flatten().collect();
    let flat_2col: Vec<usize> = item_rows(&layout_2col).into_iter().flatten().collect();
    assert_eq!(
        flat_1col, flat_2col,
        "the same items in the same order must appear in list_area regardless of column count"
    );
}

#[test]
fn hero_paints_inline_in_two_column_mode() {
    let mut app = make_movie_list_app(vec!["Movie 0", "Movie 1 Selected"]);
    app.libs[0].nav_stack.last_mut().unwrap().cursor = 1;
    let mut layout = LayoutMain::default();
    let term = render_list_term(&mut app, &mut layout, 81, 40);
    let output = buffer_to_string(&term);

    assert!(layout.hero_area.height > 0, "hero should be shown");
    assert!(
        layout.hero_area.y > layout.left_area.y,
        "inline hero follows the active media row"
    );
    assert!(
        layout.hero_area.y + layout.hero_area.height <= 40,
        "inline hero remains inside the scrolling list"
    );
    let active_row = layout.selected_item_rect.unwrap().y as usize;
    assert!(
        !output
            .lines()
            .nth(active_row)
            .unwrap()
            .contains("Movie 1 Selected")
            && !output.lines().nth(active_row).unwrap().contains("2024")
            && !output.lines().nth(active_row).unwrap().contains("1h"),
        "the active row content belongs to the inline hero: row={active_row} line={:?}",
        output.lines().nth(active_row).unwrap()
    );
    assert!(
        output
            .lines()
            .skip(layout.hero_area.y as usize)
            .take(layout.hero_area.height as usize)
            .any(|line| line.contains("Movie 1 Selected")),
        "the inline hero contains the selected title"
    );
    assert_eq!(
        layout.left_area.y + layout.left_area.height,
        40,
        "list area retains the full content height"
    );
}

#[test]
fn letter_pills_render_above_inline_list_hero() {
    let mut app = make_movie_list_app(vec!["Movie 0", "Movie 1 Selected"]);
    app.libs[0].nav_stack.last_mut().unwrap().cursor = 1;
    // Any captured library total qualifies a top-level, non-music library
    // for the letter-range pill row (`should_show_letter_pills`).
    app.libs[0].library_total = Some(1000);
    let mut layout = LayoutMain::default();
    let _ = render_list_term(&mut app, &mut layout, 81, 40);

    assert!(layout.hero_area.height > 0, "hero should be shown");
    assert!(
        !layout.selector_tabs.is_empty(),
        "letter pills should render for a large top-level library"
    );
    let pills_y = layout.selector_tabs[0].0.y;
    assert_eq!(
        layout.left_area.y,
        pills_y + 2,
        "pill row sits above the scrolling list"
    );
    assert!(
        layout.hero_area.y > pills_y,
        "list_area must sit below the pill row"
    );
}

#[test]
fn hero_height_is_constant_above_the_image_cap() {
    // The inline hero stays bounded and leaves usable list space.
    for width in [40u16, 60, 81] {
        let mut app = make_movie_list_app(vec!["Movie 0", "Movie 1 Selected"]);
        app.libs[0].nav_stack.last_mut().unwrap().cursor = 1;
        let mut layout = LayoutMain::default();
        let _ = render_list_term(&mut app, &mut layout, width, 40);
        assert!(
            layout.hero_area.height <= 23,
            "hero height at width {width} should stay bounded, got {}",
            layout.hero_area.height
        );
        assert!(
            layout.left_area.height >= 1,
            "list area must keep at least 1 row at width {width}"
        );
    }
}

#[test]
fn hero_sizes_to_content_when_a_movie_is_selected() {
    // A selected Movie's banner sizes the panel from its own content
    // (poster + meta + overview), not from the fixed placeholder reserved
    // while the slice is loading -- the placeholder is only the stand-in
    // for the no-content state. Below the breakpoint Movies still uses this
    // legacy top placement path (the wide hero-on-left card sizes independently;
    // covered in `movies_wide_tests.rs`).
    let mut app = make_movie_list_app(vec!["Movie 0", "Movie 1 Selected"]);
    app.libs[0].nav_stack.last_mut().unwrap().cursor = 1;
    let mut layout = LayoutMain::default();
    let _ = render_list_term(&mut app, &mut layout, 81, 40);

    let item = app.libs[0].nav_stack.last().unwrap().items[1].clone();
    let panel_width = 81 - 2 * super::SELECTED_BLOCK_SIDE_PADDING;
    let content_rows = app
        .compact_banner_layout_with_overview(&item, panel_width, false)
        .content_rows() as u16;
    let cols = crate::app::library_column_width::library_column_count(81);
    let expected = content_rows
        + super::HERO_TITLE_ROWS.saturating_mul((cols > 1) as u16)
        + super::HERO_BLOCK_EXTRA_ROWS;
    assert_eq!(
        layout.hero_area.height, expected,
        "selected Movie banner sizes the panel to its own content"
    );
    assert_ne!(
        layout.hero_area.height,
        super::HERO_PLACEHOLDER_ROWS,
        "the placeholder is only the no-content stand-in"
    );
}

#[test]
fn hero_stays_reserved_while_the_slice_is_loading() {
    // No active media row means there is no inline hero to reserve.
    let mut app = make_movie_list_app(vec!["Movie 0", "Movie 1 Selected"]);
    app.libs[0].nav_stack.last_mut().unwrap().items.clear();
    app.libs[0].nav_stack.last_mut().unwrap().loading = true;
    let mut layout = LayoutMain::default();
    let _ = render_list_term(&mut app, &mut layout, 81, 40);

    assert_eq!(
        layout.hero_area.height, 0,
        "hero is suppressed with empty, loading items"
    );
    assert!(
        layout.left_area.height >= 1,
        "list area must keep at least 1 row while loading"
    );
}

#[test]
fn music_library_top_level_reserves_hero_placeholder() {
    // The top browse level of a hero-capable collection (music included)
    // keeps the placeholder panel reserved even before content loads, so
    // a letter-pill switch doesn't make the slot jump away and back.
    let mut app = make_movie_list_app(vec!["Album A", "Album B"]);
    app.libs[0].library.collection_type = "music".into();
    let mut layout = LayoutMain::default();
    let _ = render_list_term(&mut app, &mut layout, 82, 40);

    assert_eq!(
        layout.hero_area.height,
        super::HERO_PLACEHOLDER_ROWS,
        "a music library at its top browse level reserves the hero placeholder"
    );
}

#[test]
fn inline_detail_flow_accounts_for_detail_rows_and_scroll() {
    let flow = super::super::hero::inline_detail_flow(7, 4, 8, 0)
        .expect("a detail block plus its active row should fit");

    assert_eq!(flow.offset, 4);
    assert_eq!(flow.detail_screen_row, 4);
}

#[test]
fn inline_detail_is_inserted_after_the_selected_media_row() {
    let mut app = make_movie_list_app(vec!["Movie 0", "Movie 1 Selected", "Movie 2"]);
    app.libs[0].nav_stack.last_mut().unwrap().cursor = 1;
    let mut layout = LayoutMain::default();
    let _ = render_list_term(&mut app, &mut layout, 81, 40);

    let selected = layout.selected_item_rect.expect("selected row is visible");
    assert!(
        layout.hero_area.height > 0,
        "selected detail should be rendered"
    );
    assert!(
        layout.hero_area.y >= selected.y + selected.height,
        "inline detail must follow the selected media row: selected={selected:?}, detail={:?}",
        layout.hero_area
    );
}

#[test]
fn inline_detail_height_tracks_variable_selected_content() {
    let mut short = make_movie_list_app(vec!["Movie 0", "Movie 1 Selected"]);
    short.libs[0].nav_stack.last_mut().unwrap().cursor = 1;
    short.libs[0].nav_stack.last_mut().unwrap().items[1].overview = "Short".into();
    let mut short_layout = LayoutMain::default();
    let _ = render_list_term(&mut short, &mut short_layout, 81, 40);

    let mut long = make_movie_list_app(vec!["Movie 0", "Movie 1 Selected"]);
    long.libs[0].nav_stack.last_mut().unwrap().cursor = 1;
    long.libs[0].nav_stack.last_mut().unwrap().items[1].overview =
        "A long overview that must occupy additional wrapped rows in the selected detail block."
            .into();
    let mut long_layout = LayoutMain::default();
    let _ = render_list_term(&mut long, &mut long_layout, 81, 40);

    assert!(
        long_layout.hero_area.height > short_layout.hero_area.height,
        "inline detail height must follow content: short={}, long={}",
        short_layout.hero_area.height,
        long_layout.hero_area.height
    );
}

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
fn inline_detail_scroll_keeps_selected_row_and_detail_in_the_viewport() {
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
    assert!(layout.hero_area.y >= selected.y + selected.height);
    assert!(layout.hero_area.y + layout.hero_area.height <= 16);
}

#[test]
fn inline_hero_rows_are_inert_and_do_not_add_media_targets() {
    let mut app = make_movie_list_app(vec!["Movie 0", "Movie 1 Selected", "Movie 2"]);
    app.libs[0].nav_stack.last_mut().unwrap().cursor = 1;
    let mut layout = LayoutMain::default();
    let _ = render_list_term(&mut app, &mut layout, 81, 40);

    let selected_row = layout
        .left_row_map
        .iter()
        .position(|row| *row == Some(1))
        .expect("selected media row should be mapped");
    let following_row = layout
        .left_row_map
        .iter()
        .skip(selected_row + 1)
        .position(|row| row == &Some(2))
        .map(|offset| selected_row + 1 + offset)
        .expect("following media row should be mapped");
    assert!(
        layout.left_row_map[selected_row + 1..following_row]
            .iter()
            .all(Option::is_none),
        "hero-only rows must remain inert"
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
    // legacy top placement fallback) must still show that item's title even though
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

// Regression gate for the `show_grouped` fix (design.md Decision 6): without
// the `search.is_none()` guard, `render_grouped_album_rows` would pair
// its unfiltered `GroupedAlbumCatalog` with the filtered search-result
// vector, mislabeling every row. This pins the guard's two inputs directly
// rather than the rendered output, since `show_grouped` itself is a local
// binding inside `render_list`, not a standalone function.
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
