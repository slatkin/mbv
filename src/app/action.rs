//! Command seam between key-event translation (`input.rs`) and effects
//! (`actions.rs`, `player.rs`). See issue #78.
//!
//! `playback_command_for_key` is a pure function: given a key event and two
//! booleans describing playback state, it decides *whether* a key should be
//! intercepted and *what* it means, without touching `App` at all. `dispatch`
//! then owns the state transitions for each `Command` variant.
//!
//! Converted so far: `handle_playback_key` (the issue #78 pilot) and
//! `handle_key_help` (see `src/app/input.rs`). Other modal handlers still
//! speak directly to `App` and are expected to migrate to this same `Command`
//! enum over time, one handler at a time.

use super::input_resolver::KeyChord;
use super::App;
use crossterm::event::{KeyCode, KeyModifiers};
use mbv_core::player::PlayerCommand;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq)]
pub(super) enum Command {
    TogglePlayPause,
    Stop,
    /// Relative seek in seconds; negative rewinds, positive fast-forwards.
    SeekRelative(f64),
    NextTrack,
    PreviousTrack,
    /// `z`: `dispatch` always calls `cycle_sub()`, which cycles through all
    /// available subtitle tracks (plus "off") for both remote sessions and
    /// local playback -- unified in #86 so the two backends no longer
    /// diverge (local used to be a plain on/off `toggle_sub()`). The
    /// local-idle fallback (cycling the default subtitle *mode* when there's
    /// no active player) still lives inside `cycle_sub()`, since it has no
    /// session equivalent to unify with.
    CycleOrToggleSubtitle,
    AdjustVolume(i64),
    /// The `m` key: flips `mute_on` and sends `PlayerCommand::SetMute`.
    /// **Not** the same mechanism as `ToggleMuteOrCycleAudio`'s mute path
    /// below, which instead flips `ui_volume`/`pre_mute_volume` via
    /// `SetVolume` — these are two separate, pre-existing "mute" code paths
    /// with no cross-reference in the original code; not unified here since
    /// that would be a behavior change (see issue #78 follow-up, #84).
    ToggleMute,
    /// The `a` key: `dispatch` replicates the `is_audio_item()` branch,
    /// calling `toggle_mute()` (the `ui_volume`/`pre_mute_volume`/`SetVolume`
    /// mechanism, *not* `Command::ToggleMute`'s `mute_on`/`SetMute`) if the
    /// current item is audio-only, otherwise `cycle_audio()`. Gated the same
    /// way as the other transport keys (`active OR has_remote_session`) —
    /// see #88. The shared `PlaybackTarget` seam owns the local-vs-remote
    /// split underneath `is_audio_item()`, `toggle_mute()`, and
    /// `cycle_audio()`, so this action layer no longer re-derives it in each
    /// helper.
    ToggleMuteOrCycleAudio,

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
    /// Scroll `help_scroll` by a signed delta: negative clamps at zero
    /// (`Up`/`PageUp`), positive does not (`Down`/`PageDown`, preserving the
    /// pre-existing unclamped-scroll-down quirk — see `dispatch`).
    ScrollBy(i64),
    ScrollHome,

    // ── queue activation (issue #134) ───────────────────────────────────
    /// Activate the item at the visible queue's cursor: `Enter` on the queue
    /// tab, or a double-click on a queue row (`handle_mouse`'s
    /// `is_double`/queue branch — the two were already made to match in
    /// a70ad7a, before either went through `Command`; this variant is the
    /// single implementation both now share). Session-attached: hands the
    /// item off to the remote session. Otherwise: seeks to the top if it's
    /// the already-playing audio item, jumps to it if it's elsewhere in the
    /// active playback queue, or replaces the local playback queue and plays
    /// from this index if the visible queue isn't the one currently playing.
    QueuePlayCursor,

    // ── Power inline album track mode ───────────────────────────────────
    /// `Enter` while an inline album track is focused.
    PowerAlbumTrackEnter(usize),
    /// `Esc`/`Backspace` while an inline album track is focused.
    PowerAlbumTrackDismiss(usize),
    /// `Up`/`Down` while an inline album track is focused.
    PowerAlbumTrackMove {
        lib_idx: usize,
        delta: i64,
    },
}

