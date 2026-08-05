use super::*;
use crate::app::tests::{make_app_stub, make_item};
use crate::app::{BrowseLevel, LibraryTab, PanelFocus, POWER_LEFT_WIDTH_DEFAULT};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::backend::TestBackend;
use ratatui::Terminal;

fn make_power_movie_app() -> App {
    let mut app = make_app_stub();
    app.panel_focus = PanelFocus::Library;
    app.library_tab = 1;

    let mut library = make_item("Movies", "CollectionFolder");
    library.id = "lib-movies".into();
    library.is_folder = true;
    library.collection_type = "movies".into();

    let mut first = make_item("First Movie", "Movie");
    first.id = "movie-1".into();
    first.overview =
        "A compact banner overview that should not require the expanded detail mode.".into();
    first.director = "Jane Director".into();

    let mut second = make_item("Second Movie", "Movie");
    second.id = "movie-2".into();

    app.libs.push(LibraryTab {
        library,
        nav_stack: vec![BrowseLevel {
            parent_id: "lib-movies".into(),
            title: "Movies".into(),
            items: vec![first, second],
            total_count: 2,
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

fn push_power_library(app: &mut App, id: &str, name: &str) {
    let mut library = make_item(name, "CollectionFolder");
    library.id = id.into();
    library.is_folder = true;
    library.collection_type = "movies".into();

    let mut item = make_item(&format!("{name} Item"), "Movie");
    item.id = format!("{id}-item");

    app.libs.push(LibraryTab {
        library,
        nav_stack: vec![BrowseLevel {
            parent_id: id.into(),
            title: name.into(),
            items: vec![item],
            total_count: 1,
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
}

fn make_power_tab_cycle_app() -> App {
    let mut app = make_app_stub();
    app.panel_focus = PanelFocus::Library;
    app.library_tab = 1;
    push_power_library(&mut app, "lib-movies", "Movies");
    push_power_library(&mut app, "lib-shows", "Shows");
    app
}

fn shift(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::SHIFT)
}

#[test]
fn enter_on_movie_plays_without_opening_detail() {
    let mut app = make_power_movie_app();

    let handled = app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    assert!(!handled);
    assert_eq!(app.player_tab.items.len(), 1);
    assert_eq!(app.player_tab.items[0].id, "movie-1");
}

// Alt+M's full-screen detail view (and its dedicated key binding) was
// removed in #204: the compact banner is the only movie-detail surface
// now, so Alt+M is just an ordinary (unbound) key press that falls
// through without effect.
#[test]
fn alt_m_is_a_no_op_in_power_movie_view() {
    let mut app = make_power_movie_app();

    let handled = app.handle_key(KeyEvent::new(KeyCode::Char('m'), KeyModifiers::ALT));

    assert!(!handled);
}

// #263: the compact movie-detail banner's old fixed-height-with-scroll
// behavior (and the Up/Down/PageUp/PageDown banner-scroll key handling
// that came with it) was removed -- these keys must always move the
// list cursor, regardless of the selected movie's banner content, never
// get intercepted to scroll the banner instead.
#[test]
fn down_always_moves_the_list_cursor_never_scrolls_the_banner() {
    let mut app = make_power_movie_app();

    let handled = app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));

    assert!(!handled);
    assert_eq!(
        app.libs[0].nav_stack[0].cursor, 1,
        "Down should move the list cursor to the second movie"
    );
}

// Vim-style navigation for the two-column library list: j/k mirror Up/Down
// in any column count; h/l move across cells in 2-col mode only and stay
// unbound (so the user can type 'h' and 'l' as query characters) in 1-col
// mode. The sidebar-toggle key moved from 'h' to 'x' to free 'h' for this.
#[test]
fn hjkl_navigates_two_column_library_list_via_handle_key() {
    let mut app = make_power_movie_app();
    // Four movies => two 2-col rows: [0,1] / [2,3].
    let mut m2 = make_item("Third Movie", "Movie");
    m2.id = "movie-3".into();
    let mut m3 = make_item("Fourth Movie", "Movie");
    m3.id = "movie-4".into();
    app.libs[0].nav_stack[0].items.push(m2);
    app.libs[0].nav_stack[0].items.push(m3);
    // Force the layout to read as 2-col so the h/l guard passes.
    app.layout.main.left_area = ratatui::layout::Rect::new(0, 0, 84, 20);

    // l moves right one cell: 0 -> 1.
    let _ = app.handle_key(KeyEvent::new(KeyCode::Char('l'), KeyModifiers::NONE));
    assert_eq!(app.libs[0].nav_stack[0].cursor, 1, "'l' should move right");

    // h moves left one cell: 1 -> 0.
    let _ = app.handle_key(KeyEvent::new(KeyCode::Char('h'), KeyModifiers::NONE));
    assert_eq!(app.libs[0].nav_stack[0].cursor, 0, "'h' should move left");

    // j moves down one row in 2-col: 0 -> 2.
    let _ = app.handle_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE));
    assert_eq!(
        app.libs[0].nav_stack[0].cursor, 2,
        "'j' should move down one row"
    );

    // k moves up one row in 2-col: 2 -> 0.
    let _ = app.handle_key(KeyEvent::new(KeyCode::Char('k'), KeyModifiers::NONE));
    assert_eq!(
        app.libs[0].nav_stack[0].cursor, 0,
        "'k' should move up one row"
    );
}

#[test]
fn hl_are_unbound_in_one_column_library_list() {
    let mut app = make_power_movie_app();
    // Force the layout to read as 1-col so the h/l guard fails.
    app.layout.main.left_area = ratatui::layout::Rect::new(0, 0, 60, 20);

    let _ = app.handle_key(KeyEvent::new(KeyCode::Char('h'), KeyModifiers::NONE));
    let _ = app.handle_key(KeyEvent::new(KeyCode::Char('l'), KeyModifiers::NONE));
    // Cursor must not have moved: h and l fell through as unbound.
    assert_eq!(
        app.libs[0].nav_stack[0].cursor, 0,
        "h/l must stay unbound in 1-col mode (so they remain free as input characters)"
    );
}

#[test]
fn lib_search_ignores_navigation_keys_and_only_accepts_query_editing() {
    // Regression: per design, when the lib search box is open and the user
    // is typing into the query, NO navigation key should fire -- not
    // Up/Down/PageUp/PageDown, not Enter, not h/j/k/l. Only Esc, Backspace,
    // and printable characters are recognised. This prevents "h" typed into
    // a query for "Harry Potter" from being silently consumed as nav.
    let mut app = make_power_movie_app();
    app.libs[0].search = Some(crate::app::LibSearch {
        query: String::new(),
        items: app.libs[0].nav_stack[0].items.clone(),
        results: (0..app.libs[0].nav_stack[0].items.len()).collect(),
        cursor: 0,
        scroll: 0,
        loading: false,
    });

    // Type a multi-letter query that uses every previously-bound letter.
    for c in "hjklll".chars() {
        let _ = app.handle_key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE));
    }
    assert_eq!(
        app.libs[0].search.as_ref().unwrap().query,
        "hjklll",
        "h/j/k/l must be typed into the query, not consumed as nav"
    );
    // Cursor stays at 0 -- nothing has moved it.
    assert_eq!(app.libs[0].search.as_ref().unwrap().cursor, 0);
}

#[test]
fn shift_right_resizes_without_switching_focus() {
    let mut app = make_power_movie_app();
    app.panel_focus = PanelFocus::Queue;
    app.terminal_width = 100;

    let handled = app.handle_key(shift(KeyCode::Right));

    assert!(!handled);
    assert!(matches!(app.panel_focus, PanelFocus::Queue));
    assert_eq!(app.queue_column_width, 45);
    assert!(app.status.contains("45"), "status was {:?}", app.status);
    assert_eq!(App::load_prefs()["queue_column_width"].as_u64(), Some(45));
}

#[test]
fn left_does_not_focus_hidden_queue_when_power_left_column_is_collapsed() {
    let mut app = make_power_movie_app();
    app.queue_column_collapsed = true;
    app.panel_focus = PanelFocus::Library;

    let handled = app.handle_key(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE));

    assert!(!handled);
    assert_eq!(app.panel_focus, PanelFocus::Library);
}

// `shift_resize_is_ignored_outside_power_view` (deleted test, #361): asserted
// Shift+Right resize does nothing on a plain `make_app_stub()` (i.e.
// on the Home tab). That state no longer exists -- mbv has one view
// now, and the queue column
// it resizes is present on every tab including Home, so Shift+Right
// correctly resizes it here too. The surviving resize guards (column
// collapsed, overlay open) are still covered by
// `shift_resize_is_ignored_while_power_left_column_is_collapsed` and
// `help_overlay_blocks_power_resize_shortcuts` below/above.

#[test]
fn shift_resize_is_ignored_while_power_left_column_is_collapsed() {
    let mut app = make_power_movie_app();
    app.queue_column_collapsed = true;
    app.terminal_width = 100;

    let handled = app.handle_key(shift(KeyCode::Right));

    assert!(!handled);
    assert_eq!(app.queue_column_width, POWER_LEFT_WIDTH_DEFAULT);
    assert!(app.status.is_empty(), "status was {:?}", app.status);
}

#[test]
fn help_overlay_blocks_power_resize_shortcuts() {
    let mut app = make_power_movie_app();
    app.show_help = true;
    app.terminal_width = 100;

    let handled = app.handle_key(shift(KeyCode::Right));

    assert!(!handled);
    assert_eq!(app.queue_column_width, POWER_LEFT_WIDTH_DEFAULT);
}

#[test]
fn shift_resize_clamps_at_min_and_max_without_resaving_on_noop() {
    let mut app = make_power_movie_app();
    app.terminal_width = 80;

    app.handle_key(shift(KeyCode::Left));
    assert_eq!(app.queue_column_width, POWER_LEFT_WIDTH_DEFAULT);
    assert!(
        app.status.contains("minimum"),
        "expected minimum toast, got {:?}",
        app.status
    );

    app.handle_key(shift(KeyCode::Right));
    assert_eq!(app.queue_column_width, 45);
    app.handle_key(shift(KeyCode::Right));
    assert_eq!(app.queue_column_width, 48);
    let saved = App::load_prefs()["queue_column_width"].as_u64();
    assert_eq!(saved, Some(48));

    app.handle_key(shift(KeyCode::Right));
    assert_eq!(app.queue_column_width, 48);
    assert!(
        app.status.contains("maximum"),
        "expected maximum toast, got {:?}",
        app.status
    );
    assert_eq!(App::load_prefs()["queue_column_width"].as_u64(), saved);
}

#[test]
fn render_normalizes_oversized_saved_power_width_and_persists_it() {
    let mut app = make_power_movie_app();
    app.queue_column_width = 80;

    let backend = TestBackend::new(70, 24);
    let mut term = Terminal::new(backend).unwrap();
    term.draw(|f| app.render(f)).unwrap();

    assert_eq!(app.queue_column_width, 42);
    assert_eq!(App::load_prefs()["queue_column_width"].as_u64(), Some(42));
}

#[test]
fn search_result_click_uses_left_row_map_around_the_inline_hero() {
    let mut app = make_power_movie_app();

    // Replace the plain nav-stack browsing state with an active search
    // over three leaf movies, cursor on the first result. The selected
    // leaf movie gets the hero, inserted inline just below the selected
    // row; the click target must be resolved against the row map (hero
    // rows are None), not the full content area.
    let mut movie1 = make_item("First Movie", "Movie");
    movie1.id = "movie-1".into();
    let mut movie2 = make_item("Second Movie", "Movie");
    movie2.id = "movie-2".into();
    let mut movie3 = make_item("Third Movie", "Movie");
    movie3.id = "movie-3".into();
    let items = vec![movie1, movie2, movie3];
    app.libs[0].search = Some(LibSearch {
        query: "movie".into(),
        items,
        results: vec![0, 1, 2],
        cursor: 0,
        scroll: 0,
        loading: false,
    });

    // Render for real so layout.main.left_row_map / left_area reflect the
    // inline hero.
    let backend = TestBackend::new(100, 40);
    let mut term = Terminal::new(backend).unwrap();
    term.draw(|f| app.render(f)).unwrap();

    let hero = app.layout.main.hero_area;
    assert!(
        hero.height > 0,
        "the search selection must show the hero, got height {}",
        hero.height
    );

    let la = app.layout.main.left_area;
    assert!(
        la.y > 0,
        "the list sits below the search input (y {})",
        la.y
    );

    // The hero is inline: it sits *inside* the list area, one row below
    // the selected item's row (cursor 0) -- not at the top of the area.
    assert!(
        hero.y > la.y,
        "the hero must be inline below the selected row (hero y {}, area y {})",
        hero.y,
        la.y
    );
    assert_eq!(
        hero.y,
        la.y + 1,
        "the hero starts one row below the selected row"
    );

    // The row map reflects the inserted hero: the selected item's row maps
    // to item 0, the hero rows are None (a click there hits the hero, not
    // an item), and the remaining results map to items below the hero.
    let row_map = app.layout.main.left_row_map.clone();
    assert_eq!(row_map[0], Some(0));
    for (i, entry) in row_map
        .iter()
        .enumerate()
        .skip(1)
        .take(hero.height as usize)
    {
        assert_eq!(*entry, None, "hero display row {i} must map to None");
    }
    assert_eq!(
        row_map[1 + hero.height as usize],
        Some(1),
        "the first result below the hero must map to search result index 1"
    );

    // Click the row mapping to search result index 1 (the "Second Movie"
    // row) -- the click coordinates are derived from the row map, which
    // accounts for the hero rows.
    let click_row_idx = row_map
        .iter()
        .position(|r| *r == Some(1))
        .expect("expected a row mapping to search result index 1");
    let row = la.y + click_row_idx as u16;
    let col = la.x + 1;

    let handled = app.click_set_cursor(col, row);

    assert!(handled);
    assert_eq!(
        app.libs[0].search.as_ref().unwrap().cursor,
        1,
        "click should select the row-map item index, not a naive offset + click_y index"
    );
}

// ── Phase 3 (#132) view-routing boundary tests ─────────────────────
// These exercise the shared `handle_global_view_key` /
// `handle_enqueue_selected_key` front doors and the queue view's
// `handle_lib_key(lib_idx, key)` routing (no more `is_lib_key` mirror).

#[test]
fn period_key_opens_context_menu_from_all_three_view_handlers() {
    // Home (combined), library, and queue views all route '.' through
    // the shared `handle_global_view_key`.
    let mut home = make_app_stub();
    home.home
        .continue_items
        .push(crate::app::tests::make_item("Continuing", "Movie"));
    assert!(home.context_menu.is_none());
    home.handle_key(KeyEvent::new(KeyCode::Char('.'), KeyModifiers::NONE));
    assert!(home.context_menu.is_some(), "combined (home) view");

    let mut lib = make_app_stub();
    lib.library_tab = 1;
    let mut library = crate::app::tests::make_item("Movies", "CollectionFolder");
    library.id = "lib-movies".into();
    library.is_folder = true;
    lib.libs.push(crate::app::LibraryTab {
        library,
        nav_stack: vec![crate::app::BrowseLevel {
            parent_id: "lib-movies".into(),
            title: "Movies".into(),
            items: vec![crate::app::tests::make_item("A Movie", "Movie")],
            total_count: 1,
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
    lib.handle_key(KeyEvent::new(KeyCode::Char('.'), KeyModifiers::NONE));
    assert!(lib.context_menu.is_some(), "library view");

    let mut queue = make_power_movie_app();
    queue.panel_focus = PanelFocus::Queue;
    queue
        .player_tab
        .items
        .push(crate::app::tests::make_item("Queued", "Movie"));
    queue.handle_key(KeyEvent::new(KeyCode::Char('.'), KeyModifiers::NONE));
    assert!(queue.context_menu.is_some(), "queue view");
}

#[test]
fn ctrl_a_enqueues_selected_from_library_view() {
    // `Ctrl+a` enqueue is shared by combined and library views via
    // `handle_enqueue_selected_key` (the queue view has no "enqueue
    // selected" concept and does not wire this in). See issue #209.
    let mut app = make_app_stub();
    app.library_tab = 1;
    let mut library = crate::app::tests::make_item("Movies", "CollectionFolder");
    library.id = "lib-movies".into();
    library.is_folder = true;
    let mut movie = crate::app::tests::make_item("A Movie", "Movie");
    movie.id = "movie-1".into();
    app.libs.push(crate::app::LibraryTab {
        library,
        nav_stack: vec![crate::app::BrowseLevel {
            parent_id: "lib-movies".into(),
            title: "Movies".into(),
            items: vec![movie],
            total_count: 1,
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

    app.handle_key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL));

    assert_eq!(
        app.player_tab.items.len(),
        1,
        "Ctrl+a enqueues from library view"
    );
    assert_eq!(app.player_tab.items[0].id, "movie-1");
}

#[test]
fn ctrl_z_while_power_library_panel_focused_does_not_leak_to_queue_undo() {
    // Preserved quirk from the pre-phase-3 `is_lib_key` mirror: while a
    // library sub-panel has focus, an unmapped
    // Ctrl/Alt-modified key (library has no Ctrl+z binding) must be
    // swallowed by the library routing, not fall through to the
    // queue's own Ctrl+z undo binding below it in `handle_queue_key`.
    let mut app = make_power_movie_app();
    app.queue_undo_stack.push(crate::app::UndoEntry::Remove(
        0,
        Box::new(crate::app::tests::make_item("removed", "Movie")),
    ));
    let stack_len_before = app.queue_undo_stack.len();

    app.handle_key(KeyEvent::new(KeyCode::Char('z'), KeyModifiers::CONTROL));

    assert_eq!(
        app.queue_undo_stack.len(),
        stack_len_before,
        "Ctrl+z must not pop the queue undo stack while the library panel is focused"
    );
}

#[test]
fn tab_cycles_from_right_panel_when_search_is_closed() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_power_tab_cycle_app();

    let handled = app.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));

    assert!(!handled);
    assert_eq!(app.library_tab, 2);
    assert_eq!(app.panel_focus, PanelFocus::Library);
}

#[test]
fn tab_cycles_from_right_panel_with_search_open() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_power_tab_cycle_app();
    app.libs[0].search = Some(LibSearch {
        query: "mov".into(),
        items: app.libs[0].nav_stack[0].items.clone(),
        results: vec![0],
        cursor: 0,
        scroll: 0,
        loading: false,
    });

    let handled = app.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));

    assert!(!handled);
    assert_eq!(app.library_tab, 2);
    assert_eq!(app.panel_focus, PanelFocus::Library);
    let search = app.libs[0]
        .search
        .as_ref()
        .expect("source search state should be untouched");
    assert_eq!(search.query, "mov");
    assert_eq!(search.cursor, 0);
    assert_eq!(search.results, vec![0]);
}

#[test]
fn backtab_cycles_from_right_panel_with_search_open() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_power_tab_cycle_app();
    app.library_tab = 2;
    app.libs[1].search = Some(LibSearch {
        query: "sho".into(),
        items: app.libs[1].nav_stack[0].items.clone(),
        results: vec![0],
        cursor: 0,
        scroll: 0,
        loading: false,
    });

    let handled = app.handle_key(KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT));

    assert!(!handled);
    assert_eq!(app.library_tab, 1);
    assert_eq!(app.panel_focus, PanelFocus::Library);
    let search = app.libs[1]
        .search
        .as_ref()
        .expect("source search state should be untouched");
    assert_eq!(search.query, "sho");
    assert_eq!(search.cursor, 0);
    assert_eq!(search.results, vec![0]);
}

