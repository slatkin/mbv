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
fn narrow_pane_renders_identically_to_single_column_output() {
    let mut app = make_no_banner_list_app(vec!["Movie 0", "Movie 1"]);
    let mut layout = LayoutMain::default();
    let out = render_power_list_to_string(&mut app, &mut layout); // 60 wide, 8 tall
    let row = |prefix: &str, title: &str| {
        format!(
            "{prefix}{title}{}",
            " ".repeat(60 - prefix.chars().count() - title.chars().count())
        )
    };
    let blank = " ".repeat(60);
    // `buffer_to_string` terminates every row (including the last) with a
    // newline, so the expected string must too.
    let expected = format!(
        "{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}\n",
        row("▌## ", "Movie 0"),
        row(" ", "Movie 1"),
        blank,
        blank,
        blank,
        blank,
        blank,
        blank,
    );
    assert_eq!(
        out, expected,
        "narrow pane must match today's single-column output (selected row gets the ▌ + ## marker)"
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

#[test]
fn hero_paints_below_selected_row_in_two_column_mode() {
    let mut app = make_power_movie_list_app(vec!["Movie 0", "Movie 1", "Movie 2", "Movie 3"]);
    // Give the selected item hero content: genre + year (the meta line) and
    // an overview, so the hero paints visible text in the test buffer
    // (poster images are disabled in the test stub).
    {
        let level = app.libs[0].nav_stack.last_mut().unwrap();
        level.items[0].genre = "Sci-Fi".into();
        level.items[0].production_year = 2020;
        level.items[0].overview = "A hero overview for the top banner.".into();
        level.cursor = 0;
    }
    let mut layout = LayoutMain::default();
    let term = render_power_list_term(&mut app, &mut layout, 82, 40);
    let buf = term.backend().buffer();
    let width = buf.area().width;

    // Inline hero: the hero sits directly below the row containing the
    // selected item (cursor 0 → display row 0), and the list wraps around
    // it — the top section ends at the selected row, the bottom section
    // continues below the hero. The block is `hero_height + 4` rows tall:
    // a `▁` top border, a bare colored-bg top padding row, the content,
    // a bare colored-bg bottom padding row, and a `▔` bottom border.
    let hero = layout.hero_area;
    assert_eq!(hero.y, 1, "hero must sit below the selected item's row");
    assert_eq!(hero.width, 82);
    assert_eq!(
        hero.height,
        super::hero_height_for_width(82) + 4,
        "hero block = content + top border + top padding + bottom padding + bottom border"
    );
    assert_eq!(
        hero.y,
        layout.left_area.y + 1,
        "hero starts one row below the selected row"
    );

    // The 4-row structure, top to bottom (focused render):
    //   row 0           : `▁` top border in SEEK_TRACK
    //   row 1           : bare colored bg (MEDIA_SELECTED_BG), no content
    //   rows 2..h-2     : content (meta + overview; image disabled in tests)
    //   row h-2         : bare colored bg (MEDIA_SELECTED_BG), no content
    //   row h-1         : `▔` bottom border in SEEK_TRACK
    let focused_bg = palette::MEDIA_SELECTED_BG;
    let top_border: String = (0..width).map(|x| buf[(x, hero.y)].symbol()).collect();
    assert_eq!(
        top_border,
        "\u{2581}".repeat(width as usize),
        "top border row must be all ▁"
    );
    for x in 0..width {
        assert_eq!(
            buf[(x, hero.y)].fg,
            palette::SEEK_TRACK,
            "top border must be painted in SEEK_TRACK at x={x}"
        );
    }
    // Top padding row: colored bg, no text or image content.
    let top_pad_row: String = (0..width).map(|x| buf[(x, hero.y + 1)].symbol()).collect();
    assert_eq!(
        top_pad_row.trim(),
        "",
        "top padding row must be bare, got: {top_pad_row:?}"
    );
    for x in 0..width {
        assert_eq!(
            buf[(x, hero.y + 1)].bg,
            focused_bg,
            "top padding row must use the focused bg at x={x}"
        );
    }
    // Bottom padding row: colored bg, no text or image content.
    let bottom_pad_y = hero.y + hero.height - 2;
    let bottom_pad_row: String = (0..width)
        .map(|x| buf[(x, bottom_pad_y)].symbol())
        .collect();
    assert_eq!(
        bottom_pad_row.trim(),
        "",
        "bottom padding row must be bare, got: {bottom_pad_row:?}"
    );
    for x in 0..width {
        assert_eq!(
            buf[(x, bottom_pad_y)].bg,
            focused_bg,
            "bottom padding row must use the focused bg at x={x}"
        );
    }
    // Bottom border row: all ▔ in SEEK_TRACK.
    let bottom_border_y = hero.y + hero.height - 1;
    let bottom_border: String = (0..width)
        .map(|x| buf[(x, bottom_border_y)].symbol())
        .collect();
    assert_eq!(
        bottom_border,
        "\u{2594}".repeat(width as usize),
        "bottom border row must be all ▔"
    );
    for x in 0..width {
        assert_eq!(
            buf[(x, bottom_border_y)].fg,
            palette::SEEK_TRACK,
            "bottom border must be painted in SEEK_TRACK at x={x}"
        );
    }

    // The hero carries the selected item's content (the poster image would
    // render here; with images off in the test stub the meta + overview
    // text is the observable content).
    let hero_text: String = (hero.y..hero.y + hero.height)
        .map(|y| (0..width).map(|x| buf[(x, y)].symbol()).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        hero_text.contains("Sci-Fi") && hero_text.contains("2020"),
        "hero must render the selected item's meta line, got:\n{hero_text}"
    );
    // The content sits offset 2 rows down (past the top border + top
    // padding), not at the block's first row.
    let content_row: String = (0..width).map(|x| buf[(x, hero.y + 2)].symbol()).collect();
    assert!(
        content_row.contains("Sci-Fi"),
        "content must start 2 rows into the hero block, got: {content_row:?}"
    );

    // The top section (above the hero) holds the selected item's row,
    // packed 2-col.
    let top_row: String = (0..width).map(|x| buf[(x, 0)].symbol()).collect();
    assert!(
        top_row.contains("Movie 0") && top_row.contains("Movie 1"),
        "top section row must hold the two-column packed cells, got:\n{top_row}"
    );
    assert!(
        !top_row.contains("Sci-Fi"),
        "hero content must not leak into the list"
    );

    // The bottom section (below the hero) continues packing the rest.
    let bottom_y = hero.y + hero.height;
    let bottom_row: String = (0..width).map(|x| buf[(x, bottom_y)].symbol()).collect();
    assert!(
        bottom_row.contains("Movie 2") && bottom_row.contains("Movie 3"),
        "bottom section must continue packing below the hero, got:\n{bottom_row}"
    );
    assert_eq!(
        item_rows(&layout),
        vec![vec![0, 1], vec![2, 3]],
        "2-col packing must be unchanged around the hero"
    );
}

#[test]
#[allow(non_snake_case)] // name specified by task: seeK
fn hero_has_top_and_bottom_borders_with_seeK_track_color() {
    let mut app = make_power_movie_list_app(vec!["Movie 0", "Movie 1"]);
    let mut layout = LayoutMain::default();
    let term = render_power_list_term(&mut app, &mut layout, 82, 40);
    let buf = term.backend().buffer();
    let hero = layout.hero_area;

    // First row of the hero block: the `▁` top border across the full
    // content width, painted in SEEK_TRACK.
    let top_row: String = (0..hero.width).map(|x| buf[(x, hero.y)].symbol()).collect();
    assert_eq!(
        top_row,
        "\u{2581}".repeat(hero.width as usize),
        "hero top border must be all ▁ across the content width, got: {top_row:?}"
    );
    for x in 0..hero.width {
        assert_eq!(
            buf[(x, hero.y)].fg,
            palette::SEEK_TRACK,
            "top border must be painted in SEEK_TRACK at x={x}"
        );
    }

    // Last row of the hero block: the `▔` bottom border in SEEK_TRACK.
    let bot_y = hero.y + hero.height - 1;
    let bot_row: String = (0..hero.width).map(|x| buf[(x, bot_y)].symbol()).collect();
    assert_eq!(
        bot_row,
        "\u{2594}".repeat(hero.width as usize),
        "hero bottom border must be all ▔ across the content width, got: {bot_row:?}"
    );
    for x in 0..hero.width {
        assert_eq!(
            buf[(x, bot_y)].fg,
            palette::SEEK_TRACK,
            "bottom border must be painted in SEEK_TRACK at x={x}"
        );
    }
}

#[test]
fn hero_uses_unfocused_bg_when_library_panel_is_unfocused() {
    let mut app = make_power_movie_list_app(vec!["Movie 0", "Movie 1"]);

    // Focused render: the hero's padding rows use MEDIA_SELECTED_BG.
    let mut layout = LayoutMain::default();
    let term = render_power_list_term(&mut app, &mut layout, 82, 40);
    let buf = term.backend().buffer();
    let hero = layout.hero_area;
    assert_eq!(
        buf[(0, hero.y + 1)].bg,
        palette::MEDIA_SELECTED_BG,
        "focused hero must use MEDIA_SELECTED_BG"
    );

    // Unfocused render: the same hero uses PLAYBACK_PANEL_BG instead.
    let mut layout = LayoutMain::default();
    let backend = TestBackend::new(82, 40);
    let mut term = Terminal::new(backend).unwrap();
    term.draw(|f| {
        app.render_power_list(f, Rect::new(0, 0, 82, 40), false, &mut layout);
    })
    .unwrap();
    let buf = term.backend().buffer();
    let hero = layout.hero_area;
    assert!(hero.height > 0, "unfocused render must still show the hero");
    for y in [hero.y + 1, hero.y + hero.height - 2] {
        for x in 0..hero.width {
            assert_eq!(
                buf[(x, y)].bg,
                palette::PLAYBACK_PANEL_BG,
                "unfocused hero padding row y={y} must use PLAYBACK_PANEL_BG at x={x}, got {:?}",
                buf[(x, y)].bg
            );
        }
    }
    // Sanity: the unfocused hero must *not* use the focused bg anywhere
    // on its padding rows.
    assert_ne!(
        palette::PLAYBACK_PANEL_BG,
        palette::MEDIA_SELECTED_BG,
        "the two bgs must differ for this test to be meaningful"
    );
}

#[test]
fn hero_top_and_bottom_padding_rows_are_colored_bg_with_no_content() {
    let mut app = make_power_movie_list_app(vec!["Movie 0", "Movie 1"]);
    // Give the selected item hero content so the content region is
    // populated (images are disabled in the test stub).
    {
        let level = app.libs[0].nav_stack.last_mut().unwrap();
        level.items[0].genre = "Sci-Fi".into();
        level.items[0].production_year = 2020;
        level.items[0].overview = "A hero overview for the top banner.".into();
        level.cursor = 0;
    }
    let mut layout = LayoutMain::default();
    let term = render_power_list_term(&mut app, &mut layout, 82, 40);
    let buf = term.backend().buffer();
    let hero = layout.hero_area;
    let bg = palette::MEDIA_SELECTED_BG;

    // The two padding rows (one inside each border) are bare colored bg:
    // no text, no image, no border glyphs -- just empty cells on the
    // focused bg.
    for y in [hero.y + 1, hero.y + hero.height - 2] {
        let row: String = (0..hero.width).map(|x| buf[(x, y)].symbol()).collect();
        assert_eq!(
            row.trim(),
            "",
            "padding row y={y} must have no content, got: {row:?}"
        );
        for x in 0..hero.width {
            assert_eq!(
                buf[(x, y)].bg,
                bg,
                "padding row y={y} must carry the colored bg at x={x}"
            );
        }
    }

    // The content region between them is populated (meta + overview text).
    let content: String = (hero.y + 2..hero.y + hero.height - 2)
        .map(|y| {
            (0..hero.width)
                .map(|x| buf[(x, y)].symbol())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        content.contains("Sci-Fi") && content.contains("2020"),
        "content between the padding rows must render the meta line, got:\n{content}"
    );
}

#[test]
fn hero_content_is_inset_by_two_cells_on_left_and_right() {
    let mut app = make_power_movie_list_app(vec!["Movie 0", "Movie 1"]);
    // Give the selected item hero content so the content region is
    // populated (images are disabled in the test stub).
    {
        let level = app.libs[0].nav_stack.last_mut().unwrap();
        level.items[0].genre = "Sci-Fi".into();
        level.items[0].production_year = 2020;
        level.items[0].overview = "A hero overview for the top banner.".into();
        level.cursor = 0;
    }
    let mut layout = LayoutMain::default();
    let term = render_power_list_term(&mut app, &mut layout, 80, 40);
    let buf = term.backend().buffer();
    let hero = layout.hero_area;
    let bg = palette::MEDIA_SELECTED_BG;

    // Content rows sit between the top/bottom padding rows (the borders
    // are the outer rows of the hero block). Their first and last 2 cols
    // must be bare colored bg -- no text, no image, no border glyphs --
    // because the content is inset by SELECTED_BLOCK_SIDE_PADDING on each
    // side.
    for y in hero.y + 2..hero.y + hero.height - 2 {
        for x in [0u16, 1, hero.width - 2, hero.width - 1] {
            let cell = &buf[(x, y)];
            assert!(
                cell.symbol().trim().is_empty(),
                "content row y={y} must have no content in the inset col x={x}, got {:?}",
                cell.symbol()
            );
            assert_eq!(
                cell.bg, bg,
                "content row y={y} inset col x={x} must carry the colored bg"
            );
        }
    }

    // The content between the insets is still rendered (meta + overview),
    // so the inset is meaningful and not a vacuous pass.
    let content: String = (hero.y + 2..hero.y + hero.height - 2)
        .map(|y| {
            (SELECTED_BLOCK_SIDE_PADDING..hero.width - SELECTED_BLOCK_SIDE_PADDING)
                .map(|x| buf[(x, y)].symbol())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        content.contains("Sci-Fi") && content.contains("2020"),
        "inset content must still render the meta line, got:\n{content}"
    );

    // The borders are NOT inset: the outer rows of the hero block still
    // span the full hero width.
    let top_row: String = (0..hero.width).map(|x| buf[(x, hero.y)].symbol()).collect();
    assert_eq!(
        top_row,
        "\u{2581}".repeat(hero.width as usize),
        "hero top border must still span the full width, got: {top_row:?}"
    );
    let bot_y = hero.y + hero.height - 1;
    let bot_row: String = (0..hero.width).map(|x| buf[(x, bot_y)].symbol()).collect();
    assert_eq!(
        bot_row,
        "\u{2594}".repeat(hero.width as usize),
        "hero bottom border must still span the full width, got: {bot_row:?}"
    );
}

#[test]
fn hero_follows_cursor_when_cursor_moves() {
    let titles: Vec<String> = (0..12).map(|i| format!("Movie {i}")).collect();
    let title_refs: Vec<&str> = titles.iter().map(|s| s.as_str()).collect();
    let mut app = make_power_movie_list_app(title_refs);
    app.libs[0].nav_stack.last_mut().unwrap().cursor = 0;
    let mut layout = LayoutMain::default();
    let term = render_power_list_term(&mut app, &mut layout, 82, 40);
    let buf = term.backend().buffer();
    assert_eq!(
        layout.hero_area.y, 1,
        "cursor on display row 0 → hero just below row 0"
    );
    // The row above the hero is the selected item's row.
    let top_row: String = (0..82).map(|x| buf[(x, 0)].symbol()).collect();
    assert!(
        top_row.contains("Movie 0") && top_row.contains("Movie 1"),
        "top section must end at the selected row, got: {top_row:?}"
    );

    // Move the cursor to item 5 (display row 2 in 2-col) and re-render:
    // the hero moves down with it.
    app.libs[0].nav_stack.last_mut().unwrap().cursor = 5;
    let mut layout = LayoutMain::default();
    let term = render_power_list_term(&mut app, &mut layout, 82, 40);
    let buf = term.backend().buffer();
    assert_eq!(
        layout.hero_area.y, 3,
        "cursor on display row 2 → hero below that row"
    );
    // The rows above the hero hold items 0..=5 packed 2-col.
    let above: String = (0..3)
        .map(|y| (0..82).map(|x| buf[(x, y)].symbol()).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        above.contains("Movie 0") && above.contains("Movie 5"),
        "top section must run through the selected row, got:\n{above}"
    );
    // The rows below the hero continue with items 6+. The hero block is
    // hero_height + 4 = 22 rows at this width (borders + padding included).
    let below: String = (3 + 22..40)
        .map(|y| (0..82).map(|x| buf[(x, y)].symbol()).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        below.contains("Movie 6") && below.contains("Movie 11"),
        "bottom section must continue below the hero, got:\n{below}"
    );
}

#[test]
fn row_map_has_none_entries_for_hero_rows() {
    let mut app = make_power_movie_list_app(vec!["Movie 0", "Movie 1", "Movie 2", "Movie 3"]);
    app.libs[0].nav_stack.last_mut().unwrap().cursor = 1;
    let mut layout = LayoutMain::default();
    let _ = render_power_list_term(&mut app, &mut layout, 82, 40);
    let hero = layout.hero_area;
    // Cursor 1 is in display row 0 (2-col); the hero occupies the rows
    // right below it. The block is hero_height + 4 rows tall (top border,
    // top padding, content, bottom padding, bottom border).
    assert_eq!(hero.y, 1);
    assert_eq!(
        hero.height,
        super::hero_height_for_width(82) + 4,
        "row map must reserve hero_height + 4 None entries for the hero block"
    );
    let row_map = &layout.left_row_map;
    // Top section: row 0 maps to item 0.
    assert_eq!(row_map[0], Some(0), "top section row must map to its item");
    // Hero rows: None (a click on them hits the hero, not an item).
    for (i, entry) in row_map
        .iter()
        .enumerate()
        .skip(1)
        .take(hero.height as usize)
    {
        assert_eq!(*entry, None, "hero display row {i} must map to None");
    }
    // Bottom section: the row below the hero maps to item 2.
    let bottom_idx = 1 + hero.height as usize;
    assert_eq!(
        row_map[bottom_idx],
        Some(2),
        "bottom section row must map to the item below the cursor"
    );
    assert_eq!(
        row_map.len(),
        bottom_idx + 1,
        "row map covers top section + hero + bottom section"
    );
}

#[test]
fn auto_scroll_keeps_cursor_and_hero_visible() {
    // 60 movies → 30 display rows (2-col) + 22 hero rows (hero_height 18
    // + 4 block rows at 82 wide). A 26-row viewport can't show everything;
    // the auto-scroll must bring the cursor row and the hero below it into
    // view even from a stale scroll offset of 0. The min-visible area is
    // the cursor's row + 1 + hero_height + 4 (cursor row, then the whole
    // block below it).
    let titles: Vec<String> = (0..60).map(|i| format!("Movie {i}")).collect();
    let title_refs: Vec<&str> = titles.iter().map(|s| s.as_str()).collect();
    let mut app = make_power_movie_list_app(title_refs);
    app.libs[0].nav_stack.last_mut().unwrap().cursor = 59;
    app.libs[0].nav_stack.last_mut().unwrap().scroll = 0;
    let mut layout = LayoutMain::default();
    let _ = render_power_list_term(&mut app, &mut layout, 82, 26);
    let area = layout.left_area;

    // The cursor's row and the hero below it are both on screen.
    let cursor_y = layout.cursor_screen_y.expect("cursor row");
    assert!(
        cursor_y >= area.y && cursor_y < area.y + area.height,
        "cursor row must be visible (y={cursor_y})"
    );
    let hero = layout.hero_area;
    assert_eq!(
        hero.y,
        cursor_y + 1,
        "hero must sit directly below the cursor's row"
    );
    assert!(
        hero.y + hero.height <= area.y + area.height,
        "hero must fit in the viewport (hero {hero:?}, area {area:?})"
    );
}

#[test]
fn hero_height_scales_with_content_width() {
    // The image cap (design decision 3a) bounds the hero at ≤ 18 content
    // rows at these widths; the hero *block* adds the 4 border/padding
    // rows, and the list above and below keeps rows at any width.
    for width in [60u16, 82, 100, 150] {
        let mut app = make_power_movie_list_app(vec!["Movie 0", "Movie 1", "Movie 2", "Movie 3"]);
        let mut layout = LayoutMain::default();
        let term = render_power_list_term(&mut app, &mut layout, width, 40);
        let buf = term.backend().buffer();

        let hero = layout.hero_area;
        assert_eq!(hero.width, width);
        assert_eq!(
            hero.height,
            super::hero_height_for_width(width) + 4,
            "hero block = content + top border + top padding + bottom padding + bottom border at width {width}"
        );
        assert!(
            hero.height <= 22,
            "hero must be bounded by the 12-row image cap + meta + 4 block rows at width {width}, got {}",
            hero.height
        );
        // Cursor 0 → the hero sits one row below the selected item's row.
        assert_eq!(
            hero.y, 1,
            "hero must sit below the selected row at width {width}"
        );
        assert!(
            hero.y + hero.height <= buf.area().height,
            "hero must fit in the content area at width {width}"
        );
        // The top section row above the hero holds the selected item.
        let top_row: String = (0..width).map(|x| buf[(x, 0)].symbol()).collect();
        assert!(
            top_row.contains("Movie 0"),
            "top section must show the selected item at width {width}, got: {top_row:?}"
        );
        // The bottom section below the hero keeps real item rows.
        let bottom_row: String = (0..width)
            .map(|x| buf[(x, hero.y + hero.height)].symbol())
            .collect();
        assert!(
            !bottom_row.trim().is_empty(),
            "bottom section must keep rows below the hero at width {width}, got: {bottom_row:?}"
        );
    }
}

#[test]
fn selected_cell_uses_carat_and_double_hash_in_two_column_mode() {
    let mut app = make_power_movie_list_app(vec!["Movie 0", "Movie 1", "Movie 2", "Movie 3"]);
    app.libs[0].nav_stack.last_mut().unwrap().cursor = 0;
    let mut layout = LayoutMain::default();
    let term = render_power_list_term(&mut app, &mut layout, 82, 40);
    let buf = term.backend().buffer();
    let width = buf.area().width;

    // First list row: the selected left cell (cursor 0).
    let list_y = layout.left_area.y;
    let first_row: String = (0..width).map(|x| buf[(x, list_y)].symbol()).collect();
    assert!(
        first_row.starts_with("▌## Movie 0"),
        "selected left cell must start with the ▌ mark and ## prefix, got: {first_row:?}"
    );
    // The ▌ mark sits at the cell's left edge (the list area's first
    // column).
    assert_eq!(
        buf[(0, list_y)].symbol(),
        "▌",
        "left edge of the selected cell must carry the ▌ mark"
    );
    // The partner cell keeps the ordinary 1-col leading separator.
    let right_cell: String = first_row.chars().skip(42).take(40).collect();
    assert!(
        right_cell.starts_with(" Movie 1"),
        "partner cell must keep the plain leading space, got: {right_cell:?}"
    );
}
