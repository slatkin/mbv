use super::*;
use crate::app::tests::make_app_stub;

fn key(code: KeyCode) -> KeyChord {
    KeyChord::new(code, KeyModifiers::NONE)
}

fn key_ctrl(code: KeyCode) -> KeyChord {
    KeyChord::new(code, KeyModifiers::CONTROL)
}

// ── PLAYBACK_HELP_BINDINGS stays truthful to playback_command_for_key ───

/// Characterization test: replays every `PLAYBACK_HELP_BINDINGS` sample
/// chord (all of them, not just one side of a paired display entry like
/// `< / >`) through the real `playback_command_for_key` and asserts each
/// resolves to the command the help table claims — for `gated` entries,
/// only when gated open, and never resolving to *some other* command
/// when gated closed. This is what keeps the help overlay's `[playback]`
/// section from silently drifting off the real bindings (issue #133).
#[test]
fn playback_help_bindings_match_playback_command_for_key() {
    for binding in PLAYBACK_HELP_BINDINGS {
        for (sample, command) in binding.samples {
            if binding.gated {
                assert_eq!(
                    playback_command_for_key(*sample, true, false),
                    Some(command.clone()),
                    "keys={:?} label={:?} sample={:?} should fire when active",
                    binding.keys,
                    binding.label,
                    sample
                );
                assert_eq!(
                    playback_command_for_key(*sample, false, true),
                    Some(command.clone()),
                    "keys={:?} label={:?} sample={:?} should fire on a remote session",
                    binding.keys,
                    binding.label,
                    sample
                );
                assert_eq!(
                    playback_command_for_key(*sample, false, false),
                    None,
                    "keys={:?} label={:?} sample={:?} should not fire when ungated",
                    binding.keys,
                    binding.label,
                    sample
                );
            } else {
                assert_eq!(
                    playback_command_for_key(*sample, false, false),
                    Some(command.clone()),
                    "keys={:?} label={:?} sample={:?} should fire unconditionally",
                    binding.keys,
                    binding.label,
                    sample
                );
            }
        }
    }
}

// ── playback_command_for_key: gated on (active OR has_remote_session) ────

#[test]
fn enter_never_stops() {
    assert_eq!(
        playback_command_for_key(key(KeyCode::Enter), true, true),
        None
    );
    assert_eq!(
        playback_command_for_key(key(KeyCode::Enter), false, false),
        None
    );
}

/// Assert that `code` produces `expected` for every (active, has_remote_session)
/// combination — i.e. it fires unconditionally, with no gating at all.
fn assert_fires_unconditionally(code: KeyCode, expected: Command) {
    for active in [false, true] {
        for remote in [false, true] {
            assert_eq!(
                playback_command_for_key(key(code), active, remote),
                Some(expected.clone()),
                "code={code:?} active={active} remote={remote}"
            );
        }
    }
}

// ── `z`: unconditional, no `active` gate in either branch ───────────────

#[test]
fn z_fires_unconditionally() {
    assert_fires_unconditionally(KeyCode::Char('z'), Command::CycleOrToggleSubtitle);
}

#[test]
fn ctrl_z_does_not_fire() {
    assert_eq!(
        playback_command_for_key(key_ctrl(KeyCode::Char('z')), true, true),
        None
    );
}

// ── `m`: unconditional, no session check at all (the flagged bug) ──────

#[test]
fn m_fires_unconditionally() {
    assert_fires_unconditionally(KeyCode::Char('m'), Command::ToggleMute);
}

// ── `-`/`+`: unconditional volume ────────────────────────────────────────

#[test]
fn volume_keys_fire_unconditionally() {
    assert_fires_unconditionally(KeyCode::Char('-'), Command::AdjustVolume(-5));
    assert_fires_unconditionally(KeyCode::Char('+'), Command::AdjustVolume(5));
    assert_fires_unconditionally(KeyCode::Char('='), Command::AdjustVolume(5));
}

// ── `a`: gated on (active OR has_remote_session), same as the other
// transport keys -- see #88 (previously `active` only, no remote path).

#[test]
fn a_fires_when_active_only() {
    assert_eq!(
        playback_command_for_key(key(KeyCode::Char('a')), true, false),
        Some(Command::ToggleMuteOrCycleAudio)
    );
}