/// Translate a key event into a playback `Command`, or `None` if this handler
/// doesn't intercept the key. Pure function: no `App`/`Player` access, so it's
/// testable without constructing either.
///
/// Gating is **not** a single shared rule; it mirrors the three sequential
/// match blocks `handle_playback_key` used to have, key by key:
///
/// | Keys | Fires when |
/// | --- | --- |
/// | Space, `<`/`>` (seek), `N`/`P`, Esc (stop), `a` (audio) | `has_remote_session` OR `active` |
/// | `z` (sub cycle/toggle) | unconditionally |
/// | `m` (mute) | unconditionally, no session check |
/// | `-`/`+` (volume) | unconditionally |
pub(super) fn playback_command_for_key(
    chord: KeyChord,
    active: bool,
    has_remote_session: bool,
) -> Option<Command> {
    let ctrl = chord.mods.contains(KeyModifiers::CONTROL);
    let gated = has_remote_session || active;
    match chord.code {
        KeyCode::Char(' ') if gated => Some(Command::TogglePlayPause),
        KeyCode::Esc if gated => Some(Command::Stop),
        KeyCode::Char('<') if gated => Some(Command::SeekRelative(-5.0)),
        KeyCode::Char('>') if gated => Some(Command::SeekRelative(5.0)),
        KeyCode::Char('N') if gated => Some(Command::NextTrack),
        KeyCode::Char('P') if gated => Some(Command::PreviousTrack),
        KeyCode::Char('z') if !ctrl => Some(Command::CycleOrToggleSubtitle),
        KeyCode::Char('m') => Some(Command::ToggleMute),
        KeyCode::Char('-') => Some(Command::AdjustVolume(-5)),
        KeyCode::Char('+') | KeyCode::Char('=') => Some(Command::AdjustVolume(5)),
        KeyCode::Char('a') if gated => Some(Command::ToggleMuteOrCycleAudio),
        _ => None,
    }
}

/// Help-overlay metadata for a subset of `playback_command_for_key`'s
/// bindings — the "[playback]" section of the help overlay renders directly
/// from this table (see `render_help_panel`) instead of a hand-copied list,
/// so the two can no longer silently drift apart. See issue #133 (phase 4)
/// and `docs/adr/0002-centralized-input-handling.md`.
///
/// Each entry pairs display text with a *sample* chord (or chords) + gating
/// flag that a characterization test
/// (`playback_help_bindings_match_playback_command_for_key`, below) replays
/// through `playback_command_for_key` to assert this table stays truthful.
/// When a display entry covers more than one physical key (`<`/`>`, `N`/`P`,
/// `-`/`+`/`=`), `samples` lists every one of them, each paired with the
/// command it must resolve to — so the test exercises the whole displayed
/// claim, not just one side of it.
///
/// Keys not yet covered here (e.g. `h` hide/show player, which lives in
/// `handle_key_panel_toggle`, not this Command seam) stay hand-written in
/// `render_help_panel` until their handler migrates onto `Command` too.
pub(super) struct PlaybackHelpBinding {
    /// Display text shown in the help overlay (e.g. `"Space"`, `"< / >"`).
    pub keys: &'static str,
    /// One-line description shown next to `keys`.
    pub label: &'static str,
    // Only read by the `playback_help_bindings_match_playback_command_for_key`
    // characterization test below; kept outside `#[cfg(test)]` since these
    // fields are part of the type's intended (drift-guard) purpose, not
    // test-only scaffolding — mirrors `ContextEntry::name` in
    // `input_resolver.rs`.
    #[allow(dead_code)]
    /// Every chord that produces the paired command via
    /// `playback_command_for_key`, used only to keep this table honest in
    /// tests — not consulted at runtime.
    pub samples: &'static [(KeyChord, Command)],
    #[allow(dead_code)]
    /// Whether each sample in `samples` only resolves to its command when
    /// gated (`active || has_remote_session`); `false` means it fires
    /// unconditionally.
    pub gated: bool,
}

