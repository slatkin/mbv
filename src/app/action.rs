//! Action seam between key-event translation (`input.rs`) and effects
//! (`actions.rs`, `player.rs`). See issue #78.
//!
//! `playback_action_for_key` is a pure function: given a key event and two
//! booleans describing playback state, it decides *whether* a key should be
//! intercepted and *what* it means, without touching `App` at all. `dispatch`
//! then owns the state transitions for each `Action` variant.
//!
//! Converted so far: `handle_playback_key` (the issue #78 pilot) and
//! `handle_key_help` (see `src/app/input.rs`). Other modal handlers still
//! speak directly to `App` and are expected to migrate to this same `Action`
//! enum over time, one handler at a time.

use super::App;
use crate::api::TICKS_PER_SECOND;
use crate::player::PlayerCommand;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[derive(Debug, Clone, PartialEq)]
pub(super) enum Action {
    TogglePlayPause,
    Stop,
    /// Relative seek in seconds; negative rewinds, positive fast-forwards.
    SeekRelative(f64),
    NextTrack,
    PreviousTrack,
    /// `dispatch` picks `cycle_sub()` (remote session) vs `toggle_sub()` (local).
    CycleOrToggleSubtitle,
    AdjustVolume(i64),
    ToggleMute,
    /// `dispatch` replicates the `is_audio_item()` branch.
    AudioKey,

    // ── handle_key_help variants ────────────────────────────────────────
    /// `q` while the help overlay is open.
    Quit,
    /// Esc or F1: dismiss the help overlay.
    CloseHelp,
    /// F2: dismiss help, open settings.
    ShowSettings,
    /// F3: dismiss help, open sessions.
    ShowSessions,
    /// F4: dismiss help, open the playlists panel.
    ShowPlaylists,
    ScrollUp,
    ScrollDown,
    ScrollPageUp,
    ScrollPageDown,
    ScrollHome,
}

/// Translate a key event into a playback `Action`, or `None` if this handler
/// doesn't intercept the key. Pure function: no `App`/`Player` access, so it's
/// testable without constructing either.
///
/// Gating is **not** a single shared rule; it mirrors the three sequential
/// match blocks `handle_playback_key` used to have, key by key:
///
/// | Keys | Fires when |
/// | --- | --- |
/// | Space, `<`/`>` (seek), `N`/`P`, Alt+Enter (stop) | `has_remote_session` OR `active` |
/// | `z` (sub cycle/toggle) | unconditionally |
/// | `m` (mute) | unconditionally, no session check |
/// | `-`/`+` (volume) | unconditionally |
/// | `a` (audio) | only if `active`; no remote path |
pub(super) fn playback_action_for_key(
    key: KeyEvent,
    active: bool,
    has_remote_session: bool,
) -> Option<Action> {
    let alt = key.modifiers.contains(KeyModifiers::ALT);
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    let gated = has_remote_session || active;
    match key.code {
        KeyCode::Char(' ') if gated => Some(Action::TogglePlayPause),
        KeyCode::Enter if alt && gated => Some(Action::Stop),
        KeyCode::Char('<') if gated => Some(Action::SeekRelative(-5.0)),
        KeyCode::Char('>') if gated => Some(Action::SeekRelative(5.0)),
        KeyCode::Char('N') if gated => Some(Action::NextTrack),
        KeyCode::Char('P') if gated => Some(Action::PreviousTrack),
        KeyCode::Char('z') if !ctrl => Some(Action::CycleOrToggleSubtitle),
        KeyCode::Char('m') => Some(Action::ToggleMute),
        KeyCode::Char('-') => Some(Action::AdjustVolume(-5)),
        KeyCode::Char('+') | KeyCode::Char('=') => Some(Action::AdjustVolume(5)),
        KeyCode::Char('a') if active => Some(Action::AudioKey),
        _ => None,
    }
}

/// Translate a key event into a help-overlay `Action`, or `None` if this key
/// isn't bound. Pure function; no `App` access.
///
/// Unlike `playback_action_for_key`, gating is not per-key here: the caller
/// (`handle_key_help`) only calls this after confirming `self.show_help`, so
/// this function does no gating of its own. Also note: unlike the playback
/// seam, `None` from this function does NOT mean "let the key fall through to
/// other handlers" — the thin adapter in `input.rs` still swallows the key
/// (`Some(false)`), matching the old code's `_ => {}` arm followed by an
/// unconditional `Some(false)`.
pub(super) fn help_action_for_key(key: KeyEvent) -> Option<Action> {
    match key.code {
        KeyCode::Char('q') if key.modifiers.is_empty() => Some(Action::Quit),
        KeyCode::Esc | KeyCode::F(1) => Some(Action::CloseHelp),
        KeyCode::F(2) => Some(Action::ShowSettings),
        KeyCode::F(3) => Some(Action::ShowSessions),
        KeyCode::F(4) => Some(Action::ShowPlaylists),
        KeyCode::Up => Some(Action::ScrollUp),
        KeyCode::Down => Some(Action::ScrollDown),
        KeyCode::PageUp => Some(Action::ScrollPageUp),
        KeyCode::PageDown => Some(Action::ScrollPageDown),
        KeyCode::Home => Some(Action::ScrollHome),
        _ => None,
    }
}

