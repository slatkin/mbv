//! Command seam between key-event translation (`input.rs`) and effects
//! (`actions.rs`, `player.rs`). See issue #78.
//!
//! Router-owned key resolution lives in the central keyboard policy
//! (`key_policy.rs`): the split transport actions match their configured
//! chords there and bind their fixed payloads in `command_for_policy`. This
//! module keeps the `Command` variants, the idle-feed link's availability
//! condition, and `dispatch`'s state transitions.
//!
//! The help overlay was converted to a TuiRealm Interactive Component
//! (`src/app/components/help.rs`) and no longer routes through this `Command`
//! enum. Other modal handlers still speak directly to `App` and are expected to
//! migrate to this same `Command` enum over time, one handler at a time.

use super::input_resolver::KeyChord;
use super::notify_actions::ToastSeverity;
use super::App;
use crossterm::event::KeyCode;
use mbv_core::api::EmbyItem;
use mbv_core::playback_queue::QueueSlotId;
use mbv_core::player::PlayerCommand;
use std::sync::Arc;

/// The volume step the `-`/`+` keys dispatch and the `StatusBarPanel`
/// volume pill's wheel mapping mirrors (single definition, review of
/// tasks 2.1-2.2).
pub(crate) const VOLUME_STEP: i64 = 5;

#[derive(Debug, Clone, PartialEq)]
pub(super) enum Command {
    OpenIdleFeedLink,
    ToggleVisualizer,
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

    // ── queue activation (issue #134) ───────────────────────────────────
    /// Activate the item at the given queue index: `Enter` on the queue
    /// tab, or a double-click on a queue row (`handle_mouse`'s
    /// `is_double`/queue branch — the two were already made to match in
    /// a70ad7a, before either went through `Command`; this variant is the
    /// single implementation both now share). Session-attached: hands the
    /// item off to the remote session. Otherwise: seeks to the top if it's
    /// the already-playing audio item, jumps to it if it's elsewhere in the
    /// active playback queue, or replaces the local playback queue and plays
    /// from this index if the visible queue isn't the one currently playing.
    /// The target index is carried explicitly (split-queue-cursor-ownership
    /// D2): the shell resolves the slot the user selected and passes it
    /// rather than this command re-reading `queue.queue_cursor` as an
    /// ambient argument channel.
    QueuePlayCursor(usize),

    /// `x`: cycle the Power View layout through both, queue-only, and
    /// library-only (see `PanelMode`); below the mini-view threshold it
    /// toggles queue-only and library-only.
    CyclePanelMode,

    // ── destination-independent routing ─────────────────────────────────
    /// Quit the client through the normal dirty-queue/prefs shutdown path.
    Quit,
    NextLibraryTab,
    PreviousLibraryTab,
    SetLibraryTab(usize),
    ForceClear,
    /// Raise the "Clear queue?" confirmation prompt (the global `c` binding).
    RequestClearQueue,
    RefreshCurrentView,
    ToggleSettings,
    OpenSessions,
    OpenPlaylists,
    OpenSearch,
    /// Model-owned because mounting Help belongs to the TuiRealm shell.
    OpenHelp,
    FocusPanel(super::PanelFocus),
}

/// Resolve the idle-feed link shortcut. The link's eligibility is its own
/// condition, separate from the transport bindings (task 5.1 keeps it as the
/// `open_idle_feed_link` action's gate), so `o` remains available to the view
/// when no link is displayed. A daemon-backed
/// player is still an idle feed view when no Emby session is connected, so the
/// playback backend and connected-session gates stay separate here. The
/// `playback_panel_present_idle` input is the Queue playback panel's presence
/// with nothing playing (task 3.8): the panel is mounted in every
/// queue-visible layout, so the gate follows it instead of the panel mode and
/// the link is gated off in idle `both` and `queue-only`, on in idle
/// `library-only` where the strip displays the feed.
pub(super) fn idle_feed_command_for_key(
    chord: KeyChord,
    player_active: bool,
    has_connected_session: bool,
    playback_panel_present_idle: bool,
    link_available: bool,
) -> Option<Command> {
    match chord.code {
        KeyCode::Char('o')
            if chord.mods.is_empty()
                && !player_active
                && !has_connected_session
                && !playback_panel_present_idle
                && link_available =>
        {
            Some(Command::OpenIdleFeedLink)
        }
        _ => None,
    }
}