pub(super) const PLAYBACK_HELP_BINDINGS: &[PlaybackHelpBinding] = &[
    PlaybackHelpBinding {
        keys: "Space",
        label: "Pause/Resume",
        samples: &[(
            KeyChord {
                code: KeyCode::Char(' '),
                mods: KeyModifiers::NONE,
            },
            Command::TogglePlayPause,
        )],
        gated: true,
    },
    PlaybackHelpBinding {
        keys: "Esc",
        label: "Stop",
        samples: &[(
            KeyChord {
                code: KeyCode::Esc,
                mods: KeyModifiers::NONE,
            },
            Command::Stop,
        )],
        gated: true,
    },
    PlaybackHelpBinding {
        keys: "< / >",
        label: "Seek \u{b1}5 seconds",
        samples: &[
            (
                KeyChord {
                    code: KeyCode::Char('<'),
                    mods: KeyModifiers::NONE,
                },
                Command::SeekRelative(-5.0),
            ),
            (
                KeyChord {
                    code: KeyCode::Char('>'),
                    mods: KeyModifiers::NONE,
                },
                Command::SeekRelative(5.0),
            ),
        ],
        gated: true,
    },
    PlaybackHelpBinding {
        keys: "Shift+N / P",
        label: "Next / Previous track",
        samples: &[
            (
                KeyChord {
                    code: KeyCode::Char('N'),
                    mods: KeyModifiers::NONE,
                },
                Command::NextTrack,
            ),
            (
                KeyChord {
                    code: KeyCode::Char('P'),
                    mods: KeyModifiers::NONE,
                },
                Command::PreviousTrack,
            ),
        ],
        gated: true,
    },
    PlaybackHelpBinding {
        keys: "- / +",
        label: "Volume down / up",
        samples: &[
            (
                KeyChord {
                    code: KeyCode::Char('-'),
                    mods: KeyModifiers::NONE,
                },
                Command::AdjustVolume(-5),
            ),
            (
                KeyChord {
                    code: KeyCode::Char('+'),
                    mods: KeyModifiers::NONE,
                },
                Command::AdjustVolume(5),
            ),
            (
                KeyChord {
                    code: KeyCode::Char('='),
                    mods: KeyModifiers::NONE,
                },
                Command::AdjustVolume(5),
            ),
        ],
        gated: false,
    },
    PlaybackHelpBinding {
        keys: "m",
        label: "Mute",
        samples: &[(
            KeyChord {
                code: KeyCode::Char('m'),
                mods: KeyModifiers::NONE,
            },
            Command::ToggleMute,
        )],
        gated: false,
    },
    PlaybackHelpBinding {
        keys: "a",
        label: "Cycle audio track",
        samples: &[(
            KeyChord {
                code: KeyCode::Char('a'),
                mods: KeyModifiers::NONE,
            },
            Command::ToggleMuteOrCycleAudio,
        )],
        gated: true,
    },
    PlaybackHelpBinding {
        keys: "z",
        label: "Cycle subtitles",
        samples: &[(
            KeyChord {
                code: KeyCode::Char('z'),
                mods: KeyModifiers::NONE,
            },
            Command::CycleOrToggleSubtitle,
        )],
        gated: false,
    },
];

/// Translate a key event into a help-overlay `Command`, or `None` if this key
/// isn't bound. Pure function; no `App` access.
///
/// Unlike `playback_command_for_key`, gating is not per-key here: the caller
/// (`handle_key_help`) only calls this after confirming `self.show_help`, so
/// this function does no gating of its own. Also note: unlike the playback
/// seam, `None` from this function does NOT mean "let the key fall through to
/// other handlers" — the thin adapter in `input.rs` still swallows the key
/// (`Some(false)`), matching the old code's `_ => {}` arm followed by an
/// unconditional `Some(false)`.
pub(super) fn help_command_for_key(chord: KeyChord) -> Option<Command> {
    match chord.code {
        KeyCode::Char('q') if chord.mods.is_empty() => Some(Command::Quit),
        KeyCode::Esc | KeyCode::F(1) => Some(Command::CloseHelp),
        KeyCode::F(2) => Some(Command::ShowSettings),
        KeyCode::F(3) => Some(Command::ShowSessions),
        KeyCode::F(4) => Some(Command::ShowPlaylists),
        KeyCode::Up => Some(Command::ScrollBy(-1)),
        KeyCode::Down => Some(Command::ScrollBy(1)),
        KeyCode::PageUp => Some(Command::ScrollBy(-10)),
        KeyCode::PageDown => Some(Command::ScrollBy(10)),
        KeyCode::Home => Some(Command::ScrollHome),
        _ => None,
    }
}

/// Translate a key event in active Power inline album track mode.
///
/// This context is only active once `album_track_focus` is already `Some`, so
/// entering track mode from the album row remains in the Power left-panel view
/// handler. The command keeps `lib_idx` because Power View's left panel can
/// point at any library tab while `tab_idx` remains on the queue tab.
pub(super) fn power_album_track_command_for_key(
    chord: KeyChord,
    lib_idx: usize,
) -> Option<Command> {
    let is_power_nav = matches!(
        chord.code,
        KeyCode::Left | KeyCode::Right | KeyCode::Up | KeyCode::Down
    ) && chord.mods.contains(KeyModifiers::ALT);
    if is_power_nav {
        return None;
    }

    match chord.code {
        KeyCode::Enter => Some(Command::PowerAlbumTrackEnter(lib_idx)),
        KeyCode::Esc | KeyCode::Backspace => Some(Command::PowerAlbumTrackDismiss(lib_idx)),
        KeyCode::Up => Some(Command::PowerAlbumTrackMove { lib_idx, delta: -1 }),
        KeyCode::Down => Some(Command::PowerAlbumTrackMove { lib_idx, delta: 1 }),
        _ => None,
    }
}

