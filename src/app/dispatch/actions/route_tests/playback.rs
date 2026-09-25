use super::*;
use crate::app::QueueScope;

#[rstest]
#[case::ctrl_attached(true, false, true, &["Video"], PlaybackEligibility::WhollyUnplayable { unplayable_count: 1 })]
#[case::emby_session(true, false, true, &["Video", "Audio"], PlaybackEligibility::Mixed { unplayable_count: 1 })]
#[case::library_route(true, true, true, &["Video"], PlaybackEligibility::Ineligible)]
#[case::unknown_capability(true, false, false, &["Video"], PlaybackEligibility::Ineligible)]
#[case::wholly_unplayable(true, false, true, &["Video", "Photo"], PlaybackEligibility::WhollyUnplayable { unplayable_count: 2 })]
#[case::mixed(true, false, true, &["Audio", "Video"], PlaybackEligibility::Mixed { unplayable_count: 1 })]
#[case::wholly_playable(true, false, true, &["Audio", "Audio"], PlaybackEligibility::WhollyPlayable)]
fn playback_eligibility_classifies_owner_and_selection(
    #[case] attached: bool,
    #[case] library_route: bool,
    #[case] owner_is_audio_only: bool,
    #[case] media_types: &[&str],
    #[case] expected: PlaybackEligibility,
) {
    assert_eq!(
        super::classify_playback_eligibility(
            attached,
            library_route,
            owner_is_audio_only,
            &selection(media_types),
        ),
        expected
    );
}

#[test]
fn stay_alive_single_item_play_does_not_reuse_the_previous_queue_source() {
    let _guard = crate::config::TestStateDirGuard::new();
    let (mut app, commands) =
        crate::app::tests::make_local_daemon_app_stub_with_cmd_rx(make_items(1));
    app.queue_source = crate::config::QueueSource::Playlist {
        id: Some("old-playlist".into()),
        name: "Old playlist".into(),
    };
    let mut item = make_item("Movie", "Movie");
    item.id = "movie-1".into();

    app.play_item(item);

    assert!(matches!(
        commands
            .try_iter()
            .find(|command| matches!(command, CtrlCmd::UnifiedQueueReplace { .. })),
        Some(CtrlCmd::UnifiedQueueReplace {
            source: crate::config::QueueSource::Unknown,
            ..
        })
    ));
}

#[test]
fn wholly_unplayable_play_is_deferred_before_mutating_local_state() {
    let (mut app, command_rx) =
        make_audio_only_remote_app_stub_with_cmd_rx(Vec::new(), make_items(1));
    let mut item = make_item("Movie", "Movie");
    item.id = "movie-1".into();

    app.play_item(item.clone());

    assert!(matches!(
        app.pending_local_play,
        Some(PendingQueueAction::PlayItems {
            items,
            start_idx: 0,
            autostart: true,
            ..
        }) if items.len() == 1 && items[0].id == item.id
    ));
    assert!(app.player_tab.emby_items().is_empty());
    assert!(command_rx.try_recv().is_err());
}

#[test]
fn wholly_unplayable_series_play_defers_the_expanded_series() {
    let mut app = make_app_stub();
    let http = MockHttp::new();
    let mut config = app.config.lock().unwrap().clone();
    config.server_url = "http://127.0.0.1:1".into();
    install_test_emby(&mut app, config);
    let client = app
        .emby_runtime
        .client
        .as_ref()
        .unwrap()
        .lock()
        .unwrap()
        .clone()
        .with_test_agent(http.agent());
    app.emby_runtime = mbv_core::service_runtime::EmbyRuntime::ready(std::sync::Arc::new(
        std::sync::Mutex::new(client),
    ));

    let (remote, remote_rx, command_rx) =
        mbv_core::remote_player::RemotePlayer::stub_audio_only_with_command_rx(Vec::new(), 0);
    let sess = crate::app::tests::make_session("remote-mbv", "mbv");
    app.switch_to_direct_remote(
        &sess,
        remote,
        remote_rx,
        &mbv_core::remote_player::DaemonEndpoint::Tcp("127.0.0.1:0".parse().unwrap()),
    );
    while command_rx.try_recv().is_ok() {}
    app.player.always_play_next = true;

    http.respond(
        200,
        r#"{"Items":[
            {"Id":"episode-1","Name":"Episode 1","Type":"Episode","MediaType":"Video"},
            {"Id":"episode-2","Name":"Episode 2","Type":"Episode","MediaType":"Video"}
        ]}"#,
    );
    let mut selected = make_item("Episode 1", "Episode");
    selected.id = "episode-1".into();
    selected.series_id = "series-1".into();
    app.play_item(selected);

    assert!(matches!(
        app.pending_local_play,
        Some(PendingQueueAction::PlayItems {
            items,
            start_idx: 0,
            source: crate::config::QueueSource::Series,
            autostart: true,
        }) if items.len() == 2
            && items.iter().map(|item| item.id.as_str()).collect::<Vec<_>>()
                == ["episode-1", "episode-2"]
    ));
    assert!(
        command_rx.try_recv().is_err(),
        "nothing is submitted while the prompt is open"
    );
}

