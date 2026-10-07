//! Command seam between key-event translation (`input.rs`) and effects
//! (`actions.rs`, `player.rs`). See issue #78.
//!
//! Router-owned key resolution lives in the central keyboard policy
//! (`key_policy.rs`): the split transport actions match their configured
//! chords there and bind their fixed payloads in `command_for_policy`. This
//! module keeps the `Command` variants, the idle-feed link's availability
//! condition, and `dispatch`'s state transitions.
//!
//! The help overlay was converted to a `TuiRealm` Interactive Component
//! (`crates/mbv-components/src/help.rs`) and no longer routes through this `Command`
//! enum. Other modal handlers still speak directly to `App` and are expected to
//! migrate to this same `Command` enum over time, one handler at a time.

use crate::app::App;
use crate::app::dispatch::notify::ToastSeverity;
use crate::app::dispatch::queue::QueueOpEdit;
use crate::app::input::resolver::KeyChord;
#[cfg(test)]
use crate::app::tests::QueueViewTestExt;
use crossterm::event::KeyCode;
use mbv_ctrl::Direction;
use mbv_ctrl::player::PlayerCommand;
use mbv_emby_model::EmbyItem;
use mbv_queue::QueueSlotId;

#[derive(Debug, Clone, PartialEq)]
pub(in crate::app) enum Command {
    OpenIdleFeedLink,
    ToggleVisualizer,
    ToggleVisualSlotHidden,
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
    /// rather than this command re-reading `queue.cursor()` as an
    /// ambient argument channel.
    QueuePlayCursor(usize),

    /// `x`: cycle the Power View layout through both, queue-only, and
    /// library-only (see `PanelMode`); below the mini-view threshold it
    /// toggles queue-only and library-only.
    CyclePanelMode,

    /// `pinned_width_toggle`: flip the running pinned panel between its
    /// collapsed (`cols`) and expanded (`cols_expanded`) saved widths
    /// (change `panel-expand-toggle`, design D3).
    TogglePinnedWidth,

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
    /// Model-owned because mounting Help belongs to the `TuiRealm` shell.
    OpenHelp,
    FocusPanel(mbv_ui_model::settings::PanelFocus),
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
#[expect(
    clippy::struct_excessive_bools,
    reason = "four independently-derived idle-feed gate facts (player active, connected session, idle panel presence, link availability); the command fires only when all activity gates are false AND the link is available; any combination is reachable (design analysis, issue #804)"
)]
pub(in crate::app) struct IdleFeedLinkContext {
    pub player_active: bool,
    pub has_connected_session: bool,
    pub playback_panel_present_idle: bool,
    pub link_available: bool,
}

pub(in crate::app) fn idle_feed_command_for_key(
    chord: KeyChord,
    context: &IdleFeedLinkContext,
) -> Option<Command> {
    match chord.code {
        KeyCode::Char('o')
            if chord.mods.is_empty()
                && !context.player_active
                && !context.has_connected_session
                && !context.playback_panel_present_idle
                && context.link_available =>
        {
            Some(Command::OpenIdleFeedLink)
        }
        _ => None,
    }
}

impl App {
    fn reject_disconnected_remote_jump(&mut self) -> bool {
        if !self.player.is_remote_disconnected() {
            return false;
        }
        self.handle_player_event(mbv_ctrl::player::PlayerEvent::CommandRejected(
            crate::app::dispatch::actions::CONNECTION_LOST_MESSAGE.to_string(),
        ));
        true
    }

    fn request_remote_slot_jump(&mut self, slot_id: QueueSlotId) -> bool {
        if self.reject_disconnected_remote_jump() {
            return false;
        }
        // Row 5.3 (design D6): the jump is an answered owner operation, so
        // the playing slot changes only through the owner's answer.
        let sent = self.queue_op(
            self.playing_queue_scope(),
            mbv_remote_player::QueueOp::PlaySlot {
                slot_id: mbv_ctrl::slot_id_to_u64(slot_id),
            },
        );
        sent != QueueOpEdit::NotApplied
    }

    pub(in crate::app) fn request_relative_step(&mut self, direction: Direction) -> bool {
        match direction {
            Direction::Next => self.player.next(),
            Direction::Previous => self.player.previous(),
        }
    }

