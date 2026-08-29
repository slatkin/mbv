use super::*;
use mbv_core::api::TICKS_PER_SECOND;
use mbv_core::audiobookshelf_socket::AudiobookshelfProgress;
use mbv_core::audiobookshelf_socket::SocketEvent;
use mbv_core::playback_queue::QueueItem;
use mbv_core::player::PlayerEvent;

fn enable_audiobookshelf_owner(app: &App) {
    let context = mbv_core::player::AudiobookshelfPlayerContext::new(
        mbv_core::service_runtime::SetupGeneration::new(1),
        mbv_core::config::AudiobookshelfSetup::new("https://books.example"),
        "secret".into(),
        "device".into(),
    )
    .expect("valid test Audiobookshelf context");
    app.player.update_audiobookshelf_context(Some(context));
}

#[test]
fn audiobookshelf_play_selects_canonical_slot_and_submits_to_eligible_owner() {
    let mut app = super::tests_podcast::audiobookshelf_app();
    enable_audiobookshelf_owner(&app);
    app.player.status.lock().unwrap().active = true;
    let commands = app.player.spy_on_commands();

    app.play_selected_audiobookshelf_episode(0, 0, crate::app::types_audiobookshelf_browse::AudiobookshelfEpisodeFilter::All);

    assert_eq!(app.player_tab.total_queue_len(), 1);
    assert_eq!(app.player_tab.queue_cursor, 0);
    assert!(app.player_tab.queue.active_slot_id().is_some());
    match commands.recv().unwrap() {
        mbv_core::player::PlayerCommand::SubmitQueue { items, start_idx } => {
            assert_eq!(start_idx, 0);
            assert!(items[0].is_audiobookshelf());
        }
        _ => panic!("expected canonical play submission"),
    }
}

#[test]
fn audiobookshelf_enqueue_mutates_composed_queue_without_starting() {
    let mut app = super::tests_podcast::audiobookshelf_app();
    let commands = app.player.spy_on_commands();

    app.enqueue_selected_audiobookshelf_episode(0, 0, crate::app::types_audiobookshelf_browse::AudiobookshelfEpisodeFilter::All);

    assert_eq!(app.player_tab.total_queue_len(), 1);
    assert!(!app.player.status.lock().unwrap().active);
    assert!(
        commands.try_recv().is_err(),
        "Composed enqueue must not submit"
    );
}

#[test]
fn audiobookshelf_enqueue_mutates_eligible_bound_queue_without_starting() {
    let mut app = super::tests_podcast::audiobookshelf_app();
    enable_audiobookshelf_owner(&app);
    app.player.status.lock().unwrap().active = true;
    let commands = app.player.spy_on_commands();

    app.enqueue_selected_audiobookshelf_episode(0, 0, crate::app::types_audiobookshelf_browse::AudiobookshelfEpisodeFilter::All);

    assert_eq!(app.player_tab.total_queue_len(), 1);
    assert!(app.player.status.lock().unwrap().active);
    match commands.recv().unwrap() {
        mbv_core::player::PlayerCommand::QueueAppend { items } => {
            assert_eq!(items.len(), 1);
            assert!(items[0].is_audiobookshelf());
        }
        _ => panic!("expected canonical enqueue"),
    }
    assert!(commands.try_recv().is_err(), "enqueue must not submit/play");
}

#[test]
fn audiobookshelf_ordinary_actions_reject_unsupported_owner_without_side_effects() {
    let mut app = super::tests_podcast::audiobookshelf_app();
    let (remote, _events) = mbv_core::remote_player::RemotePlayer::stub(Vec::new(), 0);
    app.player = mbv_core::player::PlayerProxy::remote(remote, false);

    app.play_selected_audiobookshelf_episode(0, 0, crate::app::types_audiobookshelf_browse::AudiobookshelfEpisodeFilter::All);
    app.enqueue_selected_audiobookshelf_episode(0, 0, crate::app::types_audiobookshelf_browse::AudiobookshelfEpisodeFilter::All);

    assert_eq!(app.player_tab.total_queue_len(), 0);
}

#[test]
fn audiobookshelf_unavailable_episode_row_has_no_queue_or_playback_side_effects() {
    let mut app = super::tests_podcast::audiobookshelf_app();
    app.audiobookshelf_browse[0].episodes = Some(vec![
        mbv_core::audiobookshelf::AudiobookshelfDownloadedEpisode {
            library_item_id: "show-a".into(),
            episode_id: String::new(),
            title: "Unavailable".into(),
            published_at: None,
            duration_seconds: None,
        },
    ]);

    app.play_selected_audiobookshelf_episode(0, 0, crate::app::types_audiobookshelf_browse::AudiobookshelfEpisodeFilter::All);
    app.enqueue_selected_audiobookshelf_episode(0, 0, crate::app::types_audiobookshelf_browse::AudiobookshelfEpisodeFilter::All);

    assert_eq!(app.player_tab.total_queue_len(), 0);
    assert!(!app.player.status.lock().unwrap().active);
}