impl App {
    /// Own the state transitions for a `Command`. Returns whether the app
    /// should quit (`true` only for `Command::Quit`'s non-prompting path;
    /// `false` for every other variant).
    ///
    /// For most playback variants this means picking a remote-session
    /// command vs. a local `Player` command, matching the divergent behavior
    /// `handle_playback_key` had inline (including its known bugs — see issue
    /// #78 follow-up).
    pub(super) fn dispatch(&mut self, command: Command) -> bool {
        match command {
            Command::Quit => return self.try_quit(),

            Command::TogglePlayPause => {
                self.playback_target().toggle_play_pause(self);
            }
            Command::Stop => {
                self.playback_target().stop(self);
            }
            Command::SeekRelative(delta) => {
                self.playback_target().seek_relative(self, delta);
            }
            Command::NextTrack => {
                self.playback_target().jump_track(self, 1, "NextTrack");
            }
            Command::PreviousTrack => {
                self.playback_target().jump_track(self, -1, "PreviousTrack");
            }
            Command::CycleOrToggleSubtitle => {
                // cycle_sub() branches internally on connected_session_id,
                // and falls back to the idle subtitle-mode cycle itself when
                // local playback has no active player (see #86).
                self.cycle_sub();
            }
            Command::AdjustVolume(delta) => {
                // adjust_volume already branches session vs. local internally.
                self.adjust_volume(delta);
            }
            Command::ToggleMute => {
                self.playback_target().toggle_command_mute(self);
            }
            Command::ToggleMuteOrCycleAudio => {
                if self.is_audio_item() {
                    self.toggle_mute();
                } else {
                    self.cycle_audio();
                }
            }

            Command::CloseHelp => {
                self.show_help = false;
            }
            Command::ShowSettings | Command::ShowSessions | Command::ShowPlaylists => {
                self.show_help = false;
                match command {
                    Command::ShowSettings => self.show_settings = true,
                    Command::ShowSessions => self.show_sessions = true,
                    Command::ShowPlaylists => self.open_playlists_panel(),
                    _ => unreachable!(),
                }
            }
            Command::ScrollBy(delta) => {
                if delta < 0 {
                    self.help_scroll = self.help_scroll.saturating_sub((-delta) as u16);
                } else {
                    // No upper clamp here, matching the pre-existing quirk in
                    // the original inline handler (presumably clamped at
                    // render time instead).
                    self.help_scroll += delta as u16;
                }
            }
            Command::ScrollHome => {
                self.help_scroll = 0;
            }

            Command::QueuePlayCursor => {
                let queue = self.displayed_queue();
                let t = queue.queue_cursor;
                let n = queue.items.len();
                if t < n {
                    if let Some(conn_id) = self.connected_session_id.clone() {
                        let item = queue.items[t].clone();
                        let item_ids: Vec<String> =
                            queue.items.iter().map(|i| i.id.clone()).collect();
                        let start_ticks = item.playback_position_ticks;
                        let label = item.playback_label();
                        self.flash_status(format!("Playing on remote: {label}"));
                        self.do_session_command(move |c| {
                            c.session_play_items(&conn_id, &item_ids, t, start_ticks)
                        });
                    } else {
                        // Only read once we know we're not handing off to a
                        // session -- `queue_scope_is_playback` is the one
                        // reader below.
                        let scope = self.visible_queue_scope();
                        let st = self.player.status.lock().unwrap();
                        let active = st.active;
                        let current_idx = st.current_idx;
                        drop(st);
                        if active && self.queue_scope_is_playback(scope) {
                            let is_audio =
                                queue.items.get(t).map(|i| i.is_audio()).unwrap_or(false);
                            if t == current_idx && is_audio {
                                self.player.send_command(PlayerCommand::SeekAbsolute(0.0));
                            } else if t != current_idx {
                                self.player.send_command(PlayerCommand::JumpTo(t));
                            }
                        } else {
                            // `t < n` above already guarantees the queue is
                            // non-empty, so no `is_empty()` re-check here.
                            //
                            // `replace_playback_queue` and `play_queue` each
                            // take ownership of their own `Vec<MediaItem>`
                            // and both run, so two clones of `queue.items`
                            // are the minimum here, not a redundant third.
                            let items = queue.items.clone();
                            let c = Arc::new(self.client.lock().unwrap().clone());
                            self.replace_playback_queue(items.clone(), t);
                            self.player.play_queue(
                                items,
                                t,
                                self.queue_source.clone(),
                                c,
                                self.ui_volume,
                            );
                        }
                    }
                }
            }

            Command::PowerAlbumTrackEnter(lib_idx) => {
                if self
                    .selected_album_item(lib_idx)
                    .and_then(|album| {
                        self.album_tracks_cache.get(&album.id).and_then(|tracks| {
                            self.libs[lib_idx]
                                .album_track_focus
                                .and_then(|idx| tracks.get(idx))
                        })
                    })
                    .is_some()
                {
                    let saved = self.tab_idx;
                    self.tab_idx = self.lib_tab_offset() + lib_idx;
                    self.select();
                    self.tab_idx = saved;
                }
            }
            Command::PowerAlbumTrackDismiss(lib_idx) => {
                self.libs[lib_idx].album_track_focus = None;
            }
            Command::PowerAlbumTrackMove { lib_idx, delta } => {
                if let Some(idx) = self.libs[lib_idx].album_track_focus {
                    let track_count = self
                        .selected_album_item(lib_idx)
                        .and_then(|item| self.album_tracks_cache.get(&item.id))
                        .map(|tracks| tracks.len())
                        .unwrap_or(0);
                    if track_count > 0 {
                        let new_idx =
                            (idx as i64 + delta).clamp(0, track_count as i64 - 1) as usize;
                        self.libs[lib_idx].album_track_focus = Some(new_idx);
                    }
                }
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
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
    fn space_fires_when_active_only() {
        assert_eq!(
            playback_command_for_key(key(KeyCode::Char(' ')), true, false),
            Some(Command::TogglePlayPause)
        );
    }

    #[test]
    fn space_fires_when_remote_session_only() {
        assert_eq!(
            playback_command_for_key(key(KeyCode::Char(' ')), false, true),
            Some(Command::TogglePlayPause)
        );
    }

    #[test]
    fn space_does_not_fire_when_neither_active_nor_remote() {
        assert_eq!(
            playback_command_for_key(key(KeyCode::Char(' ')), false, false),
            None
        );
    }

    #[test]
    fn esc_stops_when_gated() {
        assert_eq!(
            playback_command_for_key(key(KeyCode::Esc), true, false),
            Some(Command::Stop)
        );
        assert_eq!(
            playback_command_for_key(key(KeyCode::Esc), false, true),
            Some(Command::Stop)
        );
    }

    #[test]
    fn esc_does_not_stop_when_ungated() {
        assert_eq!(
            playback_command_for_key(key(KeyCode::Esc), false, false),
            None
        );
    }

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

    #[test]
    fn seek_keys_fire_when_gated() {
        assert_eq!(
            playback_command_for_key(key(KeyCode::Char('<')), true, false),
            Some(Command::SeekRelative(-5.0))
        );
        assert_eq!(
            playback_command_for_key(key(KeyCode::Char('>')), false, true),
            Some(Command::SeekRelative(5.0))
        );
    }

    #[test]
    fn seek_keys_do_not_fire_when_ungated() {
        assert_eq!(
            playback_command_for_key(key(KeyCode::Char('<')), false, false),
            None
        );
        assert_eq!(
            playback_command_for_key(key(KeyCode::Char('>')), false, false),
            None
        );
    }

    #[test]
    fn track_nav_keys_fire_when_gated() {
        assert_eq!(
            playback_command_for_key(key(KeyCode::Char('N')), true, false),
            Some(Command::NextTrack)
        );
        assert_eq!(
            playback_command_for_key(key(KeyCode::Char('P')), false, true),
            Some(Command::PreviousTrack)
        );
    }

    #[test]
    fn track_nav_keys_do_not_fire_when_ungated() {
        assert_eq!(
            playback_command_for_key(key(KeyCode::Char('N')), false, false),
            None
        );
        assert_eq!(
            playback_command_for_key(key(KeyCode::Char('P')), false, false),
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
    fn a_fires_when_active_and_remote_session() {
        assert_eq!(
            playback_command_for_key(key(KeyCode::Char('a')), true, true),
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

    fn set_local_queue(
        app: &mut crate::app::App,
        items: Vec<mbv_core::api::MediaItem>,
        cursor: usize,
    ) {
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
}
