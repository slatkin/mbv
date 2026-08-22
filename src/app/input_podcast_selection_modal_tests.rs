use crate::app::{SelectionModalListState, SelectionModalRow, SelectionModalSource};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;

/// Cycling the podcast selection modal's played/unplayed filter with `[`/`]`
/// (design.md decision 4; task 4.4) rebuilds `modal.rows` from the newly
/// filtered episode list.
#[test]
fn podcast_selection_modal_filter_cycle_updates_episode_rows() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = crate::app::tests_podcast::audiobookshelf_app();
    app.audiobookshelf_browse[0].progress.insert(
        ("show-a".into(), "episode-a".into()),
        mbv_core::audiobookshelf::AudiobookshelfProgress {
            library_item_id: "show-a".into(),
            episode_id: "episode-a".into(),
            current_time_seconds: 0.0,
            is_finished: true,
        },
    );
    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    let modal = app.selection_modal.as_ref().expect("modal open");
    assert_eq!(modal.filter.as_ref().unwrap().selected, 0, "starts on All");
    assert!(modal
        .state
        .rows()
        .iter()
        .any(|row| matches!(row, SelectionModalRow::Item(item) if item.id == "episode-a")));

    app.handle_key(KeyEvent::new(KeyCode::Char(']'), KeyModifiers::NONE));
    let modal = app.selection_modal.as_ref().expect("modal still open");
    assert_eq!(
        modal.filter.as_ref().unwrap().selected,
        1,
        "cycled to Played"
    );
    assert!(
        modal
            .state
            .rows()
            .iter()
            .any(|row| matches!(row, SelectionModalRow::Item(item) if item.id == "episode-a")),
        "Played filter still shows the played episode"
    );

    app.handle_key(KeyEvent::new(KeyCode::Char(']'), KeyModifiers::NONE));
    let modal = app.selection_modal.as_ref().expect("modal still open");
    assert_eq!(
        modal.filter.as_ref().unwrap().selected,
        2,
        "cycled to Unplayed"
    );
    assert!(
        !modal
            .state
            .rows()
            .iter()
            .any(|row| matches!(row, SelectionModalRow::Item(_))),
        "Unplayed filter hides the played episode"
    );
}

#[test]
fn podcast_modal_projects_pending_detail_as_loading() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = crate::app::tests_podcast::audiobookshelf_app();
    app.audiobookshelf_browse[0].episodes = None;
    app.audiobookshelf_browse[0].detail_loading = true;

    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    assert!(matches!(
        app.selection_modal.as_ref().unwrap().state,
        SelectionModalListState::Loading
    ));
}

#[test]
fn podcast_modal_filter_movement_preserves_loading_while_detail_is_pending() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = crate::app::tests_podcast::audiobookshelf_app();
    app.audiobookshelf_browse[0].episodes = None;
    app.audiobookshelf_browse[0].detail_loading = true;
    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    app.handle_key(KeyEvent::new(KeyCode::Char(']'), KeyModifiers::NONE));

    assert_eq!(
        app.selection_modal
            .as_ref()
            .unwrap()
            .filter
            .as_ref()
            .unwrap()
            .selected,
        1
    );
    assert!(matches!(
        app.selection_modal.as_ref().unwrap().state,
        SelectionModalListState::Loading
    ));
}

#[test]
fn podcast_modal_filter_movement_preserves_loading_without_detail_cache() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = crate::app::tests_podcast::audiobookshelf_app();
    app.audiobookshelf_browse[0].episodes = None;
    app.audiobookshelf_browse[0].detail_loading = false;
    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    app.handle_key(KeyEvent::new(KeyCode::Char(']'), KeyModifiers::NONE));

    assert!(matches!(
        app.selection_modal.as_ref().unwrap().state,
        SelectionModalListState::Loading
    ));
}

#[test]
fn clicking_podcast_selection_modal_filter_updates_episode_rows() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = crate::app::tests_podcast::audiobookshelf_app();
    app.audiobookshelf_browse[0].progress.insert(
        ("show-a".into(), "episode-a".into()),
        mbv_core::audiobookshelf::AudiobookshelfProgress {
            library_item_id: "show-a".into(),
            episode_id: "episode-a".into(),
            current_time_seconds: 0.0,
            is_finished: true,
        },
    );
    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    app.select_audiobookshelf_filter(2);

    let modal = app.selection_modal.as_ref().expect("modal still open");
    assert_eq!(modal.filter.as_ref().unwrap().selected, 2);
    assert!(
        !modal
            .state
            .rows()
            .iter()
            .any(|row| matches!(row, SelectionModalRow::Item(_))),
        "Unplayed filter must rebuild the modal episode rows"
    );
}

