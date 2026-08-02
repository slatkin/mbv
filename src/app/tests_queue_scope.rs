use super::*;
use crate::app::tests::*;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use mbv_core::api::device_name;
use mbv_core::ctrl::{CtrlCmd, WireCommand};
use unicode_width::UnicodeWidthStr;

#[test]
fn stale_remote_queue_scope_falls_back_to_local_when_not_in_direct_remote_mode() {
    let mut app = make_app_stub();
    app.remote_player_tab = Some(PlayerTab::new(make_items(2), 1));
    app.queue_scope = QueueScope::Remote;

    assert_eq!(app.visible_queue_scope(), QueueScope::Local);

    app.set_queue_scope(QueueScope::Remote);
    assert_eq!(app.visible_queue_scope(), QueueScope::Local);
    assert_eq!(app.queue_scope, QueueScope::Local);
}

#[test]
fn queue_scope_resolution_matrix_without_remote_queue() {
    let mut app = make_app_stub();
    app.queue_scope = QueueScope::Local;

    assert!(!app.has_direct_remote_queue());
    assert_eq!(app.playback_target_queue_scope(), QueueScope::Local);
    assert_eq!(app.visible_queue_scope(), QueueScope::Local);
    assert!(app.local_queue_metadata_applies(QueueScope::Local));
    assert!(app.local_queue_metadata_applies(QueueScope::Remote));
}

#[test]
fn queue_scope_resolution_matrix_stale_remote_scope_without_direct_remote() {
    let mut app = make_app_stub();
    app.remote_player_tab = Some(PlayerTab::new(make_items(2), 0));
    app.queue_scope = QueueScope::Remote;

    assert!(!app.has_direct_remote_queue());
    assert_eq!(app.playback_target_queue_scope(), QueueScope::Local);
    assert_eq!(app.visible_queue_scope(), QueueScope::Local);
    assert!(app.local_queue_metadata_applies(QueueScope::Local));
    assert!(app.local_queue_metadata_applies(QueueScope::Remote));
}

#[test]
fn queue_scope_resolution_matrix_direct_remote_displaying_local() {
    let local_items = make_items(1);
    let remote_items = make_items(2);
    let mut app = make_remote_app_stub(local_items, remote_items);
    app.queue_scope = QueueScope::Local;

    assert!(app.has_direct_remote_queue());
    assert_eq!(app.playback_target_queue_scope(), QueueScope::Remote);
    assert_eq!(app.visible_queue_scope(), QueueScope::Local);
    assert!(app.local_queue_metadata_applies(QueueScope::Local));
    assert!(!app.local_queue_metadata_applies(QueueScope::Remote));
}

#[test]
fn queue_scope_resolution_matrix_direct_remote_displaying_remote() {
    let local_items = make_items(1);
    let remote_items = make_items(2);
    let mut app = make_remote_app_stub(local_items, remote_items);
    app.queue_scope = QueueScope::Remote;

    assert!(app.has_direct_remote_queue());
    assert_eq!(app.playback_target_queue_scope(), QueueScope::Remote);
    assert_eq!(app.visible_queue_scope(), QueueScope::Remote);
    assert!(app.local_queue_metadata_applies(QueueScope::Local));
    assert!(!app.local_queue_metadata_applies(QueueScope::Remote));
}

#[test]
fn power_queue_renders_scope_pills_and_hitboxes_for_direct_remote() {
    let mut app = make_remote_app_stub(make_items(1), make_items(2));
    app.panel_focus = PanelFocus::Library;
    app.set_queue_scope(QueueScope::Local);

    let rendered = render_app_to_string(&mut app, 90, 28);
    let device_name = device_name();
    let upper_device_name = device_name.to_uppercase();

    assert!(
        rendered.contains(&format!(" {} ", upper_device_name)),
        "expected power queue local/session pills to use the device name:\n{rendered}"
    );
    assert!(app.layout.main.queue_scope_local_area.width >= device_name.width() as u16);
    assert!(app.layout.main.queue_scope_remote_area.width >= device_name.width() as u16);
    assert!(app.layout.main.queue_scope_remote_area.x > app.layout.main.queue_scope_local_area.x);
}

#[test]
fn power_queue_scope_switch_via_keyboard_works_from_queue_focus() {
    let mut app = make_remote_app_stub(make_items(1), make_items(2));
    app.panel_focus = PanelFocus::Queue;
    app.set_queue_scope(QueueScope::Local);

    let handled = app.handle_key(KeyEvent::new(KeyCode::Char(']'), KeyModifiers::NONE));
    assert!(!handled);
    assert_eq!(app.visible_queue_scope(), QueueScope::Remote);

    let handled = app.handle_key(KeyEvent::new(KeyCode::Char('['), KeyModifiers::NONE));
    assert!(!handled);
    assert_eq!(app.visible_queue_scope(), QueueScope::Local);
}

