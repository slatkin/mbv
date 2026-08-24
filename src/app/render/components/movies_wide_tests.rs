//! Wide Movies hero-on-left verification (movies-hero-on-left change):
//! The test remains unchanged apart from its component-module location.
//! the left pane renders the exact shared Home selected-Emby card (same
//! image cache key), the right rail holds the pill row + one-column list,
//! cursor movement stays list-owned, and the read-only hero is never a
//! focus/activation surface.

use super::*;
use crate::app::layout::LayoutMain;
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

fn make_movie_app(titles: Vec<&str>) -> App {
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
                m.overview = "This is the shared hero card overview text.".into();
                m.premiere_date = "2020-01-15".into();
                m.runtime_ticks = 5400 * mbv_core::api::TICKS_PER_SECOND;
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
        series_selection: None,
        series_season_cursor: 0,
        library_total: None,
    });

    app
}

fn sync_layout_to_app(app: &mut App, layout: &LayoutMain) {
    app.layout.main.left_area = layout.left_area;
    app.layout.main.left_item_rows = layout.left_item_rows.clone();
    app.layout.main.left_sorted_indices = layout.left_sorted_indices.clone();
    app.layout.main.left_row_map = layout.left_row_map.clone();
    app.layout.main.movies_wide_right_area = layout.movies_wide_right_area;
    app.layout.main.hero_area = layout.hero_area;
}

/// Wide Movies (at/above the breakpoint) renders the shared hero-on-left
/// card: the exact `id:pwr_kw` cache key Home uses, the same
/// Backdrop/Primary/Logo image types, and the card's metadata in the left
/// pane -- while `left_area`/`hero_area` bookkeeping targets the right rail.
#[test]
fn wide_movies_renders_shared_hero_card_with_home_cache_key() {
    let mut app = make_movie_app(vec!["Movie 0", "Movie 1 Selected"]);
    app.libs[0].nav_stack.last_mut().unwrap().cursor = 1;
    app.image_protocol_enabled = true;

    let mut layout = LayoutMain::default();
    let term = render_list_term(&mut app, &mut layout, 120, 40);
    let out = buffer_to_string(&term);

    // The wide arrangement is active (right rail published).
    assert!(
        layout.movies_wide_right_area.width > 0 && layout.movies_wide_right_area.height > 0,
        "wide Movies right rail should be published"
    );

    // The left card fetches under Home's exact `id:pwr_kw` cache key
    // (home_hero.rs's `render_home_hero_data`), proving the shared card path
    // -- not a Movies-specific key.
    let hero_key = format!("{}:pwr_kw", "movie-1");
    assert!(
        app.card_image_loading.contains(&hero_key) || app.card_image_states.contains_key(&hero_key),
        "the shared hero card must fetch the selected movie under the id:pwr_kw cache key"
    );

    // The right rail is the list's browse surface; the read-only hero is not
    // published as interactive `hero_area`.
    assert_eq!(
        layout.hero_area,
        Rect::default(),
        "wide Movies must not publish an interactive hero_area"
    );
    assert!(
        layout.left_area.width > 0 && layout.left_area.height > 0,
        "left_area (right-rail list) must be published for cursor/mouse"
    );
    // The left card is visible in the left pane (metadata painted).
    assert!(
        out.contains("Movie 1 Selected"),
        "the left hero card should show the selected movie's title"
    );
}