impl App {
    /// Send an already-accepted transition to whichever component owns
    /// playback. When the owner runs out of process (reached over ctrl,
    /// including this machine's Local daemon), request the jump from it via
    /// `UnifiedQueuePlaySlot`; when this process is the owner, send the local
    /// `JumpTo` to the Playback run. Returns `false` when the request could
    /// not be sent.
    fn reject_disconnected_remote_jump(&mut self) -> bool {
        if !self.player.is_remote_disconnected() {
            return false;
        }
        self.handle_player_event(mbv_core::player::PlayerEvent::CommandRejected(
            super::actions::CONNECTION_LOST_MESSAGE.to_string(),
        ));
        true
    }

    fn request_remote_slot_jump(&mut self, slot_id: QueueSlotId) -> bool {
        if self.reject_disconnected_remote_jump() {
            return false;
        }
        let sent = self
            .player
            .queue_play_slot(mbv_core::ctrl::slot_id_to_u64(slot_id));
        if !sent {
            self.reject_disconnected_remote_jump();
        }
        sent
    }

    pub(super) fn dispatch_jump(
        &mut self,
        transition: mbv_core::playback_transition::Transition,
    ) -> bool {
        if self.player.is_remote() {
            return self.request_remote_slot_jump(transition.target);
        }
        let resume_ticks = mbv_core::player::resume_ticks_for_slot(
            &self.playback_queue().queue,
            transition.target,
        );
        self.player.send_command(transition.into_jump(resume_ticks))
    }

    /// Request a jump to an existing canonical slot: the fresh-jump seam.
    /// When the Player owner runs out of process, request the jump from it
    /// (`UnifiedQueuePlaySlot`) and report its acceptance; the active slot
    /// then follows the owner's queue snapshot, never a client cursor write
    /// here. When this process is the owner, sync the canonical snapshot,
    /// mint and accept a local transition, and dispatch the transition the
    /// owner accepted now (or queue it behind an in-flight one).
    pub(super) fn request_slot_jump(&mut self, slot_id: QueueSlotId) -> bool {
        if self.player.is_remote() {
            return self.request_remote_slot_jump(slot_id);
        }
        self.bare_owner
            .sync_canonical_queue(self.playback_queue().queue.clone());
        let (request_id, generation) = self.bare_owner.mint_local_transition();
        let transition =
            mbv_core::playback_transition::Transition::new(request_id, generation, slot_id);
        match self.bare_owner.accept_local_transition(transition) {
            mbv_core::playback_transition::DispatchDecision::DispatchNow(t) => {
                self.dispatch_jump(t)
            }
            mbv_core::playback_transition::DispatchDecision::Queued { .. } => true,
        }
    }

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
            Command::OpenIdleFeedLink => {
                self.open_idle_feed_link();
            }
            Command::ToggleVisualizer => self.toggle_visualizer(),

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