#[test]
fn audiobookshelf_progress_ack_updates_matching_queue_slots_and_browse_state() {
    let mut app = super::tests_podcast::audiobookshelf_app();
    app.audiobookshelf_browse[0].episodes = Some(vec![
        mbv_core::audiobookshelf::AudiobookshelfDownloadedEpisode {
            library_item_id: "show-a".into(),
            episode_id: "episode-a".into(),
            title: "Episode A".into(),
            published_at: None,
            duration_seconds: Some(120.0),
        },
        mbv_core::audiobookshelf::AudiobookshelfDownloadedEpisode {
            library_item_id: "show-a".into(),
            episode_id: "episode-b".into(),
            title: "Episode B".into(),
            published_at: None,
            duration_seconds: Some(120.0),
        },
    ]);
    enable_audiobookshelf_owner(&app);
    app.play_selected_audiobookshelf_episode(0, 0, crate::app::types_audiobookshelf_browse::AudiobookshelfEpisodeFilter::All);
    app.enqueue_selected_audiobookshelf_episode(0, 0, crate::app::types_audiobookshelf_browse::AudiobookshelfEpisodeFilter::All);
    app.enqueue_selected_audiobookshelf_episode(0, 1, crate::app::types_audiobookshelf_browse::AudiobookshelfEpisodeFilter::All);

    let generation = app.audiobookshelf_runtime.generation();
    let update = mbv_core::player::AudiobookshelfProgressUpdate {
        generation,
        library_item_id: "show-a".into(),
        episode_id: "episode-a".into(),
        current_time_seconds: 120.0,
        duration_seconds: 120.0,
        is_finished: true,
    };
    app.handle_lib_event(LibEvent::AudiobookshelfProgressAcknowledged(update));

    let matching = app
        .player_tab
        .queue
        .slots()
        .iter()
        .filter_map(|slot| slot.item.as_audiobookshelf())
        .filter(|episode| episode.library_item_id == "show-a" && episode.episode_id == "episode-a")
        .collect::<Vec<_>>();
    assert_eq!(matching.len(), 2);
    assert_eq!(
        matching
            .iter()
            .map(|episode| episode.is_finished)
            .collect::<Vec<_>>(),
        vec![true, true]
    );
    assert!(matching
        .iter()
        .all(|episode| episode.position_ticks
            == (120.0 * mbv_core::api::TICKS_PER_SECOND as f64) as i64));
    let progress = &app.audiobookshelf_browse[0].progress[&("show-a".into(), "episode-a".into())];
    assert_eq!(progress.current_time_seconds, 120.0);
    assert!(progress.is_finished);
    assert!(app.audiobookshelf_browse[0]
        .visible_episodes(super::types_audiobookshelf_browse::AudiobookshelfEpisodeFilter::Unplayed)
        .iter()
        .all(|episode| episode.episode_id != "episode-a"));
}

#[test]
fn audiobookshelf_progress_ack_updates_local_queue_when_remote_queue_is_target() {
    let mut app = super::tests_podcast::audiobookshelf_app();
    app.enqueue_selected_audiobookshelf_episode(0, 0, crate::app::types_audiobookshelf_browse::AudiobookshelfEpisodeFilter::All);

    let (remote, remote_rx) = mbv_core::remote_player::RemotePlayer::stub(Vec::new(), 0);
    app.switch_to_library_route(
        "podcasts",
        remote,
        remote_rx,
        &mbv_core::remote_player::DaemonEndpoint::Tcp("127.0.0.1:9".parse().unwrap()),
    );
    let remote_item = app.player_tab.all_queue_items()[0].clone();
    app.remote_player_tab
        .as_mut()
        .unwrap()
        .set_queue_items(vec![remote_item], 0);

    let generation = app.audiobookshelf_runtime.generation();
    app.handle_lib_event(LibEvent::AudiobookshelfProgressAcknowledged(
        mbv_core::player::AudiobookshelfProgressUpdate {
            generation,
            library_item_id: "show-a".into(),
            episode_id: "episode-a".into(),
            current_time_seconds: 42.5,
            duration_seconds: 120.0,
            is_finished: true,
        },
    ));

    let local_episode = app
        .player_tab
        .queue
        .slots()
        .iter()
        .find_map(|slot| slot.item.as_audiobookshelf())
        .expect("local queue episode");
    assert_eq!(
        local_episode.position_ticks,
        (42.5 * mbv_core::api::TICKS_PER_SECOND as f64) as i64
    );
    assert!(local_episode.is_finished);

    let remote_episode = app.remote_player_tab.as_ref().unwrap().queue.slots()[0]
        .item
        .as_audiobookshelf()
        .unwrap();
    assert_eq!(remote_episode.position_ticks, 0);
    assert!(!remote_episode.is_finished);

    let progress = &app.audiobookshelf_browse[0].progress[&("show-a".into(), "episode-a".into())];
    assert_eq!(progress.current_time_seconds, 42.5);
    assert!(progress.is_finished);
}

