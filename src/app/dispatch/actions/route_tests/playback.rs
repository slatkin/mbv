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
    app.local_view
        .adopt_source(mbv_queue::QueueSource::Playlist {
            id: Some("old-playlist".into()),
            name: "Old playlist".into(),
        });
    let mut item = make_item("Movie", "Movie");
    item.id = "movie-1".into();

    app.play_item(item);

    assert!(matches!(
        commands
            .try_iter()
            .find(|command| matches!(command, CtrlCmd::UnifiedQueueReplace { .. })),
        Some(CtrlCmd::UnifiedQueueReplace {
            source: mbv_queue::QueueSource::Unknown,
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

    assert!(app.queue_deferrals.has_local_play());
    assert!(app.local_view.emby_items().is_empty());
    command_rx.try_recv().unwrap_err();
}

fn make_home_audio_only_owner() -> (App, std::sync::mpsc::Receiver<CtrlCmd>) {
    let (remote, player_rx, commands) =
        mbv_remote_player::RemotePlayer::stub_audio_only_with_command_rx(Vec::new(), 0);
    let config = crate::config::Config {
        stay_alive: true,
        ..crate::config::Config::default()
    };
    let app = App::new_remote_with_config(
        mbv_emby::EmbyClient::new(config.clone()),
        remote,
        player_rx,
        &mbv_remote_player::DaemonEndpoint::Local,
        config,
    );
    (app, commands)
}

fn confirm_local_fall_through(app: &mut App) {
    app.apply_confirm_action(
        ConfirmAction::PlayLocallyInstead,
        crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Enter,
            crossterm::event::KeyModifiers::NONE,
        ),
    );
}

#[test]
fn confirmed_fall_through_keeps_the_current_home_link_connected() {
    let (mut app, home_commands) = make_home_audio_only_owner();
    app.connected_session_id = Some("controlled-session".into());
    let mut session = make_session("controlled-session", "Emby");
    session.media_info.audio_only = true;
    session.playable_media_types = vec!["Audio".into()];
    app.connected_session_state = Some(session);
    app.play_item(make_item("Movie", "Movie"));

    confirm_local_fall_through(&mut app);

    assert!(app.is_local_daemon());
    assert!(app.connected_session_id.is_none());
    assert!(
        !home_commands.try_iter().any(|command| matches!(
            command,
            CtrlCmd::PlaybackIntent(intent)
                if intent.action == mbv_ctrl::PlaybackIntentAction::Stop
        )),
        "fall-through must not stop the home link"
    );
}

#[test]
fn confirmed_fall_through_reinstates_a_suspended_home_link() {
    let (mut app, _) = make_home_audio_only_owner();
    let (remote, remote_rx, _) =
        mbv_remote_player::RemotePlayer::stub_audio_only_with_command_rx(Vec::new(), 0);
    let session = make_session("audio-owner", "mbv");
    let endpoint = mbv_remote_player::DaemonEndpoint::Tcp("127.0.0.1:0".parse().unwrap());
    app.switch_to_direct_remote(&session, remote, remote_rx, &endpoint);
    assert!(app.suspended_local.is_some());
    app.play_item(make_item("Movie", "Movie"));

    confirm_local_fall_through(&mut app);

    assert!(app.is_local_daemon());
    assert!(app.suspended_local.is_none());
}

fn connect_local_daemon(
    _: &mbv_remote_player::DaemonEndpoint,
) -> (
    mbv_remote_player::RemotePlayer,
    std::sync::mpsc::Receiver<mbv_ctrl::player::PlayerEvent>,
) {
    mbv_remote_player::RemotePlayer::stub(Vec::new(), 0)
}

#[test]
fn confirmed_local_fall_through_stops_and_detaches_remote_owner() {
    use crate::app::{DAEMON_ROUTE_CONNECT_OVERRIDE, DAEMON_ROUTE_CONNECT_TEST_LOCK};

    let (mut app, command_rx) =
        make_audio_only_remote_app_stub_with_cmd_rx(Vec::new(), make_items(1));
    let mut item = make_item("Movie", "Movie");
    item.id = "movie-1".into();
    app.play_item(item);

    let _guard = DAEMON_ROUTE_CONNECT_TEST_LOCK.lock().unwrap();
    *DAEMON_ROUTE_CONNECT_OVERRIDE.lock().unwrap() = Some(connect_local_daemon);
    confirm_local_fall_through(&mut app);
    *DAEMON_ROUTE_CONNECT_OVERRIDE.lock().unwrap() = None;

    assert!(!app.queue_deferrals.has_local_play());
    assert!(app.is_local_daemon());
    assert_eq!(app.queue_scope, QueueScope::Local);
    assert!(app.connected_session_id.is_none());
    let commands: Vec<_> = command_rx.try_iter().collect();
    assert!(commands.iter().any(|command| {
        matches!(
            command,
            mbv_ctrl::CtrlCmd::PlaybackIntent(intent)
                if intent.action == mbv_ctrl::PlaybackIntentAction::Stop
        )
    }));
    assert!(
        !commands
            .iter()
            .any(|command| { matches!(command, mbv_ctrl::CtrlCmd::UnifiedQueueReplace { .. }) })
    );
}

#[test]
fn wholly_playable_play_keeps_the_existing_play_path() {
    let (mut app, command_rx) =
        make_audio_only_remote_app_stub_with_cmd_rx(Vec::new(), make_items(1));
    let mut item = make_item("Song", "Audio");
    item.id = "song-1".into();
    item.media_type = "Audio".into();

    app.play_item(item.clone());

    assert!(!app.queue_deferrals.has_local_play());
    let slots = command_rx
        .try_iter()
        .find_map(|command| match command {
            mbv_ctrl::CtrlCmd::UnifiedQueueReplace { slots, .. } => Some(slots),
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

    assert!(!app.queue_deferrals.has_local_play());
    assert!(!app.queue_deferrals.is_save_deferred());
    assert!(app.pending_overlay.is_none());
    assert!(app.local_view.emby_items().is_empty());
    // The appended entry appears in the view only through the owner's
    // answer (row 5.3); this legacy stub owner sends no snapshot, so the
    // optimistic remote-tab content is not asserted here.
    assert!(command_rx.try_iter().any(|command| {
        matches!(
            command,
            mbv_ctrl::CtrlCmd::UnifiedQueueAppend { items, .. }
                if items.len() == 1 && items[0].id() == item.id
        )
    }));
}