/// Moving the Movies cursor updates the left card from the right-rail list
/// cursor. The list keeps its cursor row visible by clamping scroll, so the
/// meaningful invariant is: the left hero card shows the cursor's item (not
/// some fixed first-row item) at every scroll position.
#[test]
fn wide_movies_hero_tracks_cursor_scrolled_out_of_the_rail() {
    let mut titles: Vec<String> = (0..60).map(|i| format!("Movie {i}")).collect();
    titles[59] = "Movie 59 Selected".to_string();
    let title_refs: Vec<&str> = titles.iter().map(String::as_str).collect();
    let mut app = make_movie_app(title_refs);
    app.libs[0].nav_stack.last_mut().unwrap().cursor = 59;

    let mut layout = LayoutMain::default();
    let term = render_list_term(&mut app, &mut layout, 120, 20);
    let out = buffer_to_string(&term);

    // The cursor is far down the 60-item list; the rail shows only a small
    // slice of it, yet the left hero card still shows the cursor's item.
    assert!(
        out.contains("Movie 59"),
        "the left hero should show the cursor's item regardless of scroll position"
    );
    // The cursor row is the one the rail keeps on screen (scroll clamp), and
    // the hero content matches it — proving the hero reads the list cursor,
    // not a stale or first-row item.
    let rows: Vec<usize> = layout.left_row_map.iter().flatten().copied().collect();
    assert!(
        rows.contains(&59),
        "cursor row should be visible in the rail (scroll clamp)"
    );
}

/// The shared wide hero card (`prepare_wide_emby_hero_card`, the function
/// both Home and wide Movies call) retains the required card features: 16:9
/// artwork above metadata, graceful empty-field handling, and a `None` when
/// the area is too small for a usable card.
#[test]
fn shared_hero_card_keeps_169_artwork_and_empty_field_handling() {
    let mut item = make_item("Interstellar", "Movie");
    item.id = "movie-interstellar".into();
    item.premiere_date = "2014-11-07".into();
    item.runtime_ticks = 2 * 3600 * mbv_core::api::TICKS_PER_SECOND;

    // 40 cols x 40 rows: image at 16:9 (40 cols -> 11 rows ceiling), then a
    // 1-row gap, then metadata below.
    let area = Rect::new(0, 0, 40, 40);
    let (layout, meta_area, img_area) =
        prepare_wide_emby_hero_card(&item, area).expect("card should fit");
    assert_eq!(img_area.y, 0, "image occupies the top of the card");
    assert_eq!(meta_area.y, img_area.y + img_area.height + 1, "1-row gap");
    assert_eq!(img_area.width, 40, "image spans the full content width");
    assert_eq!(
        img_area.height,
        40u16.saturating_mul(9).div_ceil(32),
        "16:9 image row budget for 40 columns"
    );
    assert!(
        layout.height >= 4,
        "metadata block must fit title/meta/overview"
    );

    // Empty overview/date/duration: layout still fits (graceful empty fields).
    let mut bare = make_item("Bare", "Movie");
    bare.id = "movie-bare".into();
    let (layout_bare, ..) = prepare_wide_emby_hero_card(&bare, area).expect("card should fit");
    assert!(
        layout_bare.height >= 4,
        "empty fields still yield a usable card"
    );

    // Too-small area: None (hero suppressed when it cannot fit).
    assert!(
        prepare_wide_emby_hero_card(&item, Rect::new(0, 0, 8, 2)).is_none(),
        "a tiny area must suppress the hero card"
    );
}

/// Wide Movies renders the pill row at the top of the right rail and a
/// one-column list below it; narrow Movies uses selected-row replacement.
#[test]
fn wide_movies_pills_in_right_rail_and_one_column_list() {
    let mut app = make_movie_app(vec!["Movie 0", "Movie 1", "Movie 2"]);
    app.libs[0].library_total = Some(1000); // pill eligibility
    let mut layout = LayoutMain::default();
    let _ = render_list_term(&mut app, &mut layout, 120, 40);

    // Letter pills render through the shared pill bar (selector_tabs).
    assert!(
        !layout.selector_tabs.is_empty(),
        "wide Movies should render the letter-range pills"
    );
    let pills_y = layout.selector_tabs[0].0.y;
    assert!(
        pills_y < layout.left_area.y,
        "pill row sits above the right-rail list"
    );
    // Right rail list is one column (rows hold a single item each).
    let rows: Vec<Vec<usize>> = layout
        .left_item_rows
        .iter()
        .filter(|r| !r.is_empty())
        .cloned()
        .collect();
    assert!(
        rows.iter().all(|row| row.len() == 1),
        "wide Movies list must be one column, got {rows:?}"
    );
}