            Command::QueuePlayCursor(t) => {
                let (n, item) = {
                    let queue = self.displayed_queue();
                    let n = queue.total_queue_len();
                    let item = queue.item_at(t).cloned();
                    (n, item)
                };
                if t >= n {
                    return false;
                }
                // Validate the item at the cursor exists.
                let Some(item) = item else {
                    return false;
                };
                let owner_can_admit_audiobookshelf = self.player.can_admit_audiobookshelf();
                if !item.admissible_for_owner_with_audiobookshelf(
                    false,
                    |service| {
                        service != mbv_core::config::ServiceKind::Audiobookshelf
                            || owner_can_admit_audiobookshelf
                    },
                    owner_can_admit_audiobookshelf,
                ) {
                    self.flash(
                        "Playback owner rejected this Audiobookshelf item".into(),
                        super::notify_actions::ToastSeverity::Error,
                    );
                    return false;
                }
                if self.player.is_remote_disconnected() {
                    let status = self.player.status.lock().unwrap();
                    let active = status.active;
                    let current_idx = status.current_idx;
                    drop(status);
                    let queue = self.displayed_queue();
                    let is_jump = active
                        && self.viewed_queue_scope() == self.playing_queue_scope()
                        && t != current_idx;
                    if is_jump {
                        if let Some(slot_id) = queue.slot_id_at(t) {
                            let _ = self.request_slot_jump(slot_id);
                        } else {
                            self.handle_player_event(
                                mbv_core::player::PlayerEvent::CommandRejected(
                                    super::actions::CONNECTION_LOST_MESSAGE.to_string(),
                                ),
                            );
                        }
                    } else {
                        self.flash(
                            super::actions::CONNECTION_LOST_MESSAGE.into(),
                            ToastSeverity::Warning,
                        );
                    }
                    return false;
                }
                // Validate source for Feed entries early.
                if let mbv_core::playback_queue::QueueItem::Feed(ref entry) = item {
                    if entry.primary_source().is_none() {
                        self.flash(
                            "Feed entry has no playable source".into(),
                            super::notify_actions::ToastSeverity::Error,
                        );
                        return false;
                    }
                }
                // Hydrate stored feed-entry state before building the
                // playback snapshot so resume uses the latest position.
                if let mbv_core::playback_queue::QueueItem::Feed(ref entry) = item {
                    let hydrated = self.hydrate_feed_entry_state(entry.clone());
                    let sid = self.playback_queue().slot_id_at(t);
                    if let Some(sid) = sid {
                        let queue_mut = self.playback_queue_mut();
                        let _ = queue_mut.queue.apply_progress(
                            sid,
                            hydrated.position_ticks,
                            hydrated.played,
                        );
                    }
                }
                // Snapshot data from the queue before any mutable borrows.
                let queue = self.displayed_queue();
                let emby_items: Vec<EmbyItem> = queue
                    .queue
                    .slots()
                    .iter()
                    .filter_map(|slot| slot.item.as_emby().cloned())
                    .collect();
                let all_slots = queue.all_queue_slots();
                let slot_id = queue.slot_id_at(t);
                // Pre-compute the Emby-only projection index for the cursor
                // position, needed by the session API boundary.
                let emby_start = queue
                    .queue
                    .slots()
                    .iter()
                    .take(t)
                    .filter(|s| s.item.as_emby().is_some())
                    .count();
                // Connected remote session: hand off Emby items to the
                // session; Feed entries cannot cross the Emby session API
                // so they fall through to the local/direct-remote path.
                if let mbv_core::playback_queue::QueueItem::Emby(_) = &item {
                    if let Some(conn_id) = self.connected_session_id.clone() {
                        let label = item.display_name();
                        self.flash(
                            format!("Requesting playback: {label}"),
                            ToastSeverity::Neutral,
                        );
                        self.set_queue_scope(self.playing_queue_scope());
                        self.submit_attached_sequence(&conn_id, &emby_items, emby_start);
                        return false;
                    }
                }
                // Local / direct-remote playback.  The same path handles
                // both Feed and Emby items: jump to an active slot or
                // cold-start the full canonical queue.
                let scope = self.viewed_queue_scope();
                let st = self.player.status.lock().unwrap();
                let active = st.active;
                let current_idx = st.current_idx;
                drop(st);
                if active
                    && self.queue_scope_is_playback(scope)
                    && self.local_queue_is_owner_queue(scope)
                {
                    let is_audio = item.is_audio();
                    if t == current_idx && is_audio {
                        self.player.send_command(PlayerCommand::SeekAbsolute(0.0));
                    } else if t != current_idx {
                        let Some(slot_id) = slot_id else {
                            return false;
                        };
                        // One owner-kind seam for every fresh jump, so
                        // explicit play and the Next-Up accept cannot diverge.
                        if !self.request_slot_jump(slot_id) && !self.player.is_remote_disconnected()
                        {
                            self.flash(
                                "Playback owner rejected the queue selection".into(),
                                ToastSeverity::Error,
                            );
                        }
                    }
                } else {
                    // Cold start: submit the full canonical queue (all
                    // variants) so the player's internal playlist matches
                    // the PlayerTab's queue exactly.
                    let owner_can_admit_audiobookshelf = self.player.can_admit_audiobookshelf();
                    let eligible: Vec<_> = all_slots
                        .into_iter()
                        .filter(|slot| {
                            slot.item.admissible_for_owner_with_audiobookshelf(
                                false,
                                |service| {
                                    service != mbv_core::config::ServiceKind::Audiobookshelf
                                        || owner_can_admit_audiobookshelf
                                },
                                owner_can_admit_audiobookshelf,
                            )
                        })
                        .collect();
                    if eligible.is_empty() {
                        self.flash(
                            "Playback owner rejected the queue".into(),
                            ToastSeverity::Error,
                        );
                        return false;
                    }
                    let start_idx = eligible
                        .iter()
                        .position(|slot| slot.item.content_id() == item.content_id())
                        .unwrap_or_else(|| {
                            eligible
                                .iter()
                                .take(t)
                                .count()
                                .min(eligible.len().saturating_sub(1))
                        });
                    let headless = eligible.iter().all(|slot| slot.item.is_audio());
                    let submitted = self.player.submit_queue_slots(
                        eligible,
                        start_idx,
                        self.queue_source.clone(),
                        self.emby_snapshot().map(Arc::new),
                        headless,
                        self.ui_volume,
                    );
                    if !submitted && self.player.is_remote_disconnected() {
                        self.flash(
                            super::actions::CONNECTION_LOST_MESSAGE.into(),
                            ToastSeverity::Warning,
                        );
                    }
                    if submitted {
                        self.stamp_queue_generation(scope);
                    }
                }
            }

