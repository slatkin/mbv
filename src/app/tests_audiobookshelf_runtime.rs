use super::*;
use crate::config::TestStateDirGuard;
use mbv_core::config::AudiobookshelfSetup;
use mbv_core::service_runtime::ServiceState;

fn user() -> mbv_core::audiobookshelf::AudiobookshelfUser {
    mbv_core::audiobookshelf::AudiobookshelfUser {
        id: "user-id".into(),
        username: "reader".into(),
    }
}

fn completion(
    generation: mbv_core::service_runtime::SetupGeneration,
    kind: super::service_startup::AudiobookshelfCompletionKind,
    result: Result<
        mbv_core::audiobookshelf::AudiobookshelfUser,
        mbv_core::audiobookshelf::AudiobookshelfError,
    >,
) -> super::service_startup::AudiobookshelfCompletion {
    super::service_startup::AudiobookshelfCompletion {
        generation,
        kind,
        result,
    }
}

#[test]
fn configured_startup_is_independent_and_reaches_ready() {
    let _guard = TestStateDirGuard::new();
    let config = crate::config::Config {
        audiobookshelf_setup: Some(AudiobookshelfSetup::new("https://books.example")),
        ..Default::default()
    };
    mbv_core::config::save_service_secret(
        mbv_core::config::ServiceKind::Audiobookshelf,
        "book-secret",
    )
    .unwrap();
    let mut app = App::new_independent(config);
    assert!(app.audiobookshelf_startup_request.is_some());
    let generation = app.audiobookshelf_runtime.generation();
    app.apply_audiobookshelf_completion(completion(
        generation,
        super::service_startup::AudiobookshelfCompletionKind::Startup,
        Ok(user()),
    ));
    assert_eq!(app.audiobookshelf_runtime.state, ServiceState::Ready);
    assert_eq!(app.audiobookshelf_runtime.user.unwrap().username, "reader");
}

#[test]
fn rejected_key_clears_only_secret_and_unavailable_retains_it() {
    let _guard = TestStateDirGuard::new();
    let mut app = tests::make_app_stub();
    app.config.lock().unwrap().audiobookshelf_setup =
        Some(AudiobookshelfSetup::new("https://books.example"));
    mbv_core::config::save_service_secret(
        mbv_core::config::ServiceKind::Audiobookshelf,
        "book-secret",
    )
    .unwrap();
    let generation = app.audiobookshelf_runtime.begin_validation();
    app.install_audiobookshelf_player_context(generation);
    assert!(app.player.can_admit_audiobookshelf());
    app.apply_audiobookshelf_completion(completion(
        generation,
        super::service_startup::AudiobookshelfCompletionKind::Startup,
        Err(mbv_core::audiobookshelf::AudiobookshelfError {
            class: mbv_core::audiobookshelf::AudiobookshelfFailureClass::AuthenticationRejected,
        }),
    ));
    assert_eq!(
        app.audiobookshelf_runtime.state,
        ServiceState::NeedsAuthentication
    );
    assert!(
        mbv_core::config::load_service_secret(mbv_core::config::ServiceKind::Audiobookshelf)
            .is_none()
    );
    assert!(app.config.lock().unwrap().audiobookshelf_setup.is_some());
    assert!(!app.player.can_admit_audiobookshelf());

    mbv_core::config::save_service_secret(
        mbv_core::config::ServiceKind::Audiobookshelf,
        "book-secret",
    )
    .unwrap();
    let generation = app.audiobookshelf_runtime.begin_validation();
    app.apply_audiobookshelf_completion(completion(
        generation,
        super::service_startup::AudiobookshelfCompletionKind::Startup,
        Err(mbv_core::audiobookshelf::AudiobookshelfError {
            class: mbv_core::audiobookshelf::AudiobookshelfFailureClass::Connectivity,
        }),
    ));
    assert_eq!(app.audiobookshelf_runtime.state, ServiceState::Unavailable);
    assert_eq!(
        mbv_core::config::load_service_secret(mbv_core::config::ServiceKind::Audiobookshelf)
            .as_deref(),
        Some("book-secret")
    );
}

#[test]
fn rejected_key_clears_owner_admission_but_preserves_repairable_queue_snapshot() {
    let _guard = TestStateDirGuard::new();
    let mut app = tests::make_app_stub();
    app.config.lock().unwrap().audiobookshelf_setup =
        Some(AudiobookshelfSetup::new("https://books.example"));
    mbv_core::config::save_service_secret(
        mbv_core::config::ServiceKind::Audiobookshelf,
        "book-secret",
    )
    .unwrap();
    let generation = app.audiobookshelf_runtime.begin_validation();
    app.install_audiobookshelf_player_context(generation);
    assert!(app.player.can_admit_audiobookshelf());

    let queue = mbv_core::config::QueueState {
        source: mbv_core::config::QueueSource::Unknown,
        items: vec![mbv_core::playback_queue::QueueItem::Audiobookshelf(
            mbv_core::playback_queue::AudiobookshelfQueueItem {
                library_item_id: "show-1".into(),
                episode_id: "episode-1".into(),
                title: "Episode 1".into(),
                show_title: None,
                author: None,
                duration_ticks: Some(100),
                position_ticks: 42,
                played: false,
                pub_date_secs: None,
                is_finished: false,
                cover_path: None,
            },
        )],
        cursor: 0,
        last_played_content_id: None,
        last_played_item_id: None,
        last_played_completed: false,
        positions: Default::default(),
    };
    mbv_core::config::save_queue_state(&queue).unwrap();
    app.player_tab.set_queue_items(queue.items.clone(), 0);

    app.clear_audiobookshelf_authentication().unwrap();

    assert!(!app.player.can_admit_audiobookshelf());
    assert!(app.config.lock().unwrap().audiobookshelf_setup.is_some());
    assert_eq!(app.player_tab.total_queue_len(), 1);
    assert_eq!(mbv_core::config::load_queue_state().unwrap().items.len(), 1);
}

