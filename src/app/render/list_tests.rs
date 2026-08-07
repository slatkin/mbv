use super::*;
use crate::app::layout::LayoutMain;
use crate::app::tests::{make_app_stub, make_item};
use crate::app::{BrowseLevel, LibraryTab};
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

fn render_power_list_to_string(app: &mut App, layout: &mut LayoutMain) -> String {
    let backend = TestBackend::new(60, 8);
    let mut term = Terminal::new(backend).unwrap();
    term.draw(|f| {
        app.render_power_list(f, Rect::new(0, 0, 60, 8), true, layout);
    })
    .unwrap();
    buffer_to_string(&term)
}

fn make_power_movie_list_app(titles: Vec<&str>) -> App {
    let mut app = make_app_stub();
    app.library_tab = 1;

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
            }
            m
        })
        .collect();
    let total = items.len();

    app.libs.push(LibraryTab {
        library,
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
        artist_header_focus: None,
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
    let mut app = make_power_movie_list_app(titles);
    app.image_protocol_enabled = true;

    let mut layout = LayoutMain::default();
    let _ = render_power_list_to_string(&mut app, &mut layout);

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

/// Renders the power list at an explicit width/height and returns the
/// terminal for buffer (background) inspection.
fn render_power_list_term(
    app: &mut App,
    layout: &mut LayoutMain,
    width: u16,
    height: u16,
) -> Terminal<TestBackend> {
    let backend = TestBackend::new(width, height);
    let mut term = Terminal::new(backend).unwrap();
    term.draw(|f| {
        app.render_power_list(f, Rect::new(0, 0, width, height), true, layout);
    })
    .unwrap();
    term
}

/// Same as `make_power_movie_list_app`, but with a music collection type so
/// no inline movie banner / series detail is attached to the selection (the
/// compact banner only appears for leaf Movies in movies/homevideos/podcasts
/// collections, and series detail only in tvshows).
fn make_no_banner_list_app(titles: Vec<&str>) -> App {
    let mut app = make_power_movie_list_app(titles);
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
    let mut app = make_power_movie_list_app(vec!["A", "B", "C", "D", "E", "F"]);
    let mut layout = LayoutMain::default();
    let _ = render_power_list_term(&mut app, &mut layout, 82, 8);
    assert_eq!(
        item_rows(&layout),
        vec![vec![0, 1], vec![2, 3], vec![4, 5]],
        "item i occupies column i % 2 of row i / 2"
    );
}

#[test]
fn letter_buckets_pack_independently_with_an_odd_sized_bucket() {
    let mut app = make_power_movie_list_app(vec![
        "Aardvark", "Alpha", "Apple", "Banana", "Beta", "Cherry",
    ]);
    // >= 250 switches `letter_bucket` to per-letter headers, giving an odd
    // three-item A bucket to prove ragged trailing cells.
    app.libs[0].library_total = Some(250);
    let mut layout = LayoutMain::default();
    let _ = render_power_list_term(&mut app, &mut layout, 82, 20);
    assert_eq!(
        item_rows(&layout),
        vec![vec![0, 1], vec![2], vec![3, 4], vec![5]],
        "each bucket starts a fresh item row; no row mixes two buckets"
    );
}

#[test]
fn two_column_cursor_deltas_wrap_rows_and_clamp_at_list_end() {
    // Tall enough viewport that the 18-row hero block leaves real list rows
    // below it, so `lib_page_size` reflects the list, not 0.
    let mut app = make_power_movie_list_app(vec!["M0", "M1", "M2", "M3", "M4", "M5", "M6"]);
    let mut layout = LayoutMain::default();
    let _ = render_power_list_term(&mut app, &mut layout, 82, 30);
    sync_layout_to_app(&mut app, &layout);
    let cur = cursor_of;

    // Left/right: ±1.
    app.move_lib_cursor(1);
    assert_eq!(cur(&app), 1);
    app.move_lib_cursor(-1);
    assert_eq!(cur(&app), 0);
    // Row-boundary wrap: right from the last cell of a row wraps to the
    // next row's first item, left wraps back.
    app.move_lib_cursor(1);
    app.move_lib_cursor(1);
    assert_eq!(cur(&app), 2, "right from cell 1 wraps to the next row");
    app.move_lib_cursor(-1);
    assert_eq!(cur(&app), 1, "left from cell 0 wraps to the previous row");
    // Up/down: ±cols.
    app.move_lib_cursor_rows(-1);
    assert_eq!(cur(&app), 0);
    app.move_lib_cursor_rows(1);
    assert_eq!(cur(&app), 2);
    // Down from the second-to-last row with no item directly below clamps
    // to the last item (5 -> 6).
    app.libs[0].nav_stack.last_mut().unwrap().cursor = 5;
    app.move_lib_cursor_rows(1);
    assert_eq!(
        cur(&app),
        6,
        "down past the ragged end clamps to the last item"
    );
    // End-of-list clamp on right.
    app.move_lib_cursor(1);
    assert_eq!(cur(&app), 6);
    // Paging: one viewport of item rows, clamped at both ends.
    app.libs[0].nav_stack.last_mut().unwrap().cursor = 0;
    let page = app.lib_page_size();
    app.move_lib_cursor_rows(-(page as i64));
    assert_eq!(cur(&app), 0, "page up from the top stays");
    app.move_lib_cursor_rows(page as i64);
    assert_eq!(
        cur(&app),
        6,
        "page down past the end clamps to the last item"
    );
}

// ── Top hero area (hero-on-top) ─────────────────────────────────────────

#[test]
fn left_area_is_set_for_an_empty_library_list() {
    let mut app = make_power_movie_list_app(vec![]);
    let mut layout = LayoutMain::default();
    let _ = render_power_list_term(&mut app, &mut layout, 82, 40);

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
    // width 81 stays under POWER_TWO_COLUMN_THRESHOLD (82) -> 1 col; 82
    // crosses it -> 2 col. Same items, same order; only the packing shape
    // should differ, not which item occupies which position.
    let titles: Vec<&str> = vec!["Movie A", "Movie B", "Movie C", "Movie D"];
    let mut app_1col = make_no_banner_list_app(titles.clone());
    let mut layout_1col = LayoutMain::default();
    let _ = render_power_list_term(&mut app_1col, &mut layout_1col, 81, 12);

    let mut app_2col = make_no_banner_list_app(titles);
    let mut layout_2col = LayoutMain::default();
    let _ = render_power_list_term(&mut app_2col, &mut layout_2col, 82, 12);

    let flat_1col: Vec<usize> = item_rows(&layout_1col).into_iter().flatten().collect();
    let flat_2col: Vec<usize> = item_rows(&layout_2col).into_iter().flatten().collect();
    assert_eq!(
        flat_1col, flat_2col,
        "the same items in the same order must appear in list_area regardless of column count"
    );
}

#[test]
fn hero_paints_above_list_area_in_two_column_mode() {
    let mut app = make_power_movie_list_app(vec!["Movie 0", "Movie 1 Selected"]);
    app.libs[0].nav_stack.last_mut().unwrap().cursor = 1;
    let mut layout = LayoutMain::default();
    let _ = render_power_list_term(&mut app, &mut layout, 82, 40);

    assert!(layout.hero_area.height > 0, "hero should be shown");
    assert_eq!(
        layout.hero_area.y, 0,
        "hero_area starts at the top of the content area"
    );
    assert!(
        layout.left_area.y > layout.hero_area.y + layout.hero_area.height,
        "list_area sits below hero_area, separated by at least one blank row"
    );
    assert_eq!(
        layout.left_area.y + layout.left_area.height,
        40,
        "hero_area, the separator, and list_area together fill the content area"
    );
}

#[test]
fn letter_pills_render_below_hero_and_above_list_area() {
    let mut app = make_power_movie_list_app(vec!["Movie 0", "Movie 1 Selected"]);
    app.libs[0].nav_stack.last_mut().unwrap().cursor = 1;
    // Any captured library total qualifies a top-level, non-music library
    // for the letter-range pill row (`should_show_letter_pills`).
    app.libs[0].library_total = Some(1000);
    let mut layout = LayoutMain::default();
    let _ = render_power_list_term(&mut app, &mut layout, 82, 40);

    assert!(layout.hero_area.height > 0, "hero should be shown");
    assert!(
        !layout.selector_tabs.is_empty(),
        "letter pills should render for a large top-level library"
    );
    let pills_y = layout.selector_tabs[0].0.y;
    assert_eq!(
        pills_y,
        layout.hero_area.y + layout.hero_area.height,
        "pill row sits immediately below the hero's own bottom border, no extra gap"
    );
    assert!(
        layout.left_area.y > pills_y,
        "list_area must sit below the pill row"
    );
}

#[test]
fn hero_height_is_constant_above_the_image_cap() {
    for width in [60u16, 82, 100, 150] {
        let mut app = make_power_movie_list_app(vec!["Movie 0", "Movie 1 Selected"]);
        app.libs[0].nav_stack.last_mut().unwrap().cursor = 1;
        let mut layout = LayoutMain::default();
        let _ = render_power_list_term(&mut app, &mut layout, width, 40);
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

    // Per decision 2, the image cap already kicks in well below 82 columns,
    // so the hero's height at 82/100/150 should be identical -- it doesn't
    // keep growing with terminal width.
    let heights: Vec<u16> = [82u16, 100, 150]
        .into_iter()
        .map(|width| {
            let mut app = make_power_movie_list_app(vec!["Movie 0", "Movie 1 Selected"]);
            app.libs[0].nav_stack.last_mut().unwrap().cursor = 1;
            let mut layout = LayoutMain::default();
            let _ = render_power_list_term(&mut app, &mut layout, width, 40);
            layout.hero_area.height
        })
        .collect();
    assert_eq!(
        heights[0], heights[1],
        "hero height at 82 and 100 cols should be equal (image already capped)"
    );
    assert_eq!(
        heights[1], heights[2],
        "hero height at 100 and 150 cols should be equal (image already capped)"
    );
}

#[test]
fn hero_sizes_to_content_when_a_movie_is_selected() {
    // A selected Movie's banner sizes the panel from its own content
    // (poster + meta + overview), not from the fixed placeholder reserved
    // while the slice is loading -- the placeholder is only the stand-in
    // for the no-content state.
    let mut app = make_power_movie_list_app(vec!["Movie 0", "Movie 1 Selected"]);
    app.libs[0].nav_stack.last_mut().unwrap().cursor = 1;
    let mut layout = LayoutMain::default();
    let _ = render_power_list_term(&mut app, &mut layout, 82, 40);

    let item = app.libs[0].nav_stack.last().unwrap().items[1].clone();
    let panel_width = 82 - 2 * super::SELECTED_BLOCK_SIDE_PADDING;
    let content_rows = app
        .compact_banner_layout_with_overview(&item, panel_width, false)
        .content_rows() as u16;
    let cols = crate::app::library_column_width::library_column_count(82);
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
    // A letter-pill switch clears the level's items; the hero placeholder
    // must stay reserved so the panel doesn't collapse mid-switch.
    let mut app = make_power_movie_list_app(vec!["Movie 0", "Movie 1 Selected"]);
    app.libs[0].nav_stack.last_mut().unwrap().items.clear();
    app.libs[0].nav_stack.last_mut().unwrap().loading = true;
    let mut layout = LayoutMain::default();
    let _ = render_power_list_term(&mut app, &mut layout, 82, 40);

    assert_eq!(
        layout.hero_area.height,
        super::HERO_PLACEHOLDER_ROWS,
        "hero stays reserved with empty, loading items"
    );
    assert!(
        layout.left_area.height >= 1,
        "list area must keep at least 1 row while loading"
    );
}

#[test]
fn no_hero_placeholder_for_music_libraries() {
    let mut app = make_power_movie_list_app(vec!["Album A", "Album B"]);
    app.libs[0].library.collection_type = "music".into();
    let mut layout = LayoutMain::default();
    let _ = render_power_list_term(&mut app, &mut layout, 82, 40);

    assert_eq!(
        layout.hero_area.height, 0,
        "no hero placeholder in a music library"
    );
}

#[test]
fn selected_cell_uses_carat_no_double_hash_in_two_column_mode() {
    let mut app = make_no_banner_list_app(vec!["Alpha", "Beta", "Gamma", "Delta"]);
    let mut layout = LayoutMain::default();
    let term = render_power_list_term(&mut app, &mut layout, 82, 8);
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
    let mut app = make_power_movie_list_app(title_refs);
    let mut layout = LayoutMain::default();

    // Move the cursor far down the list, past what a short viewport shows,
    // so the cursor's row scrolls out of list_area.
    app.libs[0].nav_stack.last_mut().unwrap().cursor = 39;
    let term = render_power_list_term(&mut app, &mut layout, 82, 20);
    let out = buffer_to_string(&term);

    assert!(layout.hero_area.height > 0, "hero should still be shown");
    assert!(
        !layout.left_row_map.iter().any(|r| r == &Some(39)),
        "selected row 39 should be scrolled out of the visible list_area"
    );
    assert!(
        out.contains("Movie 39"),
        "the hero should still show the cursor's item even though its row is offscreen"
    );
}

#[test]
fn two_column_mouse_click_selects_the_clicked_cell_not_the_row_first_item() {
    let mut app = make_no_banner_list_app(vec!["Click A", "Click B", "Click C"]);
    let mut layout = LayoutMain::default();
    let _ = render_power_list_term(&mut app, &mut layout, 82, 8);
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
