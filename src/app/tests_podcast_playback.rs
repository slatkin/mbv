use super::*;

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
    app.enter_audiobookshelf_episode_selection();
    enable_audiobookshelf_owner(&app);
    app.player.status.lock().unwrap().active = true;
    let commands = app.player.spy_on_commands();

    app.play_selected_audiobookshelf_episode(0);

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
    app.enter_audiobookshelf_episode_selection();
    let commands = app.player.spy_on_commands();

    app.enqueue_selected_audiobookshelf_episode(0);

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
    app.enter_audiobookshelf_episode_selection();
    enable_audiobookshelf_owner(&app);
    app.player.status.lock().unwrap().active = true;
    let commands = app.player.spy_on_commands();

    app.enqueue_selected_audiobookshelf_episode(0);

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
    app.enter_audiobookshelf_episode_selection();
    let (remote, _events) = mbv_core::remote_player::RemotePlayer::stub(Vec::new(), 0);
    app.player = mbv_core::player::PlayerProxy::remote(remote, false);

    app.play_selected_audiobookshelf_episode(0);
    app.enqueue_selected_audiobookshelf_episode(0);

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
    app.enter_audiobookshelf_episode_selection();

    app.play_selected_audiobookshelf_episode(0);
    app.enqueue_selected_audiobookshelf_episode(0);

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
    app.enter_audiobookshelf_episode_selection();
    enable_audiobookshelf_owner(&app);
    app.play_selected_audiobookshelf_episode(0);
    app.enqueue_selected_audiobookshelf_episode(0);
    app.audiobookshelf_browse[0].episode_selection = Some(1);
    app.enqueue_selected_audiobookshelf_episode(0);

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
    app.audiobookshelf_browse[0].set_episode_filter(
        super::types_audiobookshelf_browse::AudiobookshelfEpisodeFilter::Unplayed,
    );
    assert!(app.audiobookshelf_browse[0]
        .visible_episodes()
        .iter()
        .all(|episode| episode.episode_id != "episode-a"));
}

#[test]
fn audiobookshelf_progress_ack_updates_local_queue_when_remote_queue_is_target() {
    let mut app = super::tests_podcast::audiobookshelf_app();
    app.enter_audiobookshelf_episode_selection();
    app.enqueue_selected_audiobookshelf_episode(0);

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
    app.enter_audiobookshelf_episode_selection();
    app.enqueue_selected_audiobookshelf_episode(0);
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
