use super::*;
// Characterization coverage stays beside the moved library component.
use crate::app::layout::LayoutMain;
use crate::app::render::components::list_rows::{
    selected_cell_rect, DisplayRow, InlineReplacementPlan,
};
use crate::app::tests::{make_app_stub, make_item};
use crate::app::{BrowseLevel, LibraryTab, TabSelection};
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;

#[path = "list_late_tests.rs"]
mod late_tests;

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
        app.render_list(f, Rect::new(0, 0, 60, 8), true, layout, &mut 0);
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
        ..LibraryTab::new(library)
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
        app.render_list(f, Rect::new(0, 0, width, height), true, layout, &mut 0);
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
    // TV inline browser still packs row-major at wide widths; the
    // dedicated Movies library moved to hero-on-left one-column (tested in
    // `movies_wide_tests.rs`). A tvshows library with Movie-type items has
    // no selected series/movie hero, exercising the plain two-column list.
    let mut app = make_movie_list_app(vec!["A", "B", "C", "D", "E", "F"]);
    app.libs[0].library.collection_type = "tvshows".into();
    for item in &mut app.libs[0].nav_stack.last_mut().unwrap().items {
        item.item_type = "Folder".into();
        item.is_folder = true;
    }
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
    // TV keeps its inline browser at wide widths (unlike Movies), so letter buckets
    // still pack two per row there.
    app.libs[0].library.collection_type = "collections".into();
    for item in &mut app.libs[0].nav_stack.last_mut().unwrap().items {
        item.item_type = "Folder".into();
        item.is_folder = true;
    }
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
    // inline two-column list at wide widths (Movies moved to
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

// ── Inline selected-row replacement ─────────────────────────────────────────────

#[test]
fn left_area_is_set_for_an_empty_library_list() {
    // Note: width 82 triggers wide Movies layout, which is now handled by
    // BrowserComponent (5.3d.17a). Use width 81 to test the narrow legacy path.
    let mut app = make_movie_list_app(vec![]);
    let mut layout = LayoutMain::default();
    let _ = render_list_term(&mut app, &mut layout, 81, 40);

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
    // inline path (the wide hero-on-left card sizes independently;
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
    let flow = crate::app::render::components::hero::inline_detail_flow(7, 4, 8, 0)
        .expect("a replacement detail block should fit");

    assert_eq!(flow.offset, 3);
    assert_eq!(flow.detail_screen_row, 4);
}

#[test]
fn inline_replacement_plan_owns_the_shared_contract() {
    let rows = vec![
        DisplayRow::Item(vec![0]),
        DisplayRow::Item(vec![1]),
        DisplayRow::LetterHeader("B".into()),
        DisplayRow::Item(vec![3]),
        DisplayRow::Item(vec![4]),
        DisplayRow::Item(vec![5]),
        DisplayRow::Item(vec![6]),
        DisplayRow::Item(vec![7]),
        DisplayRow::Item(vec![8]),
    ];
    let plan = InlineReplacementPlan::new(&rows, 7, 7, 4, 8, 0);

    assert_eq!(plan.detail_rows(), 4, "a complete detail block is admitted");
    assert_eq!(
        plan.offset(),
        3,
        "bottom selection grows the viewport upward"
    );
    assert_eq!(plan.detail_screen_row(), Some(4));
    assert_eq!(
        plan.hero_area(Rect::new(10, 2, 30, 8)),
        Some(Rect::new(10, 6, 30, 4))
    );
    assert!(matches!(
        plan.display_row(7),
        Some(super::super::hero::InlineDisplayRow::Replacement)
    ));
    assert!(matches!(
        plan.display_row(11),
        Some(super::super::hero::InlineDisplayRow::Source(8))
    ));
    assert_eq!(
        plan.row_targets(),
        vec![
            Some(0),
            Some(1),
            None,
            Some(3),
            Some(4),
            Some(5),
            Some(6),
            Some(7),
            None,
            None,
            None,
            Some(8),
        ],
        "the source row is swallowed and only the replacement's first row targets the parent"
    );
    assert_eq!(
        plan.item_rows(),
        vec![
            vec![0],
            vec![1],
            Vec::new(),
            vec![3],
            vec![4],
            vec![5],
            vec![6],
            vec![7],
            Vec::new(),
            Vec::new(),
            Vec::new(),
            vec![8],
        ]
    );
    assert!(!plan.should_draw_selection_markers());
}

