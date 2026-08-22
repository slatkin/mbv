#![allow(dead_code, unused_imports)]

use super::library_scope_routing_tests::{make_library_app, make_library_mouse_event};
use super::*;
use crate::app::tests::{make_app_stub, make_item};
use crate::app::{SelectionModalRow, SelectionModalSource, SeriesDetail};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEventKind};
use ratatui::layout::Rect;

fn make_series_app() -> App {
    let mut app = make_library_app();
    for item in app.libs[0].nav_stack[0].items.iter_mut() {
        item.is_folder = true;
        item.item_type = "Series".into();
    }
    app.libs[0].library.collection_type = "tvshows".into();
    app
}

// Regression coverage for the inline hero's click handling (issue found in
// review of #448): a single click on the hero must only focus the library
// panel, matching the app-wide "single click only focuses; double-click
// activates" convention (see `mouse_click_on_a_different_folder_row_only_focuses_it`
// / `double_click_on_a_folder_row_drills_in` above). Activation used to fire
// from a single click inside `click_set_cursor`, bypassing that convention
// entirely for the hero.
#[test]
fn single_click_on_hero_only_focuses_the_panel() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_series_app();
    app.panel_focus = PanelFocus::Queue;
    app.layout.main.hero_area = Rect {
        x: 10,
        y: 10,
        width: 20,
        height: 5,
    };
    app.layout.main.inline_hero_area = app.layout.main.hero_area;

    app.handle_mouse(make_library_mouse_event(
        MouseEventKind::Down(MouseButton::Left),
        12,
        11,
    ));

    assert_eq!(
        app.panel_focus,
        PanelFocus::Library,
        "click focuses the panel"
    );
    assert_eq!(
        app.libs[0].series_selection, None,
        "a single click on the hero must not enter series selection"
    );
    assert_eq!(app.libs[0].nav_stack.len(), 1);
}

#[test]
fn narrow_double_click_on_hero_opens_the_selection_modal() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_series_app();
    app.layout.main.hero_area = Rect {
        x: 10,
        y: 10,
        width: 20,
        height: 5,
    };
    app.layout.main.inline_hero_area = app.layout.main.hero_area;

    let click = make_library_mouse_event(MouseEventKind::Down(MouseButton::Left), 12, 11);
    app.handle_mouse(click);
    assert_eq!(
        app.libs[0].series_selection, None,
        "the first click of the pair only focuses"
    );

    app.handle_mouse(click);
    assert_eq!(
        app.libs[0].series_selection, None,
        "narrow double-click must not enter invisible wide-only Series focus"
    );
    assert!(
        app.selection_modal.is_some(),
        "narrow double-click on the hero must open the Series selection modal"
    );
}

#[test]
fn wide_double_click_on_hero_keeps_series_workspace() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_series_app();
    app.layout.main.hero_area = Rect::new(10, 10, 20, 5);
    app.layout.main.inline_hero_area = app.layout.main.hero_area;
    app.layout.main.tv_wide_right_area = Rect::new(40, 0, 20, 15);
    assert!(app.layout.main.is_wide_tv_active());

    let click = make_library_mouse_event(MouseEventKind::Down(MouseButton::Left), 42, 3);
    app.handle_mouse(click);
    app.handle_mouse(click);

    assert_eq!(
        app.libs[0].series_selection,
        Some(0),
        "wide double-click must retain the persistent Series workspace"
    );
    assert!(
        app.selection_modal.is_none(),
        "wide double-click must not open the narrow Series modal"
    );
}

#[test]
fn wide_series_episode_target_changes_episode_focus() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_series_app();
    app.layout.main.browse_destination = Some(app.tab);
    app.layout.main.tv_wide_right_area = Rect::new(40, 0, 20, 15);
    app.layout.main.tv_wide_episode_rows = vec![(Rect::new(42, 4, 16, 1), 1)];

    assert!(app.click_set_cursor(43, 4));
    assert_eq!(app.libs[0].series_selection, Some(1));
}

#[test]
fn wide_read_only_home_and_feed_heroes_are_inert() {
    let _guard = crate::config::TestStateDirGuard::new();
    for tab in [TabSelection::Home, TabSelection::Feeds] {
        let mut app = make_app_stub();
        app.tab = tab;
        app.panel_focus = PanelFocus::Queue;
        app.layout.main.hero_area = Rect::new(10, 10, 20, 5);

        assert!(
            !app.click_set_cursor(12, 11),
            "wide read-only hero must not handle clicks for {tab:?}"
        );
        assert_eq!(app.panel_focus, PanelFocus::Queue);
    }
}

