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
    OpenIdleFeedLink,
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

    /// `h` in the queue column: collapse or expand the physical left column that
    /// contains the queue card and queue.
    TogglePowerSidebar,
}

/// Resolve the idle-feed link shortcut separately from transport bindings so
/// `o` remains available to the view when no link is displayed. A daemon-backed
/// player is still an idle feed view when no Emby session is connected, so the
/// playback backend and connected-session gates stay separate here.
pub(super) fn idle_feed_command_for_key(
    chord: KeyChord,
    player_active: bool,
    has_connected_session: bool,
    link_available: bool,
) -> Option<Command> {
    match chord.code {
        KeyCode::Char('o')
            if chord.mods.is_empty()
                && !player_active
                && !has_connected_session
                && link_available =>
        {
            Some(Command::OpenIdleFeedLink)
        }
        _ => None,
    }
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
        KeyCode::Char('a') if gated && !ctrl => Some(Command::ToggleMuteOrCycleAudio),
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
/// View-specific bindings that are not playback commands stay documented
/// separately in `render_help_panel`.
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
        keys: "Space (x2)",
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
        keys: "Esc (x2)",
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

/// Translate a key event in active inline album track mode.
///
/// This context is only active once `album_track_focus` is already `Some`, so
/// entering track mode from the album row remains in the library-panel view
/// handler. The command keeps `lib_idx` because the library panel can
/// point at any library tab.
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

            Command::OpenIdleFeedLink => {
                self.open_idle_feed_link();
            }

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
                    self.select();
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
            Command::TogglePowerSidebar => {
                self.queue_column_collapsed = !self.queue_column_collapsed;
                if self.queue_column_collapsed
                    && matches!(self.panel_focus, super::PanelFocus::Queue)
                {
                    self.set_panel_focus(super::PanelFocus::Library);
                }
            }
        }
        false
    }
}

#[cfg(test)]
#[path = "action_tests.rs"]
mod tests;