    pub(in crate::app) fn request_slot_jump(&mut self, slot_id: QueueSlotId) -> bool {
        self.request_remote_slot_jump(slot_id)
    }

    /// Own the state transitions for a `Command`. Returns whether the app
    /// should quit (`true` only for `Command::Quit`'s non-prompting path;
    /// `false` for every other variant).
    ///
    /// For most playback variants this means picking a remote-session
    /// command vs. a local `Player` command, matching the divergent behavior
    /// `handle_playback_key` had inline (including its known bugs — see issue
    /// #78 follow-up).
    pub(in crate::app) fn dispatch(&mut self, command: &Command) -> bool {
        match *command {
            Command::OpenIdleFeedLink
            | Command::ToggleVisualizer
            | Command::ToggleVisualSlotHidden => self.dispatch_visual_command(command),
            Command::TogglePlayPause
            | Command::Stop
            | Command::SeekRelative(_)
            | Command::NextTrack
            | Command::PreviousTrack
            | Command::CycleOrToggleSubtitle
            | Command::AdjustVolume(_)
            | Command::ToggleMute
            | Command::ToggleMuteOrCycleAudio => self.dispatch_playback_command(command),
            Command::QueuePlayCursor(t) => self.dispatch_queue_play_cursor(t),
            Command::Quit => return self.try_quit(),
            Command::NextLibraryTab
            | Command::PreviousLibraryTab
            | Command::SetLibraryTab(_)
            | Command::ForceClear
            | Command::RequestClearQueue
            | Command::RefreshCurrentView
            | Command::ToggleSettings
            | Command::OpenSessions
            | Command::OpenPlaylists
            | Command::OpenSearch
            | Command::OpenHelp => self.dispatch_navigation_command(command),
            Command::FocusPanel(_) | Command::CyclePanelMode | Command::TogglePinnedWidth => {
                self.dispatch_panel_command(command);
            }
        }
        false
    }

    fn dispatch_visual_command(&mut self, command: &Command) {
        match *command {
            Command::OpenIdleFeedLink => self.open_idle_feed_link(),
            Command::ToggleVisualizer => self.toggle_visualizer(),
            Command::ToggleVisualSlotHidden => {
                self.visual_slot_hidden = !self.visual_slot_hidden;
                self.sync_visualizer();
                self.save_prefs();
            }
            _ => unreachable!("dispatch_visual_command only accepts visual commands"),
        }
    }

    fn dispatch_playback_command(&mut self, command: &Command) {
        match *command {
            Command::TogglePlayPause => self.playback_target().toggle_play_pause(self),
            Command::Stop => self.playback_target().stop(self),
            Command::SeekRelative(delta) => self.playback_target().seek_relative(self, delta),
            Command::NextTrack => self.playback_target().jump_track(self, 1, "NextTrack"),
            Command::PreviousTrack => self.playback_target().jump_track(self, -1, "PreviousTrack"),
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
            Command::ToggleMute => self.playback_target().toggle_command_mute(self),
            Command::ToggleMuteOrCycleAudio => {
                if self.is_audio_item() {
                    self.toggle_mute();
                } else {
                    self.cycle_audio();
                }
            }
            _ => unreachable!("dispatch_playback_command only accepts playback commands"),
        }
    }

    fn dispatch_navigation_command(&mut self, command: &Command) {
        match *command {
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
            Command::ToggleSettings => {
                self.request_sidebar_toggle(mbv_ui_model::overlay::SidebarId::Settings);
            }
            Command::OpenSessions => {
                self.request_sidebar_toggle(mbv_ui_model::overlay::SidebarId::Sessions);
            }
            Command::OpenPlaylists => self.open_playlists_panel(),
            Command::OpenSearch => self.open_search_sidebar(),
            // Model handles this shell-only command before delegating the
            // remaining commands to App::dispatch.
            Command::OpenHelp => unreachable!("OpenHelp is dispatched by Model"),
            _ => unreachable!("dispatch_navigation_command only accepts navigation commands"),
        }
    }

    fn dispatch_panel_command(&mut self, command: &Command) {
        match *command {
            Command::FocusPanel(focus) => {
                self.set_panel_focus(focus);
                // No card-checkpoint reset: `last_card_*` is the measured
                // image geometry and focus does not change the artwork
                // (hide-queue-visuals-when-idle design). Clearing it here
                // made every focus switch reserve the fallback rectangle
                // for a frame -- the now-playing panel visibly grew then
                // shrank between image and seekbar.
            }
            Command::CyclePanelMode => self.cycle_panel_mode(),
            Command::TogglePinnedWidth => self.toggle_pinned_width(),
            _ => unreachable!("dispatch_panel_command only accepts panel commands"),
        }
    }