#[test]
fn wide_read_only_home_and_feed_double_clicks_are_inert() {
    let _guard = crate::config::TestStateDirGuard::new();
    for tab in [TabSelection::Home, TabSelection::Feeds] {
        let mut app = make_app_stub();
        app.tab = tab;
        app.panel_focus = PanelFocus::Library;
        app.home.continue_items = vec![make_item("Home item", "Movie")];
        app.layout.main.browse_destination = Some(tab);
        app.layout.main.hero_area = Rect::new(10, 10, 20, 5);
        app.layout.main.left_area = Rect::new(40, 10, 20, 5);
        app.refocus_at = Some(std::time::Instant::now() - std::time::Duration::from_secs(1));
        let click = make_library_mouse_event(MouseEventKind::Down(MouseButton::Left), 12, 11);

        app.handle_mouse(click);
        app.handle_mouse(click);

        assert!(
            !app.player.status.lock().unwrap().active,
            "{tab:?} hero activated"
        );
    }
}

/// Narrow (`is_wide_tv_active() == false`, the default zero-area layout)
/// Enter on a selected Series opens the selection modal instead of entering
/// `series_selection` (design.md Decision 6/7; task 2.3).
#[test]
fn narrow_series_enter_opens_selection_modal_with_season_and_episode_rows() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_series_app();
    assert!(!app.layout.main.is_wide_tv_active());

    let mut season = make_item("Season 1", "Season");
    season.id = "season-1".into();
    season.index_number = 1;
    let mut episode = make_item("Pilot", "Episode");
    episode.id = "episode-1".into();
    episode.index_number = 1;
    let mut episodes = std::collections::HashMap::new();
    episodes.insert("season-1".into(), vec![episode]);
    app.series_detail_cache.insert(
        "id0".into(),
        SeriesDetail {
            seasons: vec![season],
            episodes,
        },
    );

    let handled = app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    assert!(!handled);
    assert_eq!(
        app.libs[0].series_selection, None,
        "narrow Enter must not enter the in-hero series-selection mode"
    );
    let modal = app
        .selection_modal
        .as_ref()
        .expect("Enter on a narrow Series must open the selection modal");
    assert!(matches!(modal.source, SelectionModalSource::Series { .. }));
    assert!(
        modal
            .state
            .rows()
            .iter()
            .any(|row| matches!(row, SelectionModalRow::Header(name) if name == "Season 1")),
        "expected a season header row"
    );
    assert!(
        modal.state.rows().iter().any(|row| matches!(
            row,
            SelectionModalRow::Item(item) if item.id == "episode-1"
        )),
        "expected an episode item row"
    );
}

#[test]
fn series_modal_projects_defined_season_pills_and_selected_season_rows() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_series_app();
    let mut season_one = make_item("Season 1", "Season");
    season_one.id = "season-1".into();
    season_one.index_number = 1;
    let mut season_two = make_item("Season 2", "Season");
    season_two.id = "season-2".into();
    season_two.index_number = 2;
    let mut episodes = std::collections::HashMap::new();
    episodes.insert(
        "season-1".into(),
        vec![{
            let mut episode = make_item("Pilot", "Episode");
            episode.id = "episode-1".into();
            episode
        }],
    );
    episodes.insert(
        "season-2".into(),
        vec![{
            let mut episode = make_item("Finale", "Episode");
            episode.id = "episode-2".into();
            episode
        }],
    );
    app.series_detail_cache.insert(
        "id0".into(),
        SeriesDetail {
            seasons: vec![season_one, season_two],
            episodes,
        },
    );

    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    let modal = app.selection_modal.as_ref().expect("modal open");
    let filter = modal
        .filter
        .as_ref()
        .expect("season pills are modal-scoped");
    assert_eq!(filter.labels, vec!["01", "02"]);
    assert_eq!(filter.selected, 0);
    assert!(modal
        .state
        .rows()
        .iter()
        .any(|row| matches!(row, SelectionModalRow::Item(item) if item.id == "episode-1")));
    assert!(!modal
        .state
        .rows()
        .iter()
        .any(|row| matches!(row, SelectionModalRow::Item(item) if item.id == "episode-2")));

    app.handle_key(KeyEvent::new(KeyCode::Char(']'), KeyModifiers::NONE));
    let modal = app.selection_modal.as_ref().expect("modal stays open");
    assert_eq!(modal.filter.as_ref().unwrap().selected, 1);
    assert!(modal
        .state
        .rows()
        .iter()
        .any(|row| matches!(row, SelectionModalRow::Item(item) if item.id == "episode-2")));
    assert!(!modal
        .state
        .rows()
        .iter()
        .any(|row| matches!(row, SelectionModalRow::Item(item) if item.id == "episode-1")));
}