#[test]
fn a_fires_when_remote_session_only() {
    assert_eq!(
        playback_command_for_key(key(KeyCode::Char('a')), false, true),
        Some(Command::ToggleMuteOrCycleAudio)
    );
}

#[test]
fn a_does_not_fire_when_neither_active_nor_remote() {
    assert_eq!(
        playback_command_for_key(key(KeyCode::Char('a')), false, false),
        None
    );
}

#[test]
fn ctrl_a_does_not_fire() {
    assert_eq!(
        playback_command_for_key(key_ctrl(KeyCode::Char('a')), true, true),
        None
    );
}

#[test]
fn unrelated_key_does_not_fire() {
    assert_eq!(
        playback_command_for_key(key(KeyCode::Char('q')), true, true),
        None
    );
}

// ── help_command_for_key: no gating (caller already checked show_help) ───

#[test]
fn help_q_fires_quit() {
    assert_eq!(
        help_command_for_key(key(KeyCode::Char('q'))),
        Some(Command::Quit)
    );
}

#[test]
fn help_ctrl_q_does_not_fire() {
    assert_eq!(help_command_for_key(key_ctrl(KeyCode::Char('q'))), None);
}

#[test]
fn help_esc_fires_close_help() {
    assert_eq!(
        help_command_for_key(key(KeyCode::Esc)),
        Some(Command::CloseHelp)
    );
}

#[test]
fn help_f1_fires_close_help() {
    assert_eq!(
        help_command_for_key(key(KeyCode::F(1))),
        Some(Command::CloseHelp)
    );
}

#[test]
fn help_f2_fires_show_settings() {
    assert_eq!(
        help_command_for_key(key(KeyCode::F(2))),
        Some(Command::ShowSettings)
    );
}

#[test]
fn help_f3_fires_show_sessions() {
    assert_eq!(
        help_command_for_key(key(KeyCode::F(3))),
        Some(Command::ShowSessions)
    );
}

#[test]
fn help_f4_fires_show_playlists() {
    assert_eq!(
        help_command_for_key(key(KeyCode::F(4))),
        Some(Command::ShowPlaylists)
    );
}

#[test]
fn help_up_fires_scroll_by_negative_one() {
    assert_eq!(
        help_command_for_key(key(KeyCode::Up)),
        Some(Command::ScrollBy(-1))
    );
}

#[test]
fn help_down_fires_scroll_by_one() {
    assert_eq!(
        help_command_for_key(key(KeyCode::Down)),
        Some(Command::ScrollBy(1))
    );
}

#[test]
fn help_page_up_fires_scroll_by_negative_ten() {
    assert_eq!(
        help_command_for_key(key(KeyCode::PageUp)),
        Some(Command::ScrollBy(-10))
    );
}

#[test]
fn help_page_down_fires_scroll_by_ten() {
    assert_eq!(
        help_command_for_key(key(KeyCode::PageDown)),
        Some(Command::ScrollBy(10))
    );
}

#[test]
fn help_home_fires_scroll_home() {
    assert_eq!(
        help_command_for_key(key(KeyCode::Home)),
        Some(Command::ScrollHome)
    );
}

#[test]
fn help_unrelated_key_does_not_fire() {
    assert_eq!(help_command_for_key(key(KeyCode::Char('x'))), None);
}

// ── dispatch: state-mutating variants ────────────────────────────────────

// `MBV_SYSTEM` is a process-global env var, so tests that touch it must
// not run concurrently with other env-mutating tests. Reuse config.rs's
// `SYS_ENV_LOCK` rather than defining a second, independent mutex here.
use crate::config::tests::SYS_ENV_LOCK as ENV_LOCK;

/// RAII guard that points state-dir lookups at a fresh tempdir and
/// cleans up on drop -- including on panic.
struct XdgStateHomeGuard {
    dir: std::path::PathBuf,
    _state_dir: crate::config::TestStateDirGuard,
}

impl XdgStateHomeGuard {
    fn new() -> Self {
        let dir = tempfile_dir();
        std::env::remove_var("MBV_SYSTEM");
        let state_dir = crate::config::TestStateDirGuard::new_at(dir.join("mbv"));
        Self {
            dir,
            _state_dir: state_dir,
        }
    }
}

impl Drop for XdgStateHomeGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