#[test]
fn inline_replacement_plan_preserves_scroll_and_restores_ordinary_rows_when_it_cannot_fit() {
    let rows = vec![
        DisplayRow::Item(vec![0]),
        DisplayRow::Item(vec![1]),
        DisplayRow::Item(vec![2]),
        DisplayRow::Item(vec![3]),
        DisplayRow::Item(vec![4]),
        DisplayRow::Item(vec![5]),
    ];
    let persisted = InlineReplacementPlan::new(&rows, 4, 4, 2, 6, 2);
    assert_eq!(persisted.offset(), 2, "a valid stored scroll is retained");
    assert_eq!(persisted.detail_screen_row(), Some(2));

    let fallback = InlineReplacementPlan::new(&rows, 4, 4, 6, 6, 2);
    assert_eq!(fallback.detail_rows(), 0, "a complete hero cannot fit");
    assert_eq!(fallback.offset(), 2);
    assert_eq!(
        fallback.item_rows(),
        vec![vec![0], vec![1], vec![2], vec![3], vec![4], vec![5]]
    );
    assert!(fallback.should_draw_selection_markers());
}

#[test]
fn inline_replacement_plan_fallback_keeps_selected_row_visible_at_header_boundary() {
    let rows = vec![
        DisplayRow::LetterHeader("A".into()),
        DisplayRow::Item(vec![1]),
        DisplayRow::Item(vec![2]),
        DisplayRow::Item(vec![3]),
        DisplayRow::Item(vec![4]),
    ];
    let fallback = InlineReplacementPlan::new(&rows, 4, 4, 4, 4, 1);

    assert_eq!(fallback.offset(), 1);
    assert_eq!(fallback.row_targets().get(4), Some(&Some(4)));
}

#[test]
fn inline_replacement_plan_fallback_keeps_the_shared_offset_when_header_would_only_fit_ordinary_row(
) {
    let rows = vec![
        DisplayRow::LetterHeader("A".into()),
        DisplayRow::Item(vec![1]),
        DisplayRow::Item(vec![2]),
        DisplayRow::Item(vec![3]),
    ];
    let fallback = InlineReplacementPlan::new(&rows, 3, 3, 4, 4, 1);

    assert_eq!(fallback.detail_rows(), 0);
    assert_eq!(fallback.offset(), 1);
}

#[test]
fn inline_replacement_plan_admits_grouping_header_only_at_the_fit_boundary() {
    let mut rows = (0..9)
        .map(|i| DisplayRow::Item(vec![i]))
        .collect::<Vec<_>>();
    rows[2] = DisplayRow::LetterHeader("B".into());

    let fits = InlineReplacementPlan::new(&rows, 7, 7, 3, 8, 3);
    assert_eq!(
        fits.offset(),
        2,
        "the header remains visible when the hero still fits"
    );
    assert_eq!(fits.detail_screen_row(), Some(5));

    let does_not_fit = InlineReplacementPlan::new(&rows, 7, 7, 4, 8, 3);
    assert_eq!(
        does_not_fit.offset(),
        3,
        "the header is omitted rather than pushing the hero below the viewport"
    );
    assert_eq!(does_not_fit.detail_screen_row(), Some(4));
}

#[test]
fn inline_detail_replaces_the_selected_media_row() {
    let mut app = make_movie_list_app(vec!["Movie 0", "Movie 1 Selected", "Movie 2"]);
    app.libs[0].nav_stack.last_mut().unwrap().cursor = 1;
    let mut layout = LayoutMain::default();
    let term = render_list_term(&mut app, &mut layout, 81, 40);
    let output = buffer_to_string(&term);

    let selected = layout
        .selected_item_rect
        .expect("selected replacement is visible");
    assert!(
        layout.hero_area.height > 0,
        "selected detail should be rendered"
    );
    assert!(
        layout.hero_area == selected,
        "the selected replacement owns its geometry: selected={selected:?}, detail={:?}",
        layout.hero_area
    );
    let hero_start = layout.hero_area.y.saturating_sub(layout.left_area.y) as usize;
    let hero_end = hero_start + layout.hero_area.height as usize;
    assert_eq!(
        layout.left_row_map.get(hero_end),
        Some(&Some(2)),
        "the next sibling follows the full replacement block"
    );
    assert_eq!(
        layout.left_item_rows.last(),
        Some(&vec![2]),
        "physical row maps retain items after the replacement block"
    );
    assert!(
        output
            .lines()
            .nth(layout.hero_area.bottom() as usize)
            .is_some_and(|line| line.contains("Movie 2")),
        "the next sibling is rendered after the hero block"
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