#[test]
fn selecting_the_last_tab_scrolls_it_into_view_with_correct_arrows() {
    // Task 5 (#361): with more libraries than fit in the tab bar, the
    // tab bar must scroll the selected tab into view rather than
    // stranding it off-screen, and the `«`/`»` arrows (derived from the
    // same `visible_tab_range` call the slicing uses -- see
    // `render_tabs`) must reflect where the bar actually scrolled to.
    let mut app = make_app_stub();
    app.terminal_width = 40;
    for i in 0..10 {
        push_power_library(
            &mut app,
            &format!("lib-{i}"),
            &format!("Library Number {i}"),
        );
    }
    app.panel_focus = PanelFocus::Library;

    // Jump straight to the last tab, the same way `library_tab_next`
    // would land there after enough presses, and let it scroll into view.
    app.library_tab = app.tab_count() - 1;
    app.ensure_tab_visible();

    let tab_w = app
        .terminal_width
        .saturating_sub(crate::app::TABBAR_LEFT_RESERVE + crate::app::TABBAR_RIGHT_RESERVE);
    let (vis_start, vis_end) = app.visible_tab_range(tab_w);
    assert!(
        app.library_tab >= vis_start && app.library_tab < vis_end,
        "selected tab {} not in visible range {vis_start}..{vis_end}",
        app.library_tab
    );
    // The bar overflowed, so scrolling right of Home must show the
    // left-scroll arrow (there are tabs before vis_start).
    assert!(vis_start > 0, "expected the tab bar to have scrolled");
    // The last tab is visible and nothing follows it, so the
    // right-scroll arrow must not falsely claim there's more.
    assert_eq!(vis_end, app.tab_count());
}