#[test]
fn power_left_focus_brackets_do_not_switch_queue_scope() {
    let mut app = make_remote_app_stub(make_items(1), make_items(2));
    app.panel_focus = PanelFocus::Library;
    app.set_queue_scope(QueueScope::Local);

    let handled = app.handle_key(KeyEvent::new(KeyCode::Char(']'), KeyModifiers::NONE));

    assert!(!handled);
    assert_eq!(app.visible_queue_scope(), QueueScope::Local);
}

#[test]
fn power_queue_scope_switch_via_click_uses_rendered_hitboxes() {
    let mut app = make_remote_app_stub(make_items(1), make_items(2));
    app.panel_focus = PanelFocus::Library;
    app.set_queue_scope(QueueScope::Local);
    let _ = render_app_to_string(&mut app, 90, 28);

    let remote = app.layout.main.queue_scope_remote_area;
    app.handle_mouse(left_down(remote.x, remote.y));
    assert_eq!(app.visible_queue_scope(), QueueScope::Remote);

    let local = app.layout.main.queue_scope_local_area;
    app.handle_mouse(left_down(local.x, local.y));
    assert_eq!(app.visible_queue_scope(), QueueScope::Local);
}

#[test]
fn power_scope_keys_are_ignored_outside_queue_tab() {
    let mut app = make_remote_app_stub(make_items(1), make_items(2));
    app.set_queue_scope(QueueScope::Local);

    let handled = app.handle_key(KeyEvent::new(KeyCode::Char(']'), KeyModifiers::NONE));

    assert!(!handled);
    assert_eq!(app.visible_queue_scope(), QueueScope::Local);
}

#[test]
fn shift_resize_grows_from_queue_focus_and_persists_pref() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_remote_app_stub(make_items(1), make_items(2));
    app.panel_focus = PanelFocus::Queue;

    let handled = app.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::SHIFT));

    assert!(!handled);
    assert_eq!(app.status, "Queue column width: 45 cols");
    let prefs: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(crate::config::prefs_path()).expect("prefs written"),
    )
    .expect("prefs json");
    assert_eq!(prefs["queue_column_width"].as_u64(), Some(45));
}

#[test]
fn shift_resize_is_blocked_by_help_overlay() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_remote_app_stub(make_items(1), make_items(2));
    app.show_help = true;

    let handled = app.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::SHIFT));

    assert!(!handled);
    assert!(app.show_help);
    assert!(app.status.is_empty());
    assert!(!crate::config::prefs_path().exists());
}

#[test]
fn shift_resize_clamps_and_reports_minimum_and_maximum() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_remote_app_stub(make_items(1), make_items(2));

    let handled = app.handle_key(KeyEvent::new(KeyCode::Left, KeyModifiers::SHIFT));
    assert!(!handled);
    assert_eq!(
        app.status,
        "Queue column width already at minimum (40 cols)"
    );
    assert!(!crate::config::prefs_path().exists());

    assert!(!app.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::SHIFT)));
    assert_eq!(app.status, "Queue column width: 45 cols");

    assert!(!app.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::SHIFT)));
    assert_eq!(app.status, "Queue column width: 48 cols");

    assert!(!app.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::SHIFT)));
    assert_eq!(
        app.status,
        "Queue column width already at maximum (48 cols)"
    );

    let prefs: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(crate::config::prefs_path()).expect("prefs written"),
    )
    .expect("prefs json");
    assert_eq!(prefs["queue_column_width"].as_u64(), Some(48));
}

#[test]
fn render_normalizes_saved_left_width_and_updates_layout() {
    let _guard = crate::config::TestStateDirGuard::new();
    let prefs = serde_json::json!({
        "queue_column_width": 70,
    });
    std::fs::write(
        crate::config::prefs_path(),
        serde_json::to_string(&prefs).expect("prefs json"),
    )
    .expect("write prefs");

    let mut app = make_remote_app_stub(make_items(1), make_items(2));

    let _ = render_app_to_string(&mut app, 70, 28);

    assert_eq!(app.layout.main.queue_area.width, 38);
    let saved: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(crate::config::prefs_path()).expect("prefs written"),
    )
    .expect("prefs json");
    assert_eq!(saved["queue_column_width"].as_u64(), Some(42));
}