#[test]
fn stale_audiobookshelf_progress_ack_is_ignored_after_generation_advance() {
    let mut app = super::tests_podcast::audiobookshelf_app();
    enable_audiobookshelf_owner(&app);
    app.enqueue_selected_audiobookshelf_episode(0, 0, crate::app::types_audiobookshelf_browse::AudiobookshelfEpisodeFilter::All);
    let before_queue = app
        .player_tab
        .queue
        .slots()
        .iter()
        .filter_map(|slot| {
            slot.item.as_audiobookshelf().map(|episode| {
                (
                    episode.library_item_id.clone(),
                    episode.episode_id.clone(),
                    episode.position_ticks,
                    episode.is_finished,
                )
            })
        })
        .collect::<Vec<_>>();
    let before_progress = app.audiobookshelf_browse[0].progress.clone();
    let stale = app.audiobookshelf_runtime.generation();
    app.audiobookshelf_runtime.begin_validation();
    app.handle_lib_event(LibEvent::AudiobookshelfProgressAcknowledged(
        mbv_core::player::AudiobookshelfProgressUpdate {
            generation: stale,
            library_item_id: "show-a".into(),
            episode_id: "episode-a".into(),
            current_time_seconds: 42.5,
            duration_seconds: 120.0,
            is_finished: true,
        },
    ));

    let after_queue = app
        .player_tab
        .queue
        .slots()
        .iter()
        .filter_map(|slot| {
            slot.item.as_audiobookshelf().map(|episode| {
                (
                    episode.library_item_id.clone(),
                    episode.episode_id.clone(),
                    episode.position_ticks,
                    episode.is_finished,
                )
            })
        })
        .collect::<Vec<_>>();
    assert_eq!(after_queue, before_queue);
    assert_eq!(app.audiobookshelf_browse[0].progress, before_progress);
}

// Task 3.1(a)(b)(c): daemon route via PlayerEvent::AudiobookshelfProgress
// updates queue slots, browse progress map, and Unplayed filter.
#[test]
fn audiobookshelf_progress_via_daemon_route_updates_queue_and_browse() {
    let mut app = super::tests_podcast::audiobookshelf_app();
    app.audiobookshelf_browse[0].episodes = Some(vec![
        mbv_core::audiobookshelf::AudiobookshelfDownloadedEpisode {
            library_item_id: "show-a".into(),
            episode_id: "episode-a".into(),
            title: "Episode A".into(),
            published_at: None,
            duration_seconds: Some(120.0),
        },
        mbv_core::audiobookshelf::AudiobookshelfDownloadedEpisode {
            library_item_id: "show-a".into(),
            episode_id: "episode-b".into(),
            title: "Episode B".into(),
            published_at: None,
            duration_seconds: Some(120.0),
        },
    ]);
    enable_audiobookshelf_owner(&app);
    app.play_selected_audiobookshelf_episode(0, 0, crate::app::types_audiobookshelf_browse::AudiobookshelfEpisodeFilter::All);
    app.enqueue_selected_audiobookshelf_episode(0, 1, crate::app::types_audiobookshelf_browse::AudiobookshelfEpisodeFilter::All);

    let generation = app.audiobookshelf_runtime.generation();
    let position_ticks = (120.0 * mbv_core::api::TICKS_PER_SECOND as f64) as i64;

    // (a)(b)(c): completion via daemon route.
    app.handle_player_event(PlayerEvent::AudiobookshelfProgress(
        mbv_core::ctrl::AudiobookshelfProgressEvent {
            library_item_id: "show-a".into(),
            episode_id: "episode-a".into(),
            position_ticks,
            is_finished: true,
            setup_generation: generation.value(),
        },
    ));

    // (a) Matching queue slots reflect acknowledged position_ticks and is_finished.
    let matching: Vec<_> = app
        .player_tab
        .queue
        .slots()
        .iter()
        .filter_map(|slot| slot.item.as_audiobookshelf())
        .filter(|ep| ep.library_item_id == "show-a" && ep.episode_id == "episode-a")
        .collect();
    assert!(!matching.is_empty(), "must have at least one matching slot");
    assert!(
        matching.iter().all(|ep| ep.is_finished),
        "all matching slots must be marked finished"
    );
    assert!(
        matching
            .iter()
            .all(|ep| ep.position_ticks == position_ticks),
        "all matching slots must have the acknowledged position_ticks"
    );

    // (b) Browse progress map updated.
    let progress = &app.audiobookshelf_browse[0].progress[&("show-a".into(), "episode-a".into())];
    assert!(progress.is_finished);
    assert_eq!(progress.current_time_seconds, 120.0);

    // (c) Unplayed filter excludes the finished episode.
    assert!(
        app.audiobookshelf_browse[0]
            .visible_episodes(super::types_audiobookshelf_browse::AudiobookshelfEpisodeFilter::Unplayed)
            .iter()
            .all(|ep| ep.episode_id != "episode-a"),
        "finished episode must be excluded from Unplayed filter"
    );
}