#[test]
fn library_search_on_an_unfocused_library_survives_tab_cycling() {
    let mut app = make_app_stub();
    app.panel_focus = PanelFocus::Library;
    app.library_tab = 1;
    push_power_library(&mut app, "lib-movies", "Movies");
    push_power_library(&mut app, "lib-shows", "Shows");
    app.libs[0].search = Some(LibSearch {
        query: "mov".into(),
        items: app.libs[0].nav_stack[0].items.clone(),
        results: vec![0],
        cursor: 0,
        scroll: 0,
        loading: false,
    });

    let handled = app.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));

    assert!(!handled);
    let search = app.libs[0]
        .search
        .as_ref()
        .expect("plain library search should stay open");
    assert_eq!(search.query, "mov");
    assert_eq!(search.cursor, 0);
    assert_eq!(search.results, vec![0]);
}

#[test]
fn ctrl_z_while_power_queue_panel_focused_does_trigger_undo() {
    // Positive counterpart: with queue focus (not library focus), the
    // same Ctrl+z reaches `handle_queue_key`'s own binding.
    let mut app = make_power_movie_app();
    app.panel_focus = PanelFocus::Queue;
    app.queue_undo_stack.push(crate::app::UndoEntry::Remove(
        0,
        Box::new(crate::app::tests::make_item("removed", "Movie")),
    ));

    app.handle_key(KeyEvent::new(KeyCode::Char('z'), KeyModifiers::CONTROL));

    assert!(
        app.queue_undo_stack.is_empty(),
        "Ctrl+z pops the queue undo stack when the queue panel is focused"
    );
}