#[test]
fn dispatch_toggle_mute_flips_state_and_persists() {
    let _g = ENV_LOCK.lock().unwrap();
    let _xdg = XdgStateHomeGuard::new();

    let mut app = make_app_stub();
    assert!(!app.mute_on);
    app.dispatch(Command::ToggleMute);
    assert!(app.mute_on);

    let prefs_path = crate::config::prefs_path();
    let saved = std::fs::read_to_string(&prefs_path).expect("prefs written");
    let v: serde_json::Value = serde_json::from_str(&saved).unwrap();
    assert_eq!(v["mute_on"], serde_json::json!(true));

    app.dispatch(Command::ToggleMute);
    assert!(!app.mute_on);
}

#[test]
fn dispatch_toggle_mute_while_attached_to_session_mutes_the_session_not_local() {
    use crate::app::tests::make_session;

    let mut app = make_app_stub();
    app.connected_session_id = Some("session-1".into());
    let mut sess = make_session("remote-host", "Emby");
    sess.muted = false;
    app.connected_session_state = Some(sess);

    app.dispatch(Command::ToggleMute);

    assert!(
        app.connected_session_state.as_ref().unwrap().muted,
        "pressing mute while attached to a session must mute that session \
         (optimistically, before the network round-trip completes)"
    );
    assert!(
        !app.mute_on,
        "the local mute preference must not change while attached to a session"
    );
}

#[test]
fn dispatch_toggle_mute_while_attached_to_session_toggles_back_off() {
    use crate::app::tests::make_session;

    let mut app = make_app_stub();
    app.connected_session_id = Some("session-1".into());
    let mut sess = make_session("remote-host", "Emby");
    sess.muted = true;
    app.connected_session_state = Some(sess);

    app.dispatch(Command::ToggleMute);

    assert!(!app.connected_session_state.as_ref().unwrap().muted);
}

#[test]
fn dispatch_toggle_mute_while_attached_to_session_with_unknown_mute_state_mutes_first() {
    // No session-state poll has landed yet for this connected session --
    // `connected_session_state` is still `None`. The first press should
    // be treated as "currently not muted" and mute.
    let mut app = make_app_stub();
    app.connected_session_id = Some("session-1".into());
    app.connected_session_state = None;

    app.dispatch(Command::ToggleMute);

    assert!(!app.mute_on);
}

#[test]
fn dispatch_toggle_play_pause_local_sends_player_command() {
    let mut app = make_app_stub();
    let rx = app.player.spy_on_commands();

    app.dispatch(Command::TogglePlayPause);

    assert!(matches!(rx.try_recv(), Ok(PlayerCommand::TogglePause)));
}

#[test]
fn dispatch_toggle_play_pause_remote_does_not_touch_local_player() {
    let mut app = make_app_stub();
    app.connected_session_id = Some("session-1".into());
    let rx = app.player.spy_on_commands();

    app.dispatch(Command::TogglePlayPause);

    assert!(
        !matches!(rx.try_recv(), Ok(PlayerCommand::TogglePause)),
        "the remote playback target must not leak transport commands into the local player"
    );
}

// ── dispatch: handle_key_help variants ───────────────────────────────────

#[test]
fn dispatch_close_help_clears_show_help() {
    let mut app = make_app_stub();
    app.show_help = true;
    assert!(!app.dispatch(Command::CloseHelp));
    assert!(!app.show_help);
}

#[test]
fn dispatch_show_settings_switches_panels() {
    let mut app = make_app_stub();
    app.show_help = true;
    assert!(!app.dispatch(Command::ShowSettings));
    assert!(!app.show_help);
    assert!(app.show_settings);
}

#[test]
fn dispatch_show_sessions_switches_panels() {
    let mut app = make_app_stub();
    app.show_help = true;
    assert!(!app.dispatch(Command::ShowSessions));
    assert!(!app.show_help);
    assert!(app.show_sessions);
}

#[test]
fn dispatch_show_playlists_switches_panels() {
    let mut app = make_app_stub();
    app.show_help = true;
    // Pre-populate `playlists` so `open_playlists_panel`'s
    // `playlists.is_empty() && !playlists_loading` guard is false and it
    // never spawns the background network-loading thread.
    app.playlists = vec![crate::app::tests::make_item("Playlist", "Playlist")];
    assert!(!app.dispatch(Command::ShowPlaylists));
    assert!(!app.show_help);
    assert!(app.show_playlists);
}