// Task 3.4: Socket-merge tests — matching inactive/browsed, skipped
// active slot, unmatched episode, superseded generation.
//
// These test `handle_audiobookshelf_socket_event` with `SocketEvent::ProgressUpdated`
// directly, bypassing the network-connected socket thread.

/// Set up the app with a known socket generation and a matching
/// browse progress entry so the merge will recognise the episode.
fn make_socket_merge_ready_app() -> App {
    let mut app = super::tests_podcast::audiobookshelf_app();
    app.audiobookshelf_socket_generation = Some(app.audiobookshelf_runtime.generation());
    app.audiobookshelf_browse[0].progress.insert(
        ("show-a".into(), "episode-a".into()),
        mbv_core::audiobookshelf::AudiobookshelfProgress {
            library_item_id: "show-a".into(),
            episode_id: "episode-a".into(),
            current_time_seconds: 0.0,
            is_finished: false,
        },
    );
    app
}

#[test]
fn socket_progress_updates_matching_inactive_queued_episode() {
    let mut app = make_socket_merge_ready_app();
    // Enqueue the known episode as an inactive slot.
    app.enqueue_selected_audiobookshelf_episode(0, 0, crate::app::types_audiobookshelf_browse::AudiobookshelfEpisodeFilter::All);

    // Activate a different slot so episode-a is inactive.
    let other = QueueItem::Audiobookshelf(mbv_core::playback_queue::AudiobookshelfQueueItem {
        library_item_id: "show-b".into(),
        episode_id: "ep-b".into(),
        title: "Other".into(),
        show_title: None,
        author: None,
        description: None,
        duration_ticks: None,
        position_ticks: 0,
        played: false,
        pub_date_secs: None,
        is_finished: false,
        cover_path: None,
    });
    app.player_tab.queue.append(other);
    let other_slot = app.player_tab.queue.slots()[1].slot_id;
    let _ = app.player_tab.queue.set_active_slot(other_slot);

    assert!(app
        .playback_queue()
        .queue
        .active_slot()
        .and_then(|s| s.item.as_audiobookshelf())
        .is_some_and(|e| e.episode_id != "episode-a"));

    // Fire the socket progress event.
    app.handle_audiobookshelf_socket_event(SocketEvent::ProgressUpdated(AudiobookshelfProgress {
        library_item_id: "show-a".into(),
        episode_id: "episode-a".into(),
        current_time_seconds: 42.5,
        is_finished: true,
    }));

    let slot = app
        .player_tab
        .queue
        .slots()
        .iter()
        .find(|s| {
            s.item
                .as_audiobookshelf()
                .is_some_and(|e| e.episode_id == "episode-a")
        })
        .expect("episode-a slot");
    let episode = slot.item.as_audiobookshelf().unwrap();
    assert_eq!(
        episode.position_ticks,
        (42.5 * TICKS_PER_SECOND as f64) as i64
    );
    assert!(episode.is_finished);

    // Browse map updated.
    let progress = &app.audiobookshelf_browse[0].progress[&("show-a".into(), "episode-a".into())];
    assert_eq!(progress.current_time_seconds, 42.5);
    assert!(progress.is_finished);
}