            Command::Quit => return self.try_quit(),
            Command::NextLibraryTab => self.library_tab_next(),
            Command::PreviousLibraryTab => self.library_tab_prev(),
            Command::SetLibraryTab(index) => {
                if index < self.tab_count() {
                    self.set_library_tab(index);
                }
            }
            Command::ForceClear => self.force_clear = true,
            Command::RequestClearQueue => self.request_clear_queue(),
            Command::RefreshCurrentView => self.refresh_current_view(),
            Command::ToggleSettings => self.request_sidebar_toggle(super::SidebarId::Settings),
            Command::OpenSessions => self.request_sidebar_toggle(super::SidebarId::Sessions),
            Command::OpenPlaylists => self.open_playlists_panel(),
            Command::OpenSearch => self.open_search_sidebar(),
            // Model handles this shell-only command before delegating the
            // remaining commands to App::dispatch.
            Command::OpenHelp => unreachable!("OpenHelp is dispatched by Model"),
            Command::FocusPanel(focus) => {
                self.set_panel_focus(focus);
                // No card-checkpoint reset: `last_card_*` is the measured
                // image geometry and focus does not change the artwork
                // (hide-queue-visuals-when-idle design). Clearing it here
                // made every focus switch reserve the fallback rectangle
                // for a frame -- the now-playing panel visibly grew then
                // shrank between image and seekbar.
            }

            Command::CyclePanelMode => {
                // Narrow terminal (< MINI_VIEW_THRESHOLD columns): mini view
                // toggles exactly two states, library-only ⇄ queue-only.
                if self.terminal_width < super::MINI_VIEW_THRESHOLD {
                    self.mini_view_focus = match self.mini_view_focus {
                        super::PanelFocus::Library => super::PanelFocus::Queue,
                        super::PanelFocus::Queue => super::PanelFocus::Library,
                    };
                    if matches!(self.mini_view_focus, super::PanelFocus::Queue) {
                        self.focus_queue_initial_item();
                    }
                } else {
                    self.panel_mode = match self.panel_mode {
                        super::PanelMode::Both => super::PanelMode::QueueOnly,
                        super::PanelMode::QueueOnly => super::PanelMode::LibraryOnly,
                        super::PanelMode::LibraryOnly => super::PanelMode::Both,
                    };
                    match self.panel_mode {
                        super::PanelMode::LibraryOnly => {
                            if matches!(self.panel_focus, super::PanelFocus::Queue) {
                                self.set_panel_focus(super::PanelFocus::Library);
                            }
                        }
                        super::PanelMode::QueueOnly => {
                            self.set_panel_focus(super::PanelFocus::Queue);
                        }
                        super::PanelMode::Both => {}
                    }
                }
            }
        }
        false
    }
}

#[cfg(test)]
#[path = "action_tests.rs"]
mod tests;