#[test]
fn dispatch_scroll_home_resets_to_zero() {
    let mut app = make_app_stub();
    app.help_scroll = 7;
    assert!(!app.dispatch(Command::ScrollHome));
    assert_eq!(app.help_scroll, 0);
}

#[test]
fn dispatch_scroll_by_negative_one_saturates_at_zero() {
    let mut app = make_app_stub();
    app.help_scroll = 0;
    app.dispatch(Command::ScrollBy(-1));
    assert_eq!(app.help_scroll, 0);
}

#[test]
fn dispatch_scroll_by_negative_ten_saturates_at_zero() {
    let mut app = make_app_stub();
    app.help_scroll = 3;
    app.dispatch(Command::ScrollBy(-10));
    assert_eq!(app.help_scroll, 0);
}

#[test]
fn dispatch_scroll_by_one_increments() {
    let mut app = make_app_stub();
    app.help_scroll = 5;
    app.dispatch(Command::ScrollBy(1));
    assert_eq!(app.help_scroll, 6);
}

#[test]
fn dispatch_scroll_by_ten_increments() {
    let mut app = make_app_stub();
    app.help_scroll = 5;
    app.dispatch(Command::ScrollBy(10));
    assert_eq!(app.help_scroll, 15);
}

#[test]
fn dispatch_quit_when_queue_not_dirty_returns_true_and_persists() {
    let _g = ENV_LOCK.lock().unwrap();
    let _xdg = XdgStateHomeGuard::new();

    let mut app = make_app_stub();
    assert!(!app.queue_dirty);
    assert!(app.dispatch(Command::Quit));

    let prefs_path = crate::config::prefs_path();
    assert!(
        std::fs::read_to_string(&prefs_path).is_ok(),
        "try_quit's non-dirty path should have called save_prefs()"
    );
}

// ── dispatch: QueuePlayCursor (issue #134) ───────────────────────────────
// Shared by the queue tab's `Enter` key and a double-click on a queue row
// (`handle_mouse`); see the `Command::QueuePlayCursor` doc comment.

use crate::app::tests::make_item;

fn set_local_queue(app: &mut crate::app::App, items: Vec<mbv_core::api::MediaItem>, cursor: usize) {
    app.player_tab.set_items(items, cursor);
}

#[test]
fn queue_play_cursor_on_empty_queue_is_a_no_op() {
    let mut app = make_app_stub();
    assert!(!app.dispatch(Command::QueuePlayCursor));
    assert!(app.status.is_empty());
}

#[test]
fn queue_play_cursor_while_attached_to_session_hands_off_to_session() {
    let mut app = make_app_stub();
    set_local_queue(
        &mut app,
        vec![
            make_item("Track One", "Audio"),
            make_item("Track Two", "Audio"),
        ],
        1,
    );
    app.connected_session_id = Some("session-1".into());

    app.dispatch(Command::QueuePlayCursor);

    assert!(
        app.status.contains("Playing on remote"),
        "expected a remote-handoff status flash, got {:?}",
        app.status
    );
}

#[test]
fn queue_play_cursor_jumps_to_cursor_when_active_and_playback_scope() {
    let mut app = make_app_stub();
    set_local_queue(
        &mut app,
        vec![
            make_item("Track One", "Audio"),
            make_item("Track Two", "Audio"),
        ],
        1,
    );
    {
        let mut st = app.player.status.lock().unwrap();
        st.active = true;
        st.current_idx = 0;
    }
    let rx = app.player.spy_on_commands();

    app.dispatch(Command::QueuePlayCursor);

    assert!(matches!(rx.try_recv(), Ok(PlayerCommand::JumpTo(1))));
}

#[test]
fn queue_play_cursor_seeks_to_start_when_cursor_is_the_current_playing_audio_item() {
    let mut app = make_app_stub();
    set_local_queue(&mut app, vec![make_item("Track One", "Audio")], 0);
    {
        let mut st = app.player.status.lock().unwrap();
        st.active = true;
        st.current_idx = 0;
    }
    let rx = app.player.spy_on_commands();

    app.dispatch(Command::QueuePlayCursor);

    assert!(matches!(
        rx.try_recv(),
        Ok(PlayerCommand::SeekAbsolute(pos)) if pos == 0.0
    ));
}

// Same unique-tempdir convention as api.rs's test-only `make_temp_data_dir`
// (uuid-suffixed, under the OS tempdir).
fn tempfile_dir() -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("mbv-test-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}