/// Wide (`is_wide_tv_active() == true`) Enter on a selected Series keeps the
/// existing in-hero episode focus, unaffected by the narrow modal routing
/// added for task 2.3.
#[test]
fn wide_series_enter_still_enters_series_selection_not_the_modal() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_series_app();
    app.layout.main.tv_wide_right_area = Rect::new(10, 0, 20, 10);
    assert!(app.layout.main.is_wide_tv_active());

    let handled = app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    assert!(!handled);
    assert_eq!(
        app.libs[0].series_selection,
        Some(0),
        "wide Enter must still enter the in-hero series-selection mode"
    );
    assert!(
        app.selection_modal.is_none(),
        "wide Enter must not open the selection modal"
    );
}

#[test]
fn keyboard_series_change_resets_the_season_cursor_before_first_season_fetch() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_series_app();
    app.layout.main.tv_wide_right_area = Rect::new(10, 0, 20, 10);
    app.libs[0].series_season_cursor = 1;

    app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));

    assert_eq!(app.libs[0].series_season_cursor, 0);
}

/// Narrow (`is_wide_music_active() == false`, the default zero-area layout)
/// Enter on a selected album opens the selection modal instead of entering
/// the in-hero `album_track_focus` mode (design.md decision 6; task 3.3).
#[test]
fn narrow_album_enter_opens_selection_modal_with_track_rows() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = super::music_track_test_support::make_music_album_app();
    assert!(!app.layout.main.is_wide_music_active());
    super::music_track_test_support::push_tracks(&mut app, "album-1", 2);

    let handled = app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    assert!(!handled);
    assert_eq!(
        app.libs[0].album_track_focus, None,
        "narrow Enter must not enter the in-hero track-focus mode"
    );
    let modal = app
        .selection_modal
        .as_ref()
        .expect("Enter on a narrow album must open the selection modal");
    assert!(matches!(modal.source, SelectionModalSource::Album { .. }));
    assert!(
        modal.state.rows().iter().any(
            |row| matches!(row, SelectionModalRow::Item(item) if item.id == "album-1-track-0")
        ),
        "expected a track item row"
    );
}

#[test]
fn narrow_album_modal_enter_activates_the_selected_track() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = super::music_track_test_support::make_music_album_app();
    super::music_track_test_support::push_tracks(&mut app, "album-1", 1);

    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    assert!(
        app.selection_modal.is_none(),
        "track activation closes the modal"
    );
    assert_eq!(app.status, "Emby is unavailable");
}

#[test]
fn mouse_click_on_modal_row_activates_that_row_not_the_library() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = super::music_track_test_support::make_music_album_app();
    super::music_track_test_support::push_tracks(&mut app, "album-1", 2);
    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    app.layout.main.selection_modal_area = Rect::new(10, 4, 40, 8);
    app.layout.main.selection_modal_rows = vec![(Rect::new(11, 6, 38, 1), 1)];
    app.layout.main.left_area = Rect::new(0, 0, 60, 20);
    app.layout.main.left_row_map = vec![Some(0)];

    app.handle_mouse(make_library_mouse_event(
        MouseEventKind::Down(MouseButton::Left),
        12,
        6,
    ));

    assert!(app.selection_modal.is_none(), "modal row click activates");
    assert_eq!(app.libs[0].nav_stack[0].cursor, 0);
    assert_eq!(app.libs[0].album_track_focus, None);
}