// ── #145 task 5: regression coverage for other library shortcuts ──
// These paths (queue-panel PageUp/PageDown and the movie context menu)
// sit in the same functions the new track-focus interception lives in
// (the power-left key dispatch match block and `open_context_menu`), but
// are untouched by tasks 1-4: the queue PageUp/PageDown block is a
// separate `if` gated on `PanelFocus::Queue`, entirely outside the
// `is_viewing_album_folders`-gated block added for track-selection mode;
// and `open_context_menu`'s non-folder branch (which a `Movie` item
// takes) doesn't consult `album_track_focus` or `is_viewing_album_folders`
// at all -- only `current_lib_item()` does, and that new branch is
// itself gated on `is_viewing_album_folders`, which is always false for
// a movies library (`collection_type != "music"`).

#[test]
fn queue_page_up_and_down_move_queue_cursor_when_queue_panel_focused() {
    let mut app = make_power_movie_app();
    app.panel_focus = PanelFocus::Queue;
    app.layout.main.queue_area = Rect::new(0, 0, 20, 11);

    for i in 0..20 {
        let mut item = make_item(&format!("Queued {i}"), "Movie");
        item.id = format!("queued-{i}");
        app.player_tab.items.push(item);
    }
    app.player_tab.queue_cursor = 15;

    let handled = app.handle_key(KeyEvent::new(KeyCode::PageUp, KeyModifiers::NONE));
    assert!(!handled);
    assert_eq!(
        app.player_tab.queue_cursor, 5,
        "PageUp should move the queue cursor back by the queue panel's page size \
             (height.saturating_sub(1)), unaffected by #145's album-folder changes"
    );

    let handled = app.handle_key(KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE));
    assert!(!handled);
    assert_eq!(
        app.player_tab.queue_cursor, 15,
        "PageDown should move the queue cursor forward by the same page size"
    );
}