#[test]
fn stale_progress_ack_after_authentication_clear_is_ignored() {
    let _guard = TestStateDirGuard::new();
    let mut app = tests::make_app_stub();
    let old_generation = app.audiobookshelf_runtime.generation();
    let queue = mbv_core::config::QueueState {
        source: mbv_core::config::QueueSource::Unknown,
        items: vec![mbv_core::playback_queue::QueueItem::Audiobookshelf(
            mbv_core::playback_queue::AudiobookshelfQueueItem {
                library_item_id: "show-1".into(),
                episode_id: "episode-1".into(),
                title: "Episode 1".into(),
                show_title: None,
                author: None,
                duration_ticks: Some(100),
                position_ticks: 42,
                played: false,
                pub_date_secs: None,
                is_finished: false,
                cover_path: None,
            },
        )],
        cursor: 0,
        last_played_content_id: None,
        last_played_item_id: None,
        last_played_completed: false,
        positions: Default::default(),
    };
    app.player_tab.set_queue_items(queue.items.clone(), 0);
    let before_position = app.player_tab.queue.slots()[0]
        .item
        .as_audiobookshelf()
        .unwrap()
        .position_ticks;
    let before_finished = app.player_tab.queue.slots()[0]
        .item
        .as_audiobookshelf()
        .unwrap()
        .is_finished;

    app.clear_audiobookshelf_authentication().unwrap();
    app.handle_lib_event(LibEvent::AudiobookshelfProgressAcknowledged(
        mbv_core::player::AudiobookshelfProgressUpdate {
            generation: old_generation,
            library_item_id: "show-1".into(),
            episode_id: "episode-1".into(),
            current_time_seconds: 120.0,
            duration_seconds: 100.0,
            is_finished: true,
        },
    ));

    let episode = app.player_tab.queue.slots()[0]
        .item
        .as_audiobookshelf()
        .unwrap();
    assert_eq!(episode.position_ticks, before_position);
    assert_eq!(episode.is_finished, before_finished);
    assert!(app.audiobookshelf_browse.is_empty());
}

#[test]
fn stale_completion_after_removal_cannot_mutate_runtime() {
    let mut app = tests::make_app_stub();
    let generation = app.audiobookshelf_runtime.begin_validation();
    app.audiobookshelf_runtime.remove_setup();
    app.apply_audiobookshelf_completion(completion(
        generation,
        super::service_startup::AudiobookshelfCompletionKind::Test,
        Ok(user()),
    ));
    assert_eq!(
        app.audiobookshelf_runtime.state,
        ServiceState::NotConfigured
    );
    assert!(app.audiobookshelf_runtime.user.is_none());
}

#[test]
fn test_success_reports_server_and_user_without_secret_details() {
    let _guard = TestStateDirGuard::new();
    let mut app = tests::make_app_stub();
    app.config.lock().unwrap().audiobookshelf_setup =
        Some(AudiobookshelfSetup::new("https://books.example"));
    let generation = app.audiobookshelf_runtime.begin_validation();
    app.apply_audiobookshelf_completion(completion(
        generation,
        super::service_startup::AudiobookshelfCompletionKind::Test,
        Ok(user()),
    ));
    assert_eq!(app.audiobookshelf_runtime.state, ServiceState::Ready);
    assert!(app.status.contains("https://books.example"));
    assert!(app.status.contains("reader"));
    assert!(!app.status.contains("Authorization"));
    assert!(!app.status.contains("book-secret"));
}

#[test]
fn context_updates_reach_suspended_local_owner_not_remote_proxy() {
    let _guard = TestStateDirGuard::new();
    let mut app = tests::make_app_stub();
    app.config.lock().unwrap().audiobookshelf_setup =
        Some(AudiobookshelfSetup::new("https://books.example"));
    mbv_core::config::save_service_secret(
        mbv_core::config::ServiceKind::Audiobookshelf,
        "book-secret",
    )
    .unwrap();
    let generation = app.audiobookshelf_runtime.begin_validation();
    app.install_audiobookshelf_player_context(generation);
    assert_eq!(app.player.audiobookshelf_generation(), Some(generation));

    let (remote, remote_rx) = mbv_core::remote_player::RemotePlayer::stub(Vec::new(), 0);
    app.switch_to_library_route(
        "podcasts",
        remote,
        remote_rx,
        &mbv_core::remote_player::DaemonEndpoint::Tcp("127.0.0.1:9".parse().unwrap()),
    );
    assert_eq!(app.player.audiobookshelf_generation(), None);
    assert_eq!(
        app.suspended_local
            .as_ref()
            .unwrap()
            .player
            .audiobookshelf_generation(),
        Some(generation)
    );

    app.clear_audiobookshelf_authentication().unwrap();
    assert_eq!(
        app.suspended_local
            .as_ref()
            .unwrap()
            .player
            .audiobookshelf_generation(),
        None
    );
}