#[test]
fn mouse_click_outside_selection_modal_dismisses_without_browse_action() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = super::music_track_test_support::make_music_album_app();
    super::music_track_test_support::push_tracks(&mut app, "album-1", 1);
    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    app.layout.main.selection_modal_area = Rect::new(10, 4, 40, 8);
    app.layout.main.left_area = Rect::new(0, 0, 60, 20);
    app.layout.main.left_row_map = vec![Some(0)];

    app.handle_mouse(make_library_mouse_event(
        MouseEventKind::Down(MouseButton::Left),
        2,
        2,
    ));

    assert!(app.selection_modal.is_none());
    assert_eq!(app.libs[0].nav_stack[0].cursor, 0);
}

#[test]
fn narrow_series_modal_enter_activates_the_selected_episode() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_series_app();
    let mut season = make_item("Season 1", "Season");
    season.id = "season-1".into();
    let mut episode = make_item("Pilot", "Episode");
    episode.id = "episode-1".into();
    let mut episodes = std::collections::HashMap::new();
    episodes.insert("season-1".into(), vec![episode]);
    app.series_detail_cache.insert(
        "id0".into(),
        SeriesDetail {
            seasons: vec![season],
            episodes,
        },
    );

    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    assert!(
        app.selection_modal.is_none(),
        "episode activation closes the modal"
    );
    assert_eq!(app.status, "Emby is unavailable");
}

/// Wide (`is_wide_music_active() == true`) Enter on a selected album keeps
/// the existing in-hero track-focus mode, unaffected by the narrow modal
/// routing added for task 3.3.
#[test]
fn wide_album_enter_still_enters_track_focus_not_the_modal() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = super::music_track_test_support::make_music_album_app();
    app.layout.main.wide_music_right_area = Rect::new(10, 0, 20, 10);
    assert!(app.layout.main.is_wide_music_active());

    let handled = app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    assert!(!handled);
    assert_eq!(
        app.libs[0].album_track_focus,
        Some(0),
        "wide Enter must still enter the in-hero track-focus mode"
    );
    assert!(
        app.selection_modal.is_none(),
        "wide Enter must not open the selection modal"
    );
}

fn series_detail_with_two_seasons() -> SeriesDetail {
    let mut season_one = make_item("Season 1", "Season");
    season_one.id = "season-1".into();
    season_one.index_number = 1;
    let mut season_two = make_item("Season 2", "Season");
    season_two.id = "season-2".into();
    season_two.index_number = 2;

    let mut episode_one = make_item("Pilot", "Episode");
    episode_one.id = "episode-1".into();
    let mut episode_two = make_item("Finale", "Episode");
    episode_two.id = "episode-2".into();
    let mut episodes = std::collections::HashMap::new();
    episodes.insert("season-1".into(), vec![episode_one, episode_two]);
    episodes.insert("season-2".into(), vec![make_item("Special", "Episode")]);

    SeriesDetail {
        seasons: vec![season_one, season_two],
        episodes,
    }
}

#[test]
fn series_modal_keyboard_navigation_skips_headers_and_preserves_parent_position() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_series_app();
    app.series_detail_cache
        .insert("id0".into(), series_detail_with_two_seasons());
    app.libs[0].nav_stack[0].scroll = 7;

    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert_eq!(app.selection_modal.as_ref().unwrap().cursor, 1);
    app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    assert_eq!(app.selection_modal.as_ref().unwrap().cursor, 2);
    app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    assert_eq!(app.selection_modal.as_ref().unwrap().cursor, 2);
    app.handle_key(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE));
    assert_eq!(app.selection_modal.as_ref().unwrap().cursor, 1);

    app.handle_key(KeyEvent::new(KeyCode::Char(']'), KeyModifiers::NONE));
    let modal = app.selection_modal.as_ref().unwrap();
    assert_eq!(modal.filter.as_ref().unwrap().selected, 1);
    assert_eq!(
        modal.cursor, 1,
        "season changes re-anchor on the first item"
    );
    assert_eq!(modal.state.rows()[0].item_id(), None);
    app.handle_key(KeyEvent::new(KeyCode::Char('['), KeyModifiers::NONE));
    assert_eq!(
        app.selection_modal
            .as_ref()
            .unwrap()
            .filter
            .as_ref()
            .unwrap()
            .selected,
        0
    );

    for key in [KeyCode::Esc, KeyCode::Backspace] {
        let mut app = make_series_app();
        app.series_detail_cache
            .insert("id0".into(), series_detail_with_two_seasons());
        app.libs[0].nav_stack[0].scroll = 7;
        app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        app.handle_key(KeyEvent::new(key, KeyModifiers::NONE));
        assert!(app.selection_modal.is_none());
        assert_eq!(app.panel_focus, PanelFocus::Library);
        assert_eq!(app.libs[0].nav_stack[0].cursor, 0);
        assert_eq!(app.libs[0].nav_stack[0].scroll, 7);
    }
}