#[test]
fn power_library_navigation_stays_debounced_after_focus_moves_to_queue() {
    let mut app = make_power_movie_app();

    app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    assert!(!app.power_right_panel_image_renders_allowed());

    app.handle_key(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE));
    assert_eq!(app.panel_focus, PanelFocus::Queue);
    assert!(!app.power_right_panel_image_renders_allowed());
}

#[test]
fn power_queue_navigation_keeps_right_panel_gate_open_after_focus_moves_to_library() {
    let mut app = make_power_movie_app();
    app.panel_focus = PanelFocus::Queue;
    app.layout.main.queue_area = Rect::new(0, 0, 20, 4);
    for i in 0..5 {
        let mut item = make_item(&format!("Queued {i}"), "Movie");
        item.id = format!("queued-{i}");
        app.player_tab.items.push(item);
    }
    app.player_tab.queue_cursor = 0;
    let power_library_nav_at = app.last_power_library_nav_at;
    assert!(app.power_right_panel_image_renders_allowed());

    app.handle_key(KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE));
    assert_eq!(app.player_tab.queue_cursor, 3);
    assert_eq!(app.last_power_library_nav_at, power_library_nav_at);
    assert!(app.power_right_panel_image_renders_allowed());

    app.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE));

    assert_eq!(app.panel_focus, PanelFocus::Library);
    assert!(app.power_right_panel_image_renders_allowed());
}