#[test]
fn socket_progress_skips_active_slot() {
    let mut app = make_socket_merge_ready_app();
    app.enqueue_selected_audiobookshelf_episode(0, 0, crate::app::types_audiobookshelf_browse::AudiobookshelfEpisodeFilter::All);
    let slot_id = app.player_tab.queue.slots()[0].slot_id;
    let _ = app.player_tab.queue.set_active_slot(slot_id); // episode-a is active

    let before_progress = app.audiobookshelf_browse[0]
        .progress
        .get(&("show-a".to_string(), "episode-a".to_string()))
        .cloned();

    app.handle_audiobookshelf_socket_event(SocketEvent::ProgressUpdated(AudiobookshelfProgress {
        library_item_id: "show-a".into(),
        episode_id: "episode-a".into(),
        current_time_seconds: 99.0,
        is_finished: true,
    }));

    // Browse unchanged.
    assert_eq!(
        app.audiobookshelf_browse[0]
            .progress
            .get(&("show-a".to_string(), "episode-a".to_string(),)),
        before_progress.as_ref(),
        "active slot merge must not update browse progress"
    );

    // Queue position unchanged.
    let episode = app.player_tab.queue.slots()[0]
        .item
        .as_audiobookshelf()
        .unwrap();
    assert_eq!(episode.position_ticks, 0);
    assert!(!episode.is_finished);
}

#[test]
fn socket_progress_skips_unmatched_episode() {
    let mut app = make_socket_merge_ready_app();
    let before_browse = app.audiobookshelf_browse[0].progress.clone();

    app.handle_audiobookshelf_socket_event(SocketEvent::ProgressUpdated(AudiobookshelfProgress {
        library_item_id: "unknown-show".into(),
        episode_id: "unknown-ep".into(),
        current_time_seconds: 50.0,
        is_finished: true,
    }));

    // Browse unchanged — no entry added for the unknown episode.
    assert_eq!(
        app.audiobookshelf_browse[0].progress, before_browse,
        "unmatched episode must not add a browse entry"
    );
    assert!(app.player_tab.queue.slots().is_empty());
}

#[test]
fn socket_progress_skips_superseded_generation() {
    let mut app = make_socket_merge_ready_app();
    app.audiobookshelf_browse[0].progress.insert(
        ("show-a".into(), "episode-a".into()),
        mbv_core::audiobookshelf::AudiobookshelfProgress {
            library_item_id: "show-a".into(),
            episode_id: "episode-a".into(),
            current_time_seconds: 10.0,
            is_finished: false,
        },
    );
    let before_browse = app.audiobookshelf_browse[0].progress.clone();

    // Advance the runtime generation (simulating replace/remove).
    app.audiobookshelf_runtime.begin_validation();

    // Socket generation is stale now.
    app.handle_audiobookshelf_socket_event(SocketEvent::ProgressUpdated(AudiobookshelfProgress {
        library_item_id: "show-a".into(),
        episode_id: "episode-a".into(),
        current_time_seconds: 99.0,
        is_finished: true,
    }));

    assert_eq!(
        app.audiobookshelf_browse[0].progress, before_browse,
        "superseded generation must not apply progress"
    );
}

// Task 3.1(d): a no-match daemon-route event leaves the queue unchanged but
// still writes the browse progress map for the episode.
#[test]
fn audiobookshelf_progress_via_daemon_route_no_match_updates_browse_only() {
    let mut app = super::tests_podcast::audiobookshelf_app();
    enable_audiobookshelf_owner(&app);
    // Queue is intentionally empty — episode-b has no slot.

    let generation = app.audiobookshelf_runtime.generation();
    // 60 seconds in ticks.
    let position_ticks = (60.0 * mbv_core::api::TICKS_PER_SECOND as f64) as i64;
    let before_queue_len = app.player_tab.total_queue_len();

    app.handle_player_event(PlayerEvent::AudiobookshelfProgress(
        mbv_core::ctrl::AudiobookshelfProgressEvent {
            library_item_id: "show-a".into(),
            episode_id: "episode-b".into(), // not in queue
            position_ticks,
            is_finished: false,
            setup_generation: generation.value(),
        },
    ));

    // Queue unchanged — no matching slot.
    assert_eq!(
        app.player_tab.total_queue_len(),
        before_queue_len,
        "no-match event must not mutate the queue"
    );

    // Browse progress map updated even on no queue match.
    let progress = &app.audiobookshelf_browse[0].progress[&("show-a".into(), "episode-b".into())];
    assert_eq!(progress.current_time_seconds, 60.0);
    assert!(!progress.is_finished);
}