#[test]
fn series_modal_loading_and_empty_states_ignore_movement_and_activation() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut loading = make_series_app();
    loading.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert!(matches!(
        loading.selection_modal.as_ref().unwrap().state,
        crate::app::SelectionModalListState::Loading
    ));
    loading.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    loading.handle_key(KeyEvent::new(KeyCode::Char(']'), KeyModifiers::NONE));
    loading.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert!(loading.selection_modal.is_none());

    let mut empty = make_series_app();
    let detail = series_detail_with_two_seasons();
    let mut episodes = std::collections::HashMap::new();
    episodes.insert("season-1".into(), Vec::new());
    episodes.insert("season-2".into(), Vec::new());
    empty.series_detail_cache.insert(
        "id0".into(),
        SeriesDetail {
            seasons: detail.seasons,
            episodes,
        },
    );
    empty.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert!(matches!(
        empty.selection_modal.as_ref().unwrap().state,
        crate::app::SelectionModalListState::Empty
    ));
    empty.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    empty.handle_key(KeyEvent::new(KeyCode::Char(']'), KeyModifiers::NONE));
    empty.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert!(empty.selection_modal.is_none());
}

#[test]
fn music_modal_keyboard_navigation_has_no_pills_and_preserves_parent_position() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = super::music_track_test_support::make_music_album_app();
    super::music_track_test_support::push_tracks(&mut app, "album-1", 3);
    app.libs[0].nav_stack[1].scroll = 5;

    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    assert_eq!(app.selection_modal.as_ref().unwrap().cursor, 1);
    app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    assert_eq!(app.selection_modal.as_ref().unwrap().cursor, 2);
    app.handle_key(KeyEvent::new(KeyCode::Char('['), KeyModifiers::NONE));
    app.handle_key(KeyEvent::new(KeyCode::Char(']'), KeyModifiers::NONE));
    let modal = app.selection_modal.as_ref().unwrap();
    assert!(modal.filter.is_none());
    assert_eq!(
        modal.state.rows()[modal.cursor].item_id(),
        Some("album-1-track-2")
    );

    for key in [KeyCode::Esc, KeyCode::Backspace] {
        let mut app = super::music_track_test_support::make_music_album_app();
        super::music_track_test_support::push_tracks(&mut app, "album-1", 2);
        app.libs[0].nav_stack[1].scroll = 5;
        app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        app.handle_key(KeyEvent::new(key, KeyModifiers::NONE));
        assert!(app.selection_modal.is_none());
        assert_eq!(app.panel_focus, PanelFocus::Library);
        assert_eq!(app.libs[0].nav_stack[1].cursor, 0);
        assert_eq!(app.libs[0].nav_stack[1].scroll, 5);
    }
}

#[test]
fn music_modal_loading_and_empty_states_ignore_movement_and_activation() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut cold = super::music_track_test_support::make_music_album_app();
    let album = cold.libs[0].nav_stack[1].items[0].clone();
    cold.open_album_selection_modal(&album);
    assert!(matches!(
        cold.selection_modal.as_ref().unwrap().state,
        crate::app::SelectionModalListState::Loading
    ));
    cold.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    cold.handle_key(KeyEvent::new(KeyCode::Char(']'), KeyModifiers::NONE));
    cold.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert!(cold.selection_modal.is_none());

    let mut empty = super::music_track_test_support::make_music_album_app();
    empty
        .album_tracks_cache
        .insert("album-1".into(), Vec::new());
    let album = empty.libs[0].nav_stack[1].items[0].clone();
    empty.open_album_selection_modal(&album);
    assert!(matches!(
        empty.selection_modal.as_ref().unwrap().state,
        crate::app::SelectionModalListState::Empty
    ));
    empty.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    empty.handle_key(KeyEvent::new(KeyCode::Char('['), KeyModifiers::NONE));
    empty.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert!(empty.selection_modal.is_none());
}
