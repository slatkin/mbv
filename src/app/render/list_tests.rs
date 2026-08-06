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
    // Tall enough viewport that the 23-row hero block (at 82 wide, 2-col)
    // leaves real list rows below it, so `lib_page_size` reflects the list,
    // not 0.
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