    /// `pinned_width_toggle` (change `panel-expand-toggle`, design D3): flip
    /// the running panel to the other saved `[panel]` width. A launch without
    /// a panel says so and changes nothing; the running panel validates the
    /// other width first, and a rejected layout keeps the current width and
    /// shows the reason as a Warning toast.
    pub(crate) fn toggle_pinned_width(&mut self) {
        let width = self.pinned_width.toggled();
        match self.apply_pinned_layout(width) {
            None => self.flash(
                "Width toggle needs a pinned launch (mbv --pin)".into(),
                ToastSeverity::Neutral,
            ),
            Some(Ok(())) => self.note_pinned_width_applied(width),
            Some(Err(reason)) => self.flash(reason, ToastSeverity::Warning),
        }
    }

    /// `panel_mode_cycle_x` while pinned (change `pinned-view-toggle`, design
    /// D2/D3): flip the running panel between its two pinned views through the
    /// same resize path as the width toggle. The expanded width shows
    /// library-only with library focus; the collapsed width shows queue-only
    /// with queue focus and re-runs the mini-view initial-item path. A
    /// rejected target width keeps the current width and flashes the reason.
    /// `cycle_panel_mode` guards pinned-ness (D1), so the not-pinned arm here
    /// is unreachable.
    pub(crate) fn pinned_view_toggle(&mut self) {
        let width = self.pinned_width.toggled();
        if let Some(result) = self.apply_pinned_layout(width) {
            self.apply_pinned_view(width, result);
        }
    }

    /// Resize the pinned panel to `width`; `None` when not pinned. The panic-free
    /// `map` keeps the `pinned_panel` borrow local so callers can mutate `self`.
    fn apply_pinned_layout(&self, width: crate::pin::PinnedWidth) -> Option<Result<(), String>> {
        let config = self.config.lock().unwrap().panel;
        self.pinned_panel
            .as_ref()
            .map(|panel| crate::pin::apply_layout(panel, &config, width))
    }

    /// Take an accepted target width and assign the matching pinned view, or
    /// flash a rejected layout's reason and touch nothing (design D2/D3).
    ///
    /// The view fields are written directly: at dispatch time `terminal_width`
    /// still holds the pre-toggle width (the pty resize is only observed a
    /// frame later through `pinned_resize_pending`), so on expand
    /// `set_panel_focus` would take its mini-view early return and leave the
    /// stored `panel_focus` stale.
    pub(in crate::app) fn apply_pinned_view(
        &mut self,
        width: crate::pin::PinnedWidth,
        applied: Result<(), String>,
    ) {
        if let Err(reason) = applied {
            self.flash(reason, ToastSeverity::Warning);
            return;
        }
        self.note_pinned_width_applied(width);
        let (mode, focus) = match width {
            crate::pin::PinnedWidth::Expanded => (
                mbv_ui_model::settings::PanelMode::LibraryOnly,
                mbv_ui_model::settings::PanelFocus::Library,
            ),
            crate::pin::PinnedWidth::Collapsed => (
                mbv_ui_model::settings::PanelMode::QueueOnly,
                mbv_ui_model::settings::PanelFocus::Queue,
            ),
        };
        self.panel_mode = mode;
        self.mini_view_focus = focus;
        self.panel_focus = focus;
        if width == crate::pin::PinnedWidth::Collapsed {
            self.focus_queue_initial_item();
        }
    }

    fn cycle_panel_mode(&mut self) {
        // A pinned panel binds `x` to its two view states (change
        // `pinned-view-toggle`, design D1/D2); the three-state cycle and the
        // mini-view toggle are unreachable while pinned.
        if self.pinned_panel.is_some() {
            self.pinned_view_toggle();
            return;
        }
        // Narrow terminal (< MINI_VIEW_THRESHOLD columns): mini view toggles
        // exactly two states, library-only ⇄ queue-only.
        if self.terminal_width < mbv_render::layout::MINI_VIEW_THRESHOLD {
            self.mini_view_focus = match self.mini_view_focus {
                mbv_ui_model::settings::PanelFocus::Library => {
                    mbv_ui_model::settings::PanelFocus::Queue
                }
                mbv_ui_model::settings::PanelFocus::Queue => {
                    mbv_ui_model::settings::PanelFocus::Library
                }
            };
            if matches!(
                self.mini_view_focus,
                mbv_ui_model::settings::PanelFocus::Queue
            ) {
                self.focus_queue_initial_item();
            }
        } else {
            self.cycle_wide_panel_mode();
        }
    }