#[test]
fn mouse_click_on_podcast_modal_filter_uses_modal_state() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = crate::app::tests_podcast::audiobookshelf_app();
    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    app.layout.main.selection_modal_area = Rect::new(10, 2, 40, 10);
    app.layout.main.selector_tabs = vec![(Rect::new(12, 3, 10, 1), 2)];
    app.layout.main.left_area = Rect::new(0, 0, 60, 20);
    app.layout
        .main
        .selector_tabs
        .push((Rect::new(1, 1, 5, 1), 0));

    app.handle_mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 13,
        row: 3,
        modifiers: KeyModifiers::NONE,
    });

    assert_eq!(
        app.selection_modal
            .as_ref()
            .and_then(|modal| modal.filter.as_ref())
            .map(|filter| filter.selected),
        Some(2)
    );
    assert!(app.selection_modal.is_some());
    assert_eq!(app.audiobookshelf_browse[0].cursor(), 0);
}

#[test]
fn narrow_podcast_child_target_cannot_change_episode_selection() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = crate::app::tests_podcast::audiobookshelf_app();
    app.audiobookshelf_browse[0].episode_selection = Some(0);
    app.layout.main.browse_destination = Some(app.tab);
    app.layout.main.audiobookshelf_episode_rows = vec![(Rect::new(12, 6, 20, 1), 1)];

    assert!(!app.layout.main.is_wide_podcast_active());
    app.handle_mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 13,
        row: 6,
        modifiers: KeyModifiers::NONE,
    });

    assert_eq!(app.audiobookshelf_browse[0].episode_selection, Some(0));
}

/// Narrow (`is_wide_podcast_active() == false`, the default zero-area
/// layout) Enter on a selected podcast show opens the selection modal
/// instead of entering the in-hero `episode_selection` mode (design.md
/// decision 6; task 4.4).
#[test]
fn narrow_podcast_enter_opens_selection_modal_with_episode_rows() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = crate::app::tests_podcast::audiobookshelf_app();
    assert!(!app.layout.main.is_wide_podcast_active());

    let handled = app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    assert!(!handled);
    assert_eq!(
        app.audiobookshelf_browse[0].episode_selection, None,
        "narrow Enter must not enter the in-hero episode-selection mode"
    );
    let modal = app
        .selection_modal
        .as_ref()
        .expect("Enter on a narrow podcast show must open the selection modal");
    assert!(matches!(modal.source, SelectionModalSource::Podcast { .. }));
    assert!(
        modal.filter.is_some(),
        "podcast modal shows the played/unplayed filter pills"
    );
    assert!(
        modal
            .state
            .rows()
            .iter()
            .any(|row| matches!(row, SelectionModalRow::Item(item) if item.id == "episode-a")),
        "expected an episode item row"
    );
}

#[test]
fn narrow_podcast_double_click_opens_selection_modal_like_enter() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = crate::app::tests_podcast::audiobookshelf_app();
    app.layout.main.left_area = Rect::new(10, 5, 20, 5);
    let click = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 12,
        row: 6,
        modifiers: KeyModifiers::NONE,
    };

    app.handle_mouse(click);
    assert!(app.selection_modal.is_none());
    app.handle_mouse(click);

    assert!(app.audiobookshelf_browse[0].episode_selection.is_none());
    assert!(matches!(
        app.selection_modal.as_ref().map(|modal| &modal.source),
        Some(SelectionModalSource::Podcast { .. })
    ));
}

#[test]
fn podcast_alphabetical_bucket_selects_a_show() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = crate::app::tests_podcast::audiobookshelf_app();
    let mut zulu = app.audiobookshelf_browse[0].shows[0].clone();
    zulu.library_item_id = "show-zulu".into();
    zulu.title = "Zulu Show".into();
    app.audiobookshelf_browse[0].shows.push(zulu);
    app.audiobookshelf_browse[0]
        .shows
        .sort_by_key(|show| show.title.to_lowercase());

    app.select_audiobookshelf_podcast_bucket(1);

    assert_eq!(app.audiobookshelf_browse[0].cursor(), 1);
    assert_eq!(
        app.audiobookshelf_browse[0].selected_show().unwrap().title,
        "Zulu Show"
    );
}

/// Wide (`is_wide_podcast_active() == true`) Enter on a selected podcast show
/// keeps the existing in-hero episode focus, unaffected by the narrow modal
/// routing added for task 4.4.
#[test]
fn wide_podcast_enter_still_enters_episode_selection_not_the_modal() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = crate::app::tests_podcast::audiobookshelf_app();
    app.layout.main.audiobookshelf_podcast_right_area = Rect::new(10, 0, 20, 10);
    assert!(app.layout.main.is_wide_podcast_active());

    let handled = app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    assert!(!handled);
    assert_eq!(
        app.audiobookshelf_browse[0].episode_selection,
        Some(0),
        "wide Enter must still enter the in-hero episode-selection mode"
    );
    assert!(
        app.selection_modal.is_none(),
        "wide Enter must not open the selection modal"
    );
}