/// Compute the absolute tick position for a remote-session seek, given the
/// current position in seconds and a relative delta in seconds.
///
/// This reconstructs the asymmetric math the old inline remote-session `<`/`>`
/// handlers had: rewinding (`delta < 0`) clamps at zero, fast-forwarding does
/// not (matching `input.rs`'s prior `(pos_s - 5).max(0)` vs. `(pos_s + 5)`).
fn remote_seek_ticks(pos_s: i64, delta: f64) -> i64 {
    let moved = pos_s + delta as i64;
    let target = if delta < 0.0 { moved.max(0) } else { moved };
    target * TICKS_PER_SECOND
}

impl App {
    /// Own the state transitions for an `Action`. Returns whether the app
    /// should quit (`true` only for `Action::Quit`'s non-prompting path;
    /// `false` for every other variant).
    ///
    /// For most playback variants this means picking a remote-session
    /// command vs. a local `Player` command, matching the divergent behavior
    /// `handle_playback_key` had inline (including its known bugs — see issue
    /// #78 follow-up).
    pub(super) fn dispatch(&mut self, action: Action) -> bool {
        match action {
            Action::Quit => return self.try_quit(),

            Action::TogglePlayPause => {
                if let Some(id) = self.connected_session_id.clone() {
                    self.do_session_command(move |c| c.session_transport(&id, "PlayPause"));
                } else {
                    self.player.send_command(PlayerCommand::TogglePause);
                }
            }
            Action::Stop => {
                if let Some(id) = self.connected_session_id.clone() {
                    self.do_session_command(move |c| c.session_transport(&id, "Stop"));
                } else {
                    self.player.stop();
                }
            }
            Action::SeekRelative(delta) => {
                if let Some(id) = self.connected_session_id.clone() {
                    let pos_s = self
                        .connected_session_state
                        .as_ref()
                        .map(|s| s.position_s)
                        .unwrap_or(0);
                    let t = remote_seek_ticks(pos_s, delta);
                    self.do_session_command(move |c| c.session_seek(&id, t));
                } else {
                    self.player.send_command(PlayerCommand::Seek(delta));
                }
            }
            Action::NextTrack => {
                if let Some(id) = self.connected_session_id.clone() {
                    self.session_jump_track(&id, 1, "NextTrack");
                } else {
                    self.player.next();
                }
            }
            Action::PreviousTrack => {
                if let Some(id) = self.connected_session_id.clone() {
                    self.session_jump_track(&id, -1, "PreviousTrack");
                } else {
                    self.player.previous();
                }
            }
            Action::CycleOrToggleSubtitle => {
                if self.connected_session_id.is_some() {
                    self.cycle_sub();
                } else {
                    self.toggle_sub();
                }
            }
            Action::AdjustVolume(delta) => {
                // adjust_volume already branches session vs. local internally.
                self.adjust_volume(delta);
            }
            Action::ToggleMute => {
                self.mute_on = !self.mute_on;
                self.player
                    .send_command(PlayerCommand::SetMute(self.mute_on));
                self.save_prefs();
            }
            Action::AudioKey => {
                if self.is_audio_item() {
                    self.toggle_mute();
                } else {
                    self.cycle_audio();
                }
            }

            Action::CloseHelp => {
                self.show_help = false;
            }
            Action::ShowSettings => {
                self.show_help = false;
                self.show_settings = true;
            }
            Action::ShowSessions => {
                self.show_help = false;
                self.show_sessions = true;
            }
            Action::ShowPlaylists => {
                self.show_help = false;
                self.open_playlists_panel();
            }
            Action::ScrollUp => {
                self.help_scroll = self.help_scroll.saturating_sub(1);
            }
            Action::ScrollDown => {
                self.help_scroll += 1;
            }
            Action::ScrollPageUp => {
                self.help_scroll = self.help_scroll.saturating_sub(10);
            }
            Action::ScrollPageDown => {
                self.help_scroll += 10;
            }
            Action::ScrollHome => {
                self.help_scroll = 0;
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::tests::make_app_stub;
    use std::sync::Mutex;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn key_alt(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::ALT)
    }

    fn key_ctrl(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::CONTROL)
    }

    // ── playback_action_for_key: gated on (active OR has_remote_session) ────

    #[test]
    fn space_fires_when_active_only() {
        assert_eq!(
            playback_action_for_key(key(KeyCode::Char(' ')), true, false),
            Some(Action::TogglePlayPause)
        );
    }

    #[test]
    fn space_fires_when_remote_session_only() {
        assert_eq!(
            playback_action_for_key(key(KeyCode::Char(' ')), false, true),
            Some(Action::TogglePlayPause)
        );
    }

    #[test]
    fn space_does_not_fire_when_neither_active_nor_remote() {
        assert_eq!(
            playback_action_for_key(key(KeyCode::Char(' ')), false, false),
            None
        );
    }

    #[test]
    fn alt_enter_stops_when_gated() {
        assert_eq!(
            playback_action_for_key(key_alt(KeyCode::Enter), true, false),
            Some(Action::Stop)
        );
        assert_eq!(
            playback_action_for_key(key_alt(KeyCode::Enter), false, true),
            Some(Action::Stop)
        );
    }

    #[test]
    fn plain_enter_does_not_stop() {
        assert_eq!(
            playback_action_for_key(key(KeyCode::Enter), true, true),
            None
        );
    }

    #[test]
    fn enter_without_alt_and_ungated_does_not_fire() {
        assert_eq!(
            playback_action_for_key(key(KeyCode::Enter), false, false),
            None
        );
    }

    #[test]
    fn seek_keys_fire_when_gated() {
        assert_eq!(
            playback_action_for_key(key(KeyCode::Char('<')), true, false),
            Some(Action::SeekRelative(-5.0))
        );
        assert_eq!(
            playback_action_for_key(key(KeyCode::Char('>')), false, true),
            Some(Action::SeekRelative(5.0))
        );
    }

    #[test]
    fn seek_keys_do_not_fire_when_ungated() {
        assert_eq!(
            playback_action_for_key(key(KeyCode::Char('<')), false, false),
            None
        );
        assert_eq!(
            playback_action_for_key(key(KeyCode::Char('>')), false, false),
            None
        );
    }

    #[test]
    fn track_nav_keys_fire_when_gated() {
        assert_eq!(
            playback_action_for_key(key(KeyCode::Char('N')), true, false),
            Some(Action::NextTrack)
        );
        assert_eq!(
            playback_action_for_key(key(KeyCode::Char('P')), false, true),
            Some(Action::PreviousTrack)
        );
    }

    #[test]
    fn track_nav_keys_do_not_fire_when_ungated() {
        assert_eq!(
            playback_action_for_key(key(KeyCode::Char('N')), false, false),
            None
        );
        assert_eq!(
            playback_action_for_key(key(KeyCode::Char('P')), false, false),
            None
        );
    }

    /// Assert that `code` produces `expected` for every (active, has_remote_session)
    /// combination — i.e. it fires unconditionally, with no gating at all.
    fn assert_fires_unconditionally(code: KeyCode, expected: Action) {
        for active in [false, true] {
            for remote in [false, true] {
                assert_eq!(
                    playback_action_for_key(key(code), active, remote),
                    Some(expected.clone()),
                    "code={code:?} active={active} remote={remote}"
                );
            }
        }
    }

    // ── `z`: unconditional, no `active` gate in either branch ───────────────

    #[test]
    fn z_fires_unconditionally() {
        assert_fires_unconditionally(KeyCode::Char('z'), Action::CycleOrToggleSubtitle);
    }

    #[test]
    fn ctrl_z_does_not_fire() {
        assert_eq!(
            playback_action_for_key(key_ctrl(KeyCode::Char('z')), true, true),
            None
        );
    }

    // ── `m`: unconditional, no session check at all (the flagged bug) ──────

    #[test]
    fn m_fires_unconditionally() {
        assert_fires_unconditionally(KeyCode::Char('m'), Action::ToggleMute);
    }

    // ── `-`/`+`: unconditional volume ────────────────────────────────────────

    #[test]
    fn volume_keys_fire_unconditionally() {
        assert_fires_unconditionally(KeyCode::Char('-'), Action::AdjustVolume(-5));
        assert_fires_unconditionally(KeyCode::Char('+'), Action::AdjustVolume(5));
        assert_fires_unconditionally(KeyCode::Char('='), Action::AdjustVolume(5));
    }

    // ── `a`: only if `active`; no remote path exists for it ─────────────────

    #[test]
    fn a_fires_only_when_active() {
        assert_eq!(
            playback_action_for_key(key(KeyCode::Char('a')), true, false),
            Some(Action::AudioKey)
        );
        assert_eq!(
            playback_action_for_key(key(KeyCode::Char('a')), true, true),
            Some(Action::AudioKey)
        );
    }

    #[test]
    fn a_does_not_fire_when_inactive_even_with_remote_session() {
        assert_eq!(
            playback_action_for_key(key(KeyCode::Char('a')), false, true),
            None
        );
        assert_eq!(
            playback_action_for_key(key(KeyCode::Char('a')), false, false),
            None
        );
    }

    #[test]
    fn unrelated_key_does_not_fire() {
        assert_eq!(
            playback_action_for_key(key(KeyCode::Char('q')), true, true),
            None
        );
    }

    // ── help_action_for_key: no gating (caller already checked show_help) ───

    #[test]
    fn help_q_fires_quit() {
        assert_eq!(
            help_action_for_key(key(KeyCode::Char('q'))),
            Some(Action::Quit)
        );
    }

    #[test]
    fn help_ctrl_q_does_not_fire() {
        assert_eq!(help_action_for_key(key_ctrl(KeyCode::Char('q'))), None);
    }

    #[test]
    fn help_esc_fires_close_help() {
        assert_eq!(
            help_action_for_key(key(KeyCode::Esc)),
            Some(Action::CloseHelp)
        );
    }

    #[test]
    fn help_f1_fires_close_help() {
        assert_eq!(
            help_action_for_key(key(KeyCode::F(1))),
            Some(Action::CloseHelp)
        );
    }

    #[test]
    fn help_f2_fires_show_settings() {
        assert_eq!(
            help_action_for_key(key(KeyCode::F(2))),
            Some(Action::ShowSettings)
        );
    }

    #[test]
    fn help_f3_fires_show_sessions() {
        assert_eq!(
            help_action_for_key(key(KeyCode::F(3))),
            Some(Action::ShowSessions)
        );
    }

    #[test]
    fn help_f4_fires_show_playlists() {
        assert_eq!(
            help_action_for_key(key(KeyCode::F(4))),
            Some(Action::ShowPlaylists)
        );
    }

    #[test]
    fn help_up_fires_scroll_up() {
        assert_eq!(
            help_action_for_key(key(KeyCode::Up)),
            Some(Action::ScrollUp)
        );
    }

    #[test]
    fn help_down_fires_scroll_down() {
        assert_eq!(
            help_action_for_key(key(KeyCode::Down)),
            Some(Action::ScrollDown)
        );
    }

    #[test]
    fn help_page_up_fires_scroll_page_up() {
        assert_eq!(
            help_action_for_key(key(KeyCode::PageUp)),
            Some(Action::ScrollPageUp)
        );
    }

    #[test]
    fn help_page_down_fires_scroll_page_down() {
        assert_eq!(
            help_action_for_key(key(KeyCode::PageDown)),
            Some(Action::ScrollPageDown)
        );
    }

    #[test]
    fn help_home_fires_scroll_home() {
        assert_eq!(
            help_action_for_key(key(KeyCode::Home)),
            Some(Action::ScrollHome)
        );
    }

    #[test]
    fn help_unrelated_key_does_not_fire() {
        assert_eq!(help_action_for_key(key(KeyCode::Char('x'))), None);
    }

    // ── dispatch: state-mutating variants ────────────────────────────────────

    // `XDG_STATE_HOME` is a process-global env var (like `MBV_SYSTEM` in
    // config.rs), so tests that touch it must not run concurrently with each
    // other. Mirrors config.rs's `SYS_ENV_LOCK` pattern.
    static XDG_STATE_HOME_LOCK: Mutex<()> = Mutex::new(());

    /// RAII guard that overrides `XDG_STATE_HOME` to a fresh tempdir and
    /// restores/cleans up on drop — including on panic, so a failed
    /// assertion mid-test can't leak the env var or the directory into
    /// later tests.
    struct XdgStateHomeGuard {
        dir: std::path::PathBuf,
    }

    impl XdgStateHomeGuard {
        fn new() -> Self {
            let dir = tempfile_dir();
            std::env::set_var("XDG_STATE_HOME", &dir);
            std::env::remove_var("MBV_SYSTEM");
            Self { dir }
        }
    }

    impl Drop for XdgStateHomeGuard {
        fn drop(&mut self) {
            std::env::remove_var("XDG_STATE_HOME");
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    #[test]
    fn dispatch_toggle_mute_flips_state_and_persists() {
        let _g = XDG_STATE_HOME_LOCK.lock().unwrap();
        let _xdg = XdgStateHomeGuard::new();

        let mut app = make_app_stub();
        assert!(!app.mute_on);
        app.dispatch(Action::ToggleMute);
        assert!(app.mute_on);

        let prefs_path = crate::config::prefs_path();
        let saved = std::fs::read_to_string(&prefs_path).expect("prefs written");
        let v: serde_json::Value = serde_json::from_str(&saved).unwrap();
        assert_eq!(v["mute_on"], serde_json::json!(true));

        app.dispatch(Action::ToggleMute);
        assert!(!app.mute_on);
    }

    // ── dispatch: handle_key_help variants ───────────────────────────────────

    #[test]
    fn dispatch_close_help_clears_show_help() {
        let mut app = make_app_stub();
        app.show_help = true;
        assert!(!app.dispatch(Action::CloseHelp));
        assert!(!app.show_help);
    }

    #[test]
    fn dispatch_show_settings_switches_panels() {
        let mut app = make_app_stub();
        app.show_help = true;
        assert!(!app.dispatch(Action::ShowSettings));
        assert!(!app.show_help);
        assert!(app.show_settings);
    }

    #[test]
    fn dispatch_show_sessions_switches_panels() {
        let mut app = make_app_stub();
        app.show_help = true;
        assert!(!app.dispatch(Action::ShowSessions));
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
        assert!(!app.dispatch(Action::ShowPlaylists));
        assert!(!app.show_help);
        assert!(app.show_playlists);
    }

    #[test]
    fn dispatch_scroll_home_resets_to_zero() {
        let mut app = make_app_stub();
        app.help_scroll = 7;
        assert!(!app.dispatch(Action::ScrollHome));
        assert_eq!(app.help_scroll, 0);
    }

    #[test]
    fn dispatch_scroll_up_saturates_at_zero() {
        let mut app = make_app_stub();
        app.help_scroll = 0;
        app.dispatch(Action::ScrollUp);
        assert_eq!(app.help_scroll, 0);
    }

    #[test]
    fn dispatch_scroll_page_up_saturates_at_zero() {
        let mut app = make_app_stub();
        app.help_scroll = 3;
        app.dispatch(Action::ScrollPageUp);
        assert_eq!(app.help_scroll, 0);
    }

    #[test]
    fn dispatch_scroll_down_increments_by_one() {
        let mut app = make_app_stub();
        app.help_scroll = 5;
        app.dispatch(Action::ScrollDown);
        assert_eq!(app.help_scroll, 6);
    }

    #[test]
    fn dispatch_scroll_page_down_increments_by_ten() {
        let mut app = make_app_stub();
        app.help_scroll = 5;
        app.dispatch(Action::ScrollPageDown);
        assert_eq!(app.help_scroll, 15);
    }

    #[test]
    fn dispatch_quit_when_queue_not_dirty_returns_true_and_persists() {
        let _g = XDG_STATE_HOME_LOCK.lock().unwrap();
        let _xdg = XdgStateHomeGuard::new();

        let mut app = make_app_stub();
        assert!(!app.queue_dirty);
        assert!(app.dispatch(Action::Quit));

        let prefs_path = crate::config::prefs_path();
        assert!(
            std::fs::read_to_string(&prefs_path).is_ok(),
            "try_quit's non-dirty path should have called save_prefs()"
        );
    }

    // ── remote_seek_ticks: asymmetric clamp (rewind only) ───────────────────

    #[test]
    fn remote_seek_rewind_clamps_at_zero() {
        // 3s in, rewind 5s: would go negative, must clamp to 0.
        assert_eq!(remote_seek_ticks(3, -5.0), 0);
    }

    #[test]
    fn remote_seek_rewind_does_not_clamp_when_unnecessary() {
        assert_eq!(remote_seek_ticks(20, -5.0), 15 * TICKS_PER_SECOND);
    }

    #[test]
    fn remote_seek_forward_has_no_clamp() {
        // Fast-forward has no lower-bound clamp in the original code; a small
        // pos_s plus a large forward delta simply goes wherever the math
        // says, same as rewind's clamp being absent here.
        assert_eq!(remote_seek_ticks(3, 5.0), 8 * TICKS_PER_SECOND);
    }

    // Same unique-tempdir convention as api.rs's test-only `make_temp_data_dir`
    // (uuid-suffixed, under the OS tempdir).
    fn tempfile_dir() -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("mbv-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }
}