    fn cycle_wide_panel_mode(&mut self) {
        self.panel_mode = match self.panel_mode {
            mbv_ui_model::settings::PanelMode::Both => mbv_ui_model::settings::PanelMode::QueueOnly,
            mbv_ui_model::settings::PanelMode::QueueOnly => {
                mbv_ui_model::settings::PanelMode::LibraryOnly
            }
            mbv_ui_model::settings::PanelMode::LibraryOnly => {
                mbv_ui_model::settings::PanelMode::Both
            }
        };
        match self.panel_mode {
            mbv_ui_model::settings::PanelMode::LibraryOnly => {
                if matches!(self.panel_focus, mbv_ui_model::settings::PanelFocus::Queue) {
                    self.set_panel_focus(mbv_ui_model::settings::PanelFocus::Library);
                }
            }
            mbv_ui_model::settings::PanelMode::QueueOnly => {
                self.set_panel_focus(mbv_ui_model::settings::PanelFocus::Queue);
            }
            mbv_ui_model::settings::PanelMode::Both => {}
        }
    }
    /// Own the `Command::QueuePlayCursor` state transitions (extracted from
    /// `dispatch`; every early exit there returned `false`, i.e. fell through
    /// to dispatch's own `false` tail).
    pub(in crate::app) fn dispatch_queue_play_cursor(&mut self, t: usize) {
        let Some(item) = self.queue_play_cursor_item(t) else {
            return;
        };
        if self.handle_disconnected_queue_play(t) {
            return;
        }
        if !self.validate_queue_play_feed(&item) {
            return;
        }
        let (emby_items, all_slots, slot_id, emby_start) = self.queue_play_snapshot(t);
        if self.handoff_queue_play_to_session(&item, &emby_items, emby_start) {
            return;
        }
        self.play_queue_cursor_locally(t, &item, slot_id, all_slots);
    }

    fn queue_play_cursor_item(&mut self, t: usize) -> Option<mbv_queue::QueueItem> {
        let queue = self.displayed_queue();
        if t >= queue.total_queue_len() {
            return None;
        }
        // Validate the item at the cursor exists.
        let item = queue.item_at(t).cloned()?;
        let owner_can_admit_audiobookshelf = self.player.can_admit_audiobookshelf();
        if !item.admissible_for_owner_with_audiobookshelf(
            false,
            |service| {
                service != mbv_queue::ServiceKind::Audiobookshelf || owner_can_admit_audiobookshelf
            },
            owner_can_admit_audiobookshelf,
        ) {
            self.flash(
                "Playback owner rejected this Audiobookshelf item".into(),
                crate::app::dispatch::notify::ToastSeverity::Error,
            );
            return None;
        }
        Some(item)
    }

    fn handle_disconnected_queue_play(&mut self, t: usize) -> bool {
        if !self.player.is_remote_disconnected() {
            return false;
        }
        let status = self.player.status_snapshot();
        let active = status.active;
        let current_idx = status.current_idx;
        drop(status);
        let queue = self.displayed_queue();
        let is_jump =
            active && self.viewed_queue_scope() == self.playing_queue_scope() && t != current_idx;
        if is_jump {
            if let Some(slot_id) = queue.slot_id_at(t) {
                let _ = self.request_slot_jump(slot_id);
            } else {
                self.handle_player_event(mbv_ctrl::player::PlayerEvent::CommandRejected(
                    crate::app::dispatch::actions::CONNECTION_LOST_MESSAGE.to_string(),
                ));
            }
        } else {
            self.flash(
                crate::app::dispatch::actions::CONNECTION_LOST_MESSAGE.into(),
                ToastSeverity::Warning,
            );
        }
        true
    }