#[test]
fn render_uses_resized_width_on_next_frame() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_remote_app_stub(make_items(1), make_items(2));

    let _ = render_app_to_string(&mut app, 100, 28);
    assert_eq!(app.layout.main.queue_area.width, 36);

    assert!(!app.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::SHIFT)));
    assert!(!app.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::SHIFT)));

    let _ = render_app_to_string(&mut app, 100, 28);
    assert_eq!(app.layout.main.queue_area.width, 46);
}

#[test]
fn local_daemon_queue_has_no_scope_affordance_or_remote_switch() {
    let mut app = make_local_daemon_app_stub(make_items(2));
    let rendered = render_app_to_string(&mut app, 90, 24);

    assert!(!app.has_direct_remote_queue());
    assert_eq!(
        app.layout.main.queue_scope_local_area,
        ratatui::layout::Rect::default()
    );
    assert_eq!(
        app.layout.main.queue_scope_remote_area,
        ratatui::layout::Rect::default()
    );
    assert!(
        !rendered.contains(" Local ") && !rendered.contains(" Remote "),
        "local-daemon queue should not render split-scope pills:\n{rendered}"
    );

    let handled = app.handle_key(KeyEvent::new(KeyCode::Char(']'), KeyModifiers::NONE));
    assert!(!handled);
    assert_eq!(app.visible_queue_scope(), QueueScope::Local);
}

#[test]
fn direct_remote_play_items_keeps_local_queue_intact() {
    let _guard = crate::config::TestStateDirGuard::new();
    let local_items = make_items(2);
    let remote_items = make_items(3);
    let replacement = make_items(4);
    let mut app = make_remote_app_stub(local_items.clone(), remote_items.clone());
    app.queue_source = crate::config::QueueSource::Album;

    app.execute_pending_queue_action(PendingQueueAction::PlayItems {
        items: replacement.clone(),
        start_idx: 2,
        source: crate::config::QueueSource::Shuffle,
    });

    assert_eq!(
        app.player_tab
            .items
            .iter()
            .map(|i| i.id.as_str())
            .collect::<Vec<_>>(),
        local_items
            .iter()
            .map(|i| i.id.as_str())
            .collect::<Vec<_>>()
    );
    assert_eq!(app.player_tab.queue_cursor, 0);
    assert_eq!(
        app.remote_player_tab
            .as_ref()
            .unwrap()
            .items
            .iter()
            .map(|i| i.id.as_str())
            .collect::<Vec<_>>(),
        remote_items
            .iter()
            .map(|i| i.id.as_str())
            .collect::<Vec<_>>()
    );
    assert_eq!(app.remote_player_tab.as_ref().unwrap().queue_cursor, 0);
    assert!(matches!(
        app.queue_source,
        crate::config::QueueSource::Album
    ));
    assert_eq!(app.visible_queue_scope(), QueueScope::Remote);
}

#[test]
fn clearing_a_local_daemon_queue_replaces_the_daemon_queue_with_empty() {
    let _guard = crate::config::TestStateDirGuard::new();
    let (remote, player_rx, cmd_rx) =
        mbv_core::remote_player::RemotePlayer::stub_with_command_rx(make_items(2), 0);
    let mut app = App::new_remote(
        mbv_core::api::EmbyClient::new(crate::config::Config::default()),
        remote,
        player_rx,
        mbv_core::remote_player::DaemonEndpoint::Local,
    );

    app.execute_pending_queue_action(PendingQueueAction::ClearQueue);

    assert!(app.player_tab.items.is_empty());
    assert!(cmd_rx.try_iter().any(|cmd| {
        matches!(
            cmd,
            CtrlCmd::PlayerCmd(WireCommand::ReplaceQueue { items, start_idx: 0 })
                if items.is_empty()
        )
    }));
}

#[test]
fn direct_remote_track_changes_do_not_clobber_local_last_played() {
    let _guard = crate::config::TestStateDirGuard::new();
    let local_items = make_items(2);
    let remote_items = make_items(3);
    let mut app = make_remote_app_stub(local_items.clone(), remote_items);
    app.last_played_item_id = Some(local_items[1].id.clone());
    app.last_played_completed = true;

    app.handle_player_event(PlayerEvent::TrackChanged(2));

    assert_eq!(
        app.last_played_item_id.as_deref(),
        Some(local_items[1].id.as_str())
    );
    assert!(app.last_played_completed);
}

#[test]
fn command_rejected_flashes_the_daemon_supplied_reason() {
    let mut app = make_app_stub();

    app.handle_player_event(PlayerEvent::CommandRejected(
        "Daemon is running in audio-only mode; can't play video items".to_string(),
    ));

    assert_eq!(
        app.status,
        "Daemon is running in audio-only mode; can't play video items"
    );
}