fn fail_local_player_preparation() -> Result<(), String> {
    Err("test preparation failure".into())
}

#[test]
fn local_preparation_restores_a_suspended_player_before_detach() {
    let mut app = make_app_stub();
    let (remote, player_rx) = mbv_core::remote_player::RemotePlayer::stub(Vec::new(), 0);
    let session = make_session("audio-owner", "mbv");
    let endpoint = mbv_core::remote_player::DaemonEndpoint::Tcp("127.0.0.1:0".parse().unwrap());
    app.switch_to_direct_remote(&session, remote, player_rx, &endpoint);
    assert!(app.suspended_local.is_some());

    let prepared = app.prepare_local_player().unwrap().unwrap();

    assert!(!prepared.player.is_remote());
    assert!(app.player.is_remote());
    assert!(app.suspended_local.is_none());
}

#[test]
fn local_preparation_constructs_without_an_existing_local_player() {
    let (mut app, _) = make_audio_only_remote_app_stub_with_cmd_rx(Vec::new(), make_items(1));

    let prepared = app.prepare_local_player().unwrap().unwrap();

    assert!(!prepared.player.is_remote());
    assert!(app.player.is_remote());
}

#[test]
fn failed_local_preparation_leaves_attachment_unchanged() {
    let (mut app, command_rx) =
        make_audio_only_remote_app_stub_with_cmd_rx(Vec::new(), make_items(1));
    *crate::app::LOCAL_PLAYER_PREPARE_OVERRIDE.lock().unwrap() =
        Some(fail_local_player_preparation);
    let result = app.prepare_local_player();
    *crate::app::LOCAL_PLAYER_PREPARE_OVERRIDE.lock().unwrap() = None;

    assert!(matches!(result, Err(ref error) if error == "test preparation failure"));
    assert!(app.player.is_remote());
    assert!(app.remote_player_tab.is_some());
    assert!(command_rx.try_recv().is_err());
}

#[test]
fn declined_local_fall_through_clears_only_the_pending_play() {
    let (mut app, command_rx) =
        make_audio_only_remote_app_stub_with_cmd_rx(Vec::new(), make_items(1));
    app.set_queue_scope(QueueScope::Remote);
    app.pending_local_play = Some(PendingQueueAction::PlayItems {
        items: selection(&["Video"]),
        start_idx: 0,
        source: crate::config::QueueSource::Album,
        autostart: true,
    });
    let status = app.status.clone();

    app.apply_confirm_action(
        ConfirmAction::PlayLocallyInstead,
        crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Esc,
            crossterm::event::KeyModifiers::NONE,
        ),
    );

    assert!(app.pending_local_play.is_none());
    assert!(app.player.is_remote());
    assert_eq!(app.queue_scope, QueueScope::Remote);
    assert_eq!(app.status, status);
    assert!(command_rx.try_recv().is_err());
}

#[test]
fn confirmed_local_fall_through_stops_and_detaches_remote_owner() {
    let (mut app, command_rx) =
        make_audio_only_remote_app_stub_with_cmd_rx(Vec::new(), make_items(1));
    let mut item = make_item("Movie", "Movie");
    item.id = "movie-1".into();
    app.play_item(item);

    app.apply_confirm_action(
        ConfirmAction::PlayLocallyInstead,
        crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Char('y'),
            crossterm::event::KeyModifiers::NONE,
        ),
    );

    assert!(app.pending_local_play.is_none());
    assert!(!app.player.is_remote());
    assert_eq!(app.queue_scope, QueueScope::Local);
    assert!(app.connected_session_id.is_none());
    let commands: Vec<_> = command_rx.try_iter().collect();
    assert!(commands.iter().any(|command| {
        matches!(
            command,
            mbv_core::ctrl::CtrlCmd::PlaybackIntent(intent)
                if intent.action == mbv_core::ctrl::PlaybackIntentAction::Stop
        )
    }));
    assert!(!commands
        .iter()
        .any(|command| { matches!(command, mbv_core::ctrl::CtrlCmd::UnifiedQueueReplace { .. }) }));
}