    fn validate_queue_play_feed(&mut self, item: &mbv_queue::QueueItem) -> bool {
        // Validate source for Feed entries early.
        if let mbv_queue::QueueItem::Feed(entry) = item
            && entry.primary_source().is_none()
        {
            self.flash(
                "Feed entry has no playable source".into(),
                crate::app::dispatch::notify::ToastSeverity::Error,
            );
            return false;
        }
        true
    }

    fn queue_play_snapshot(
        &self,
        t: usize,
    ) -> (
        Vec<EmbyItem>,
        Vec<mbv_queue::ExecSlot>,
        Option<QueueSlotId>,
        usize,
    ) {
        // Snapshot data from the queue before any mutable borrows.
        let queue = self.displayed_queue();
        let emby_items = queue
            .slots()
            .iter()
            .filter_map(|slot| slot.item.as_emby().cloned())
            .collect();
        let all_slots = queue.slot_pairs();
        let slot_id = queue.slot_id_at(t);
        // Pre-compute the Emby-only projection index for the cursor
        // position, needed by the session API boundary.
        let emby_start = queue
            .slots()
            .iter()
            .take(t)
            .filter(|slot| slot.item.as_emby().is_some())
            .count();
        (emby_items, all_slots, slot_id, emby_start)
    }

    fn handoff_queue_play_to_session(
        &mut self,
        item: &mbv_queue::QueueItem,
        emby_items: &[EmbyItem],
        emby_start: usize,
    ) -> bool {
        // Connected remote session: hand off Emby items to the
        // session; Feed entries cannot cross the Emby session API
        // so they fall through to the local/direct-remote path.
        if let mbv_queue::QueueItem::Emby(_) = item
            && let Some(conn_id) = self.connected_session_id.clone()
        {
            let label = item.display_name();
            self.flash(
                format!("Requesting playback: {label}"),
                ToastSeverity::Neutral,
            );
            self.set_queue_scope(self.playing_queue_scope());
            self.submit_attached_sequence(&conn_id, emby_items, emby_start);
            return true;
        }
        false
    }

    fn play_queue_cursor_locally(
        &mut self,
        t: usize,
        item: &mbv_queue::QueueItem,
        slot_id: Option<QueueSlotId>,
        all_slots: Vec<mbv_queue::ExecSlot>,
    ) {
        // Local / direct-remote playback. The same path handles
        // both Feed and Emby items: jump to an active slot or
        // cold-start the full canonical queue.
        let scope = self.viewed_queue_scope();
        let st = self.player.status_snapshot();
        let active = st.active;
        let current_idx = st.current_idx;
        drop(st);
        if active && self.queue_scope_is_playback(scope) {
            if t == current_idx {
                let _ = self.player.send_command(PlayerCommand::SeekAbsolute(0.0));
            } else if t != current_idx {
                let Some(slot_id) = slot_id else {
                    return;
                };
                // One owner-kind seam for every fresh jump, so
                // explicit play and the Next-Up accept cannot diverge.
                if !self.request_slot_jump(slot_id) && !self.player.is_remote_disconnected() {
                    self.flash(
                        "Playback owner rejected the queue selection".into(),
                        ToastSeverity::Error,
                    );
                }
            }
        } else {
            self.cold_start_queue_play(t, item, all_slots);
        }
    }

    fn cold_start_queue_play(
        &mut self,
        t: usize,
        item: &mbv_queue::QueueItem,
        all_slots: Vec<mbv_queue::ExecSlot>,
    ) {
        // Cold start: submit the full canonical queue (all
        // variants) so the player's internal playlist matches
        // the QueueView's queue exactly.
        let owner_can_admit_audiobookshelf = self.player.can_admit_audiobookshelf();
        let eligible: Vec<_> = all_slots
            .into_iter()
            .filter(|slot| {
                slot.item.admissible_for_owner_with_audiobookshelf(
                    false,
                    |service| {
                        service != mbv_queue::ServiceKind::Audiobookshelf
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
            return;
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
        let submitted = self.player.submit_queue_slots(
            eligible,
            start_idx,
            self.playback_queue().source().clone(),
        );
        if !submitted && self.player.is_remote_disconnected() {
            self.flash(
                crate::app::dispatch::actions::CONNECTION_LOST_MESSAGE.into(),
                ToastSeverity::Warning,
            );
        }
    }
}

#[cfg(test)]
mod tests;
