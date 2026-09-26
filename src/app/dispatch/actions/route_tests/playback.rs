use super::*;
use crate::app::QueueScope;

#[rstest]
#[case::ctrl_attached(true, false, true, &["Video"], PlaybackEligibility::WhollyUnplayable { unplayable_count: 1 })]
#[case::emby_session(true, false, true, &["Video", "Audio"], PlaybackEligibility::Mixed { unplayable_count: 1 })]
#[case::unknown_capability(true, false, false, &["Video"], PlaybackEligibility::Ineligible)]
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
    command_rx.try_recv().unwrap_err();
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