#[test]
fn fall_through_from_a_session_stops_a_home_daemon_owner_too() {
    // A home local-daemon thin client controlling an audio-only Emby session
    // has `player.is_remote()` true while `connected_session_id` is set; the
    // fall-through must stop and detach both, or the daemon keeps playing
    // underneath the local item and its ctrl proxy leaks.
    let (mut app, command_rx) =
        make_audio_only_remote_app_stub_with_cmd_rx(Vec::new(), make_items(1));
    assert!(app.player.is_remote());
    let session = make_session("audio-owner", "mbv");
    app.connect_to_session(&session);
    assert!(app.connected_session_id.is_some());
    assert!(app.player.is_remote());

    app.pending_local_play = Some(PendingQueueAction::PlayItems {
        items: selection(&["Video"]),
        start_idx: 0,
        source: crate::config::QueueSource::Album,
        autostart: true,
    });
    app.play_pending_local_play();

    assert!(app.pending_local_play.is_none());
    assert!(!app.player.is_remote());
    assert!(app.connected_session_id.is_none());
    assert!(app.player_endpoint.is_none());
    let commands: Vec<_> = command_rx.try_iter().collect();
    assert!(commands.iter().any(|command| {
        matches!(
            command,
            mbv_core::ctrl::CtrlCmd::PlaybackIntent(intent)
                if intent.action == mbv_core::ctrl::PlaybackIntentAction::Stop
        )
    }));
}

#[test]
fn mixed_play_submits_unchanged_and_reports_unplayable_count() {
    let (mut app, command_rx) =
        make_audio_only_remote_app_stub_with_cmd_rx(Vec::new(), make_items(1));
    let items = selection(&["Audio", "Video"]);
    app.replace_playback_queue(items.clone(), 0);

    app.play_items_routed(items.clone(), 0, crate::config::QueueSource::Album);

    assert!(app.pending_local_play.is_none());
    assert_eq!(
        app.status_severity,
        crate::app::dispatch::notify::ToastSeverity::Neutral
    );
    assert!(app.status.contains("1 item"));
    assert!(app.status.contains("unavailable"));
    assert!(!app.status.contains("Playback started"));
    let slots = command_rx
        .try_iter()
        .find_map(|command| match command {
            mbv_core::ctrl::CtrlCmd::UnifiedQueueReplace { slots, .. } => Some(slots),
            _ => None,
        })
        .expect("mixed play should submit the unchanged selection");
    assert_eq!(
        slots.iter().map(|slot| slot.item.id()).collect::<Vec<_>>(),
        items
            .iter()
            .map(|item| item.id.as_str())
            .collect::<Vec<_>>()
    );
}

#[test]
fn routed_wholly_unplayable_play_is_deferred_without_queue_replacement() {
    let mut app = make_app_stub();
    app.connected_session_id = Some("session-1".into());
    let mut session = make_session("audio-owner", "Emby");
    session.playable_media_types = vec!["Audio".into()];
    app.connected_session_state = Some(session);
    let items = selection(&["Video", "Photo"]);

    app.play_items_routed(items, 1, crate::config::QueueSource::Album);

    assert!(matches!(
        app.pending_local_play,
        Some(PendingQueueAction::PlayItems {
            start_idx: 1,
            source: crate::config::QueueSource::Album,
            autostart: true,
            ..
        })
    ));
    assert!(app.player_tab.emby_items().is_empty());
    assert!(matches!(
        app.pending_overlay,
        Some(crate::app::state::types::overlay::OverlayRequest::Confirm(ref modal))
            if modal.message.contains("item-1")
                && modal.title.contains("audio-owner")
    ));
}

#[test]
fn wholly_playable_play_keeps_the_existing_play_path() {
    let (mut app, command_rx) =
        make_audio_only_remote_app_stub_with_cmd_rx(Vec::new(), make_items(1));
    let mut item = make_item("Song", "Audio");
    item.id = "song-1".into();
    item.media_type = "Audio".into();

    app.play_item(item.clone());

    assert!(app.pending_local_play.is_none());
    let slots = command_rx
        .try_iter()
        .find_map(|command| match command {
            mbv_core::ctrl::CtrlCmd::UnifiedQueueReplace { slots, .. } => Some(slots),
            _ => None,
        })
        .expect("playable play should submit a queue replacement");
    assert_eq!(slots.len(), 1);
    assert_eq!(slots[0].item.id(), item.id);
}

#[test]
fn enqueue_unplayable_selection_keeps_append_submission_without_prompt() {
    let (mut app, command_rx) =
        make_audio_only_remote_app_stub_with_cmd_rx(Vec::new(), make_items(1));
    app.panel_focus = PanelFocus::Queue;
    let mut item = make_item("Movie", "Movie");
    item.id = "movie-1".into();

    app.execute_context_action(
        Some(ContextAction::EnqueueSelection(vec![item.clone()])),
        None,
    );

    assert!(app.pending_local_play.is_none());
    assert!(app.pending_queue_action.is_none());
    assert!(app.pending_overlay.is_none());
    assert!(app.player_tab.emby_items().is_empty());
    assert!(app
        .remote_player_tab
        .as_ref()
        .unwrap()
        .emby_items()
        .iter()
        .any(|queued| queued.id == item.id));
    assert!(command_rx.try_iter().any(|command| {
        matches!(
            command,
            mbv_core::ctrl::CtrlCmd::UnifiedQueueAppend { items }
                if items.len() == 1 && items[0].id() == item.id
        )
    }));
}