#[test]
fn movie_context_menu_offers_unaffected_non_folder_verbs() {
    let mut app = make_power_movie_app();

    app.handle_key(KeyEvent::new(KeyCode::Char('.'), KeyModifiers::NONE));

    let menu = app
        .context_menu
        .as_ref()
        .expect("expected a context menu for the focused movie");
    let labels: Vec<&str> = menu.entries.iter().map(|e| e.label).collect();
    assert_eq!(
        labels,
        vec!["Play", "Add to Queue", "Mark Watched"],
        "a movie (non-folder, non-music) item's context menu must keep its \
             original verb set, untouched by #145's album/track scoping \
             (which only ever changes the folder branch and only for music \
             libraries): {labels:?}"
    );
}

// Regression coverage for the letter-pills PR review finding: opening
// `/`-search while a letter-range pill is active must always trigger a
// full-library fetch, even when the active range's slice is already
// "fully loaded" by its own (small, filtered) total_count. Before the
// fix, `needs_full_load` compared `items.len()` against the level's
// (filtered) `total_count`, so a fully-loaded small range read as
// "nothing more to fetch" and search silently ran over just that range.
#[test]
fn opening_search_with_an_active_letter_pill_always_needs_a_full_library_fetch() {
    let mut app = make_power_movie_app();
    {
        let lvl = app.libs[0].nav_stack.last_mut().unwrap();
        // Simulate a selected, fully-loaded M–O range: only 2 items, and
        // the level's own total_count (what get_items_sorted_ranged
        // reported for that range) matches items.len() exactly -- the
        // state that used to wrongly look "fully loaded" for search.
        lvl.letter_filter = crate::app::render::LetterFilter::for_index(4);
        lvl.total_count = lvl.items.len();
    }
    app.libs[0].library_total = Some(3000); // the library's true size

    // `handle_key` returns whether the app should quit, not whether the
    // key was "handled" -- the `/` handler returns `Some(false)`, so a
    // successful (non-quitting) key press yields `false` here.
    let should_quit = app.handle_key(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE));

    assert!(!should_quit);
    let search = app.libs[0].search.as_ref().expect("search should open");
    assert!(
        search.loading,
        "a full-library fetch must be in flight, not just the active M–O range"
    );
}