/// Enter on a podcast selection-modal item resolves the episode within the
/// currently selected show's own `visible_episodes()` (episode ids are only
/// unique per-show), then consumes the activation without playback or queue
/// side effects (podcast-library spec, task 4.4).
#[test]
fn podcast_selection_modal_enter_is_read_only() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = crate::app::tests_podcast::audiobookshelf_app();
    app.audiobookshelf_browse[0]
        .episodes
        .as_mut()
        .unwrap()
        .push(mbv_core::audiobookshelf::AudiobookshelfDownloadedEpisode {
            library_item_id: "show-a".into(),
            episode_id: "episode-b".into(),
            title: "Episode B".into(),
            published_at: None,
            duration_seconds: None,
        });
    let context = mbv_core::player::AudiobookshelfPlayerContext::new(
        mbv_core::service_runtime::SetupGeneration::new(1),
        mbv_core::config::AudiobookshelfSetup::new("https://books.example"),
        "secret".into(),
        "device".into(),
    )
    .expect("valid test Audiobookshelf context");
    app.player.update_audiobookshelf_context(Some(context));

    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    let modal = app.selection_modal.as_ref().expect("modal open");
    let target_id = match &modal.state.rows()[modal.cursor] {
        SelectionModalRow::Item(item) if item.id == "episode-a" => {
            app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
            "episode-b"
        }
        SelectionModalRow::Item(_) => "episode-a",
        SelectionModalRow::Header(_) => panic!("podcast modal has no header rows"),
    };
    // Confirm the cursor now sits on the row we intend to activate.
    let modal = app.selection_modal.as_ref().expect("modal still open");
    assert!(matches!(
        &modal.state.rows()[modal.cursor],
        SelectionModalRow::Item(item) if item.id == target_id
    ));

    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    assert!(
        app.selection_modal.is_none(),
        "activating a modal item must close the modal"
    );
    assert_eq!(app.player_tab.total_queue_len(), 0);
}

#[test]
fn podcast_modal_keyboard_navigation_and_cancellation_preserve_parent_position() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = crate::app::tests_podcast::audiobookshelf_app();
    app.audiobookshelf_browse[0]
        .episodes
        .as_mut()
        .unwrap()
        .extend([
            mbv_core::audiobookshelf::AudiobookshelfDownloadedEpisode {
                library_item_id: "show-a".into(),
                episode_id: "episode-b".into(),
                title: "Episode B".into(),
                published_at: None,
                duration_seconds: None,
            },
            mbv_core::audiobookshelf::AudiobookshelfDownloadedEpisode {
                library_item_id: "show-a".into(),
                episode_id: "episode-c".into(),
                title: "Episode C".into(),
                published_at: None,
                duration_seconds: None,
            },
        ]);
    app.audiobookshelf_browse[0].scroll = 6;

    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    assert_eq!(app.selection_modal.as_ref().unwrap().cursor, 1);
    app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    assert_eq!(app.selection_modal.as_ref().unwrap().cursor, 2);
    app.handle_key(KeyEvent::new(KeyCode::Char(']'), KeyModifiers::NONE));
    assert_eq!(
        app.selection_modal
            .as_ref()
            .unwrap()
            .filter
            .as_ref()
            .unwrap()
            .selected,
        1
    );
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
        let mut app = crate::app::tests_podcast::audiobookshelf_app();
        app.audiobookshelf_browse[0].scroll = 6;
        app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        app.handle_key(KeyEvent::new(key, KeyModifiers::NONE));
        assert!(app.selection_modal.is_none());
        assert_eq!(app.panel_focus, crate::app::PanelFocus::Library);
        assert_eq!(app.audiobookshelf_browse[0].cursor(), 0);
        assert_eq!(app.audiobookshelf_browse[0].scroll, 6);
    }
}

#[test]
fn podcast_modal_loading_and_empty_states_ignore_movement_and_activation() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut loading = crate::app::tests_podcast::audiobookshelf_app();
    loading.audiobookshelf_browse[0].episodes = None;
    loading.audiobookshelf_browse[0].detail_loading = true;
    loading.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert!(matches!(
        loading.selection_modal.as_ref().unwrap().state,
        SelectionModalListState::Loading
    ));
    loading.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    loading.handle_key(KeyEvent::new(KeyCode::Char(']'), KeyModifiers::NONE));
    loading.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert!(loading.selection_modal.is_none());

    let mut empty = crate::app::tests_podcast::audiobookshelf_app();
    empty.audiobookshelf_browse[0].episodes = Some(Vec::new());
    empty.audiobookshelf_browse[0].detail_loading = false;
    empty.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert!(matches!(
        empty.selection_modal.as_ref().unwrap().state,
        SelectionModalListState::Empty
    ));
    empty.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    empty.handle_key(KeyEvent::new(KeyCode::Char(']'), KeyModifiers::NONE));
    empty.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert!(empty.selection_modal.is_none());
}
