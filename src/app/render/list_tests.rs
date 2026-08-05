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
        search: None,
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
fn two_column_placement_is_independent_of_viewport_height() {
    let mut app = make_power_movie_list_app(vec!["A", "B", "C", "D", "E", "F"]);
    let mut layout_short = LayoutMain::default();
    let _ = render_power_list_term(&mut app, &mut layout_short, 82, 6);
    let rows_short = item_rows(&layout_short);

    let mut layout_tall = LayoutMain::default();
    let _ = render_power_list_term(&mut app, &mut layout_tall, 82, 20);
    let rows_tall = item_rows(&layout_tall);

    assert_eq!(rows_short, rows_tall);
    for (r, row) in rows_tall.iter().enumerate() {
        for (c, &item) in row.iter().enumerate() {
            assert_eq!(
                r * 2 + c,
                item,
                "item {item} must stay in column {c} of row {r}"
            );
        }
    }
}

#[test]
fn moving_the_cursor_between_adjacent_items_does_not_change_any_column() {
    let mut app = make_power_movie_list_app(vec!["A", "B", "C", "D"]);
    app.libs[0].nav_stack.last_mut().unwrap().cursor = 0;
    let mut layout = LayoutMain::default();
    let _ = render_power_list_term(&mut app, &mut layout, 82, 8);
    let before = item_rows(&layout);

    app.move_lib_cursor(1); // 0 -> 1
    let _ = render_power_list_term(&mut app, &mut layout, 82, 8);
    let after = item_rows(&layout);

    assert_eq!(
        before, after,
        "columns must not change when the cursor moves"
    );
    assert_eq!(cursor_of(&app), 1);
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
    // Tall enough viewport that the 22-row hero block (at 82 wide) leaves
    // real list rows below it, so `lib_page_size` reflects the list, not 0.
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
fn letter_grouped_vertical_movement_moves_through_the_row_map() {
    let mut app = make_power_movie_list_app(vec![
        "Aardvark", "Alpha", "Apple", "Banana", "Beta", "Cherry",
    ]);
    app.libs[0].library_total = Some(250);
    let mut layout = LayoutMain::default();
    let _ = render_power_list_term(&mut app, &mut layout, 82, 20);
    sync_layout_to_app(&mut app, &layout);
    let cur = cursor_of;
    // Item rows: [0,1],[2],[3,4],[5].
    app.libs[0].nav_stack.last_mut().unwrap().cursor = 4;
    app.move_lib_cursor_rows(1);
    assert_eq!(
        cur(&app),
        5,
        "down from a ragged column falls to the next row's last item"
    );
    app.move_lib_cursor_rows(-1);
    assert_eq!(cur(&app), 3, "up keeps the same column (row [3,4] col 0)");
    app.move_lib_cursor_rows(-1);
    assert_eq!(cur(&app), 2, "up into a single-item row keeps the column");
    app.move_lib_cursor_rows(-1);
    assert_eq!(cur(&app), 0, "up into a full row keeps the column");
    app.move_lib_cursor_rows(-1);
    assert_eq!(cur(&app), 0, "up past the first row stays");
}

#[test]
fn crossing_the_column_threshold_preserves_selection_and_scrolls_it_into_view() {
    let titles: Vec<String> = (0..10)
        .map(|i| format!("Long Movie Title Number {i}"))
        .collect();
    let title_refs: Vec<&str> = titles.iter().map(|s| s.as_str()).collect();
    let mut app = make_power_movie_list_app(title_refs);
    app.libs[0].nav_stack.last_mut().unwrap().cursor = 9;
    app.libs[0].nav_stack.last_mut().unwrap().scroll = 9;
    let mut layout = LayoutMain::default();

    // Single column (list pane 60 wide).
    let _ = render_power_list_term(&mut app, &mut layout, 60, 6);
    assert_eq!(cursor_of(&app), 9, "selection preserved while narrow");

    // Two columns (list pane 82 wide): same item stays selected and the
    // renderer's scroll write-back lands on a valid row that keeps the item
    // (and its block) on screen.
    let _ = render_power_list_term(&mut app, &mut layout, 82, 6);
    assert_eq!(
        cursor_of(&app),
        9,
        "selection preserved across the threshold"
    );
    let rows = &layout.left_item_rows;
    let row_of_9 = rows
        .iter()
        .position(|r| r.contains(&9))
        .expect("item 9's row");
    let offset = app.libs[0].nav_stack.last().unwrap().scroll;
    let visible = 6usize;
    assert!(
        row_of_9 >= offset && row_of_9 < offset + visible,
        "item 9 must be visible in the new layout (row {row_of_9}, offset {offset})"
    );

    // And back to single column: selection still intact.
    let _ = render_power_list_term(&mut app, &mut layout, 60, 6);
    assert_eq!(cursor_of(&app), 9);
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
#[test]
fn two_columns_fit_more_items_per_viewport_so_the_scrollbar_stays_hidden() {
    let mut app = make_no_banner_list_app(vec![
        "S 0", "S 1", "S 2", "S 3", "S 4", "S 5", "S 6", "S 7", "S 8", "S 9", "S 10", "S 11",
    ]);
    // Two columns: 6 item rows fit in an 8-row viewport -> no scrollbar.
    let mut layout = LayoutMain::default();
    let term = render_power_list_term(&mut app, &mut layout, 82, 8);
    let buf = term.backend().buffer();
    let last_col: String = (0..buf.area().height)
        .map(|y| buf[(buf.area().width - 1, y)].symbol().to_string())
        .collect::<Vec<_>>()
        .join("");
    assert!(
        last_col.trim().is_empty(),
        "two-column list that fits must show no scroll indicator, got {last_col:?}"
    );

    // Same 12 items in one column: 12 rows > 8 -> scrollbar appears.
    let mut layout = LayoutMain::default();
    let term = render_power_list_term(&mut app, &mut layout, 60, 8);
    let buf = term.backend().buffer();
    let last_col: String = (0..buf.area().height)
        .map(|y| buf[(buf.area().width - 1, y)].symbol().to_string())
        .collect::<Vec<_>>()
        .join("");
    assert!(
        !last_col.trim().is_empty(),
        "single-column list taller than the viewport must show a scroll indicator"
    );
}

/// Maintenance invariant: the library list is one renderer parameterized
/// by column count, not two separate views. Render the same library just
/// below and just above the column-count threshold (width 81 vs 82) and
/// assert the per-cell content of the list area is identical, modulo
/// cell-width truncation. If this test fails the two modes have diverged
/// and need to be reconciled before merging (see design.md "Maintenance
/// Rule: 1-col and 2-col stay the same view, parameterized").
#[test]
fn one_and_two_column_render_the_same_per_cell_content() {
    // Items short enough to fit in both cell widths without truncating, so
    // the test compares real content rather than truncation artifacts.
    let titles = vec!["Movie 0", "Movie 1", "Movie 2", "Movie 3"];
    let mut app1 = make_power_movie_list_app(titles.clone());
    app1.libs[0].nav_stack.last_mut().unwrap().cursor = 0;
    let mut layout1 = LayoutMain::default();
    let term1 = render_power_list_term(&mut app1, &mut layout1, 81, 30);
    let buf1 = term1.backend().buffer();
    let width1 = buf1.area().width;

    let mut app2 = make_power_movie_list_app(titles);
    app2.libs[0].nav_stack.last_mut().unwrap().cursor = 0;
    let mut layout2 = LayoutMain::default();
    let term2 = render_power_list_term(&mut app2, &mut layout2, 82, 30);
    let buf2 = term2.backend().buffer();
    let width2 = buf2.area().width;

    // The inline hero (18 rows at both widths) sits below the selected
    // item's row (cursor 0 → display row 0), and the item rows are the
    // renderer under test, packed above and below the hero inside the full
    // content area.
    assert_eq!(layout1.hero_area.height, layout2.hero_area.height);
    assert_eq!(
        layout1.hero_area.y, 1,
        "hero below the selected row (1-col)"
    );
    assert_eq!(
        layout2.hero_area.y, 1,
        "hero below the selected row (2-col)"
    );
    let list_y1 = layout1.left_area.y;
    let list_y2 = layout2.left_area.y;
    assert_eq!(
        (list_y1, list_y2),
        (0, 0),
        "the list renderer must use the full content area (y1={list_y1}, y2={list_y2})"
    );

    // Helper: collect the symbols on a row, joined together.
    let line_symbols = |buf: &ratatui::buffer::Buffer, y: u16| -> String {
        (0..buf.area().width)
            .map(|x| buf[(x, y)].symbol().to_string())
            .collect::<Vec<_>>()
            .join("")
    };

    // 1-col: every item lives in its own row, and the cell starts at the
    // list area's left edge.
    let item_row_1col = (list_y1..buf1.area().height)
        .find(|&y| line_symbols(buf1, y).contains("Movie 0"))
        .expect("1-col item row");
    let one_col_row: String = (0..width1)
        .map(|x| buf1[(x, item_row_1col)].symbol().to_string())
        .collect::<Vec<_>>()
        .join("");

    // 2-col: item 0 is the left cell of the first item row. The left cell
    // starts at the list area's left edge (same x as 1-col) and is
    // `library_cell_width(82, 2) = 40` wide, so we sample the first 40
    // columns of that row.
    let item_row_2col = (list_y2..buf2.area().height)
        .find(|&y| line_symbols(buf2, y).contains("Movie 0"))
        .expect("2-col item row");
    let two_col_left_cell: String = (0..40)
        .map(|x| buf2[(x, item_row_2col)].symbol().to_string())
        .collect::<Vec<_>>()
        .join("");

    // The 1-col cell starts with the same content as the 2-col left cell:
    // same `▌` + `## ` selected marker and same item text. Truncation
    // aside, the per-cell rendering must be identical -- the two views are
    // the same renderer, parameterized by `cols`.
    assert!(
        one_col_row.starts_with(&two_col_left_cell),
        "1-col and 2-col selected-cell content must match (modulo cell width):\n\
         1-col: {one_col_row:?}\n\
         2-col left cell: {two_col_left_cell:?}"
    );

    // The 2-col right cell exists and starts with the partner item's text
    // (item 1). The right cell is cell_width(82, 2) = 40 wide.
    let two_col_right_cell: String = (42..82)
        .map(|x| buf2[(x, item_row_2col)].symbol().to_string())
        .collect::<Vec<_>>()
        .join("");
    assert!(
        two_col_right_cell.contains("Movie 1"),
        "2-col right cell should contain the partner item's text, got: {two_col_right_cell:?}"
    );

    // Backgrounds: the selected cell (left in 2-col, full row in 1-col)
    // and the partner cell all use the ordinary list background now -- the
    // hero carries the heavy selected styling.
    for x in 0..40 {
        assert_eq!(
            buf1[(x, item_row_1col)].bg,
            buf2[(x, item_row_2col)].bg,
            "selected-cell bg at x={x} must match between 1-col and 2-col"
        );
        assert_eq!(
            buf1[(x, item_row_1col)].bg,
            ratatui::style::Color::Reset,
            "1-col selected cell must use the ordinary list bg at x={x}"
        );
    }
    for x in 42..width2 {
        assert_eq!(
            buf2[(x, item_row_2col)].bg,
            ratatui::style::Color::Reset,
            "2-col partner cell must be ordinary background at x={x}"
        );
    }
}
