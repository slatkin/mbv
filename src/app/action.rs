//! Action seam between key-event translation (`input.rs`) and effects
//! (`actions.rs`, `player.rs`). See issue #78.
//!
//! `playback_action_for_key` is a pure function: given a key event and two
//! booleans describing playback state, it decides *whether* a key should be
//! intercepted and *what* it means, without touching `App` at all. `dispatch`
//! then owns the state transitions for each `Action` variant.
//!
//! This is a pilot slice covering only `handle_playback_key`
//! (see `src/app/input.rs`). Other modal handlers still speak directly to
//! `App` and are expected to migrate to this same `Action` enum over time.

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

/// Compute the absolute tick position for a remote-session seek, given the
/// current position in seconds and a relative delta in seconds.
///
/// This reconstructs the asymmetric math the old inline remote-session `<`/`>`
/// handlers had: rewinding (`delta < 0`) clamps at zero, fast-forwarding does
/// not (matching `input.rs`'s prior `(pos_s - 5).max(0)` vs. `(pos_s + 5)`).
fn remote_seek_ticks(pos_s: i64, delta: f64) -> i64 {
    let secs = delta.abs() as i64;
    let target = if delta < 0.0 {
        (pos_s - secs).max(0)
    } else {
        pos_s + secs
    };
    target * TICKS_PER_SECOND
}

impl App {
    /// Own the state transitions for a playback `Action`: for most variants
    /// this means picking a remote-session command vs. a local `Player`
    /// command, matching the divergent behavior `handle_playback_key` had
    /// inline (including its known bugs — see issue #78 follow-up).
    pub(super) fn dispatch(&mut self, action: Action) {
        match action {
            Action::TogglePlayPause => {
                if let Some(ref conn_id) = self.connected_session_id.clone() {
                    let id = conn_id.clone();
                    self.do_session_command(move |c| c.session_transport(&id, "PlayPause"));
                } else {
                    self.player.send_command(PlayerCommand::TogglePause);
                }
            }
            Action::Stop => {
                if let Some(ref conn_id) = self.connected_session_id.clone() {
                    let id = conn_id.clone();
                    self.do_session_command(move |c| c.session_transport(&id, "Stop"));
                } else {
                    self.player.stop();
                }
            }
            Action::SeekRelative(delta) => {
                if let Some(ref conn_id) = self.connected_session_id.clone() {
                    let pos_s = self
                        .connected_session_state
                        .as_ref()
                        .map(|s| s.position_s)
                        .unwrap_or(0);
                    let t = remote_seek_ticks(pos_s, delta);
                    let id = conn_id.clone();
                    self.do_session_command(move |c| c.session_seek(&id, t));
                } else {
                    self.player.send_command(PlayerCommand::Seek(delta));
                }
            }
            Action::NextTrack => {
                if let Some(ref conn_id) = self.connected_session_id.clone() {
                    self.session_jump_track(conn_id, 1, "NextTrack");
                } else {
                    self.player.next();
                }
            }
            Action::PreviousTrack => {
                if let Some(ref conn_id) = self.connected_session_id.clone() {
                    self.session_jump_track(conn_id, -1, "PreviousTrack");
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
        }
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

    // ── `z`: unconditional, no `active` gate in either branch ───────────────

    #[test]
    fn z_fires_unconditionally() {
        for active in [false, true] {
            for remote in [false, true] {
                assert_eq!(
                    playback_action_for_key(key(KeyCode::Char('z')), active, remote),
                    Some(Action::CycleOrToggleSubtitle),
                    "active={active} remote={remote}"
                );
            }
        }
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
        for active in [false, true] {
            for remote in [false, true] {
                assert_eq!(
                    playback_action_for_key(key(KeyCode::Char('m')), active, remote),
                    Some(Action::ToggleMute),
                    "active={active} remote={remote}"
                );
            }
        }
    }

    // ── `-`/`+`: unconditional volume ────────────────────────────────────────

    #[test]
    fn volume_keys_fire_unconditionally() {
        for active in [false, true] {
            for remote in [false, true] {
                assert_eq!(
                    playback_action_for_key(key(KeyCode::Char('-')), active, remote),
                    Some(Action::AdjustVolume(-5)),
                    "active={active} remote={remote}"
                );
                assert_eq!(
                    playback_action_for_key(key(KeyCode::Char('+')), active, remote),
                    Some(Action::AdjustVolume(5)),
                    "active={active} remote={remote}"
                );
                assert_eq!(
                    playback_action_for_key(key(KeyCode::Char('=')), active, remote),
                    Some(Action::AdjustVolume(5)),
                    "active={active} remote={remote}"
                );
            }
        }
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

    fn tempfile_dir() -> std::path::PathBuf {
        let mut dir = std::env::temp_dir();
        dir.push(format!(
            "mbv-action-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }
}