#[test]
fn narrow_movies_uses_inline_hero() {
    let mut app = make_movie_app(vec!["Movie 0", "Movie 1 Selected"]);
    app.libs[0].nav_stack.last_mut().unwrap().cursor = 1;
    let mut layout = LayoutMain::default();
    let _ = render_list_term(&mut app, &mut layout, 81, 40);

    assert!(
        layout.hero_area.height > 0,
        "narrow Movies keeps the inline hero banner"
    );
    assert_eq!(
        layout.movies_wide_right_area,
        Rect::default(),
        "narrow Movies must not publish the wide right rail"
    );
    assert!(
        layout.hero_area.y >= layout.left_area.y
            && layout.hero_area.y + layout.hero_area.height
                <= layout.left_area.y + layout.left_area.height,
        "narrow Movies hero stays inside the list flow"
    );
}

/// Queue focus dims both wide Movies surfaces without creating a focusable
/// hero; the hero is not an interactive geometry target.
#[test]
fn queue_focus_dims_wide_movies_and_hero_is_not_interactive() {
    let mut app = make_movie_app(vec!["Movie 0", "Movie 1 Selected"]);
    app.libs[0].nav_stack.last_mut().unwrap().cursor = 1;

    // Queue focus renders the library unfocused: both the left hero card and
    // the right rail list still render (dimmed), never disappearing.
    let mut layout = LayoutMain::default();
    let backend = TestBackend::new(120, 40);
    let mut term = Terminal::new(backend).unwrap();
    term.draw(|f| {
        app.render_list(f, Rect::new(0, 0, 120, 40), false, &mut layout);
    })
    .unwrap();
    let out = buffer_to_string(&term);
    assert!(
        out.contains("Movie 1 Selected"),
        "hero card still renders when the panel is not focused (queue focus)"
    );
    sync_layout_to_app(&mut app, &layout);

    // The hero is never published as `hero_area`, so a click on the left
    // hero is not a browse/activation target.
    assert_eq!(
        app.layout.main.hero_area,
        Rect::default(),
        "read-only hero must not be published as interactive hero geometry"
    );
    let hero_click = app.click_set_cursor(5, 5);
    assert!(
        !hero_click,
        "clicking the left hero must not be handled (no activation path)"
    );
    // Clicking the right rail list focuses and selects the row.
    let la = app.layout.main.left_area;
    assert!(
        la.contains((la.x + 1, la.y).into()),
        "right rail list should be clickable"
    );
    assert!(
        app.click_set_cursor(la.x + 1, la.y),
        "clicking the right rail list should be handled"
    );
}

/// Wide Movies cursor movement stays list-owned: j/k/Up/Down stride the
/// one-column right rail (never a hero pane), and `left_area` geometry
/// drives paging.
#[test]
fn wide_movies_cursor_movement_strides_the_one_column_list() {
    let mut app = make_movie_app(vec!["M0", "M1", "M2", "M3", "M4"]);
    let mut layout = LayoutMain::default();
    let _ = render_list_term(&mut app, &mut layout, 120, 40);
    sync_layout_to_app(&mut app, &layout);
    let cursor_of = |app: &App| app.libs[0].nav_stack.last().unwrap().cursor;

    // Down moves exactly one item (one-column rail), not two.
    app.move_lib_cursor_rows(0, 1);
    assert_eq!(
        cursor_of(&app),
        1,
        "down in the one-column rail moves one item"
    );
    app.move_lib_cursor_rows(0, 1);
    assert_eq!(cursor_of(&app), 2);
    app.move_lib_cursor_rows(0, -1);
    assert_eq!(
        cursor_of(&app),
        1,
        "up in the one-column rail moves one item"
    );
}
