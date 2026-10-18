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
use crossterm::event::KeyCode;
use mbv_ctrl::Direction;
use mbv_ctrl::player::PlayerCommand;
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
    /// available subtitle tracks (plus "off") for local playback (#86). The
    /// local-idle fallback (cycling the default subtitle *mode* when there's
    /// no active player) still lives inside `cycle_sub()`.
    CycleOrToggleSubtitle,
    AdjustVolume(i64),
    /// The `m` key: flips `mute_on` and sends `PlayerCommand::SetMute`.
    ToggleMute,
    /// The `#` key: cycles the audio track.
    CycleAudio,

    // ── queue activation (issue #134) ───────────────────────────────────
    /// Activate the item at the given queue index: `Enter` on the queue
    /// tab, or a double-click on a queue row (`handle_mouse`'s
    /// `is_double`/queue branch — the two were already made to match in
    /// a70ad7a, before either went through `Command`; this variant is the
    /// single implementation both now share). It seeks to the top if it's
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
pub(in crate::app) struct IdleFeedLinkContext {
    pub player_active: bool,
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
            | Command::CycleAudio => self.dispatch_playback_command(command),
            Command::QueuePlayCursor(t) => self.dispatch_queue_play_cursor(t),
            Command::Quit => return self.try_quit(),
            Command::NextLibraryTab
            | Command::PreviousLibraryTab
            | Command::SetLibraryTab(_)
            | Command::ForceClear
            | Command::RequestClearQueue
            | Command::ToggleSettings
            | Command::OpenSessions
            | Command::OpenPlaylists
            | Command::OpenSearch
            | Command::OpenHelp => self.dispatch_navigation_command(command),
            // Model handles this shell-only command before delegating the
            // remaining commands to App::dispatch: the podcast Latest pill
            // is component-local state, so the refresh carries the owner's
            // pill answer for the lazy destination shelf fetch (spec
            // `destination-latest-modes`, "Destination Latest lists load lazily").
            Command::RefreshCurrentView => {
                unreachable!("RefreshCurrentView is dispatched by Model")
            }
            Command::FocusPanel(_) | Command::CyclePanelMode => {
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
                self.set_visual_slot_hidden(!self.visual_slot_hidden);
            }
            _ => unreachable!("dispatch_visual_command only accepts visual commands"),
        }
    }

    fn dispatch_playback_command(&mut self, command: &Command) {
        // Issue #940: name every dispatched transport command, so a dead
        // input window shows whether the dispatch happened at all.
        tracing::debug!(
            name: "transport.dispatch",
            target: "transport",
            { command = ?command },
            "transport command dispatched"
        );
        match *command {
            Command::TogglePlayPause => crate::app::LocalPlaybackTarget::toggle_play_pause(self),
            Command::Stop => crate::app::LocalPlaybackTarget::stop(self),
            Command::SeekRelative(delta) => {
                crate::app::LocalPlaybackTarget::seek_relative(self, delta);
            }
            Command::NextTrack => crate::app::LocalPlaybackTarget::jump_track(self, 1),
            Command::PreviousTrack => crate::app::LocalPlaybackTarget::jump_track(self, -1),
            Command::CycleOrToggleSubtitle => {
                self.cycle_sub();
            }
            Command::AdjustVolume(delta) => {
                self.adjust_volume(delta);
            }
            Command::ToggleMute => crate::app::LocalPlaybackTarget::toggle_command_mute(self),
            Command::CycleAudio => self.cycle_audio(),
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
            Command::ToggleSettings => {
                self.request_sidebar_toggle(mbv_ui_model::overlay::SidebarId::Settings);
            }
            Command::OpenSessions => {
                self.request_sidebar_toggle(mbv_ui_model::overlay::SidebarId::Sessions);
            }
            Command::OpenPlaylists => {
                self.request_sidebar_toggle(mbv_ui_model::overlay::SidebarId::Playlists);
            }
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
            _ => unreachable!("dispatch_panel_command only accepts panel commands"),
        }
    }

    /// `panel_mode_cycle_x` while pinned (change `pinned-view-toggle`, design
    /// D2/D3): flip the running panel between its two pinned views, one
    /// `set-size` request per press. The expanded width shows library-only
    /// with library focus; the collapsed width shows queue-only with queue
    /// focus and re-runs the mini-view initial-item path. A refused target
    /// width keeps the current width and flashes the reason.
    /// `cycle_panel_mode` guards pinned-ness (D1), so the not-pinned arm here
    /// is unreachable.
    pub(crate) fn pinned_view_toggle(&mut self) {
        let width = self.pinned_width.toggled();
        if let Some(result) = self.request_pinned_width(width) {
            self.apply_pinned_view(width, result);
        }
    }

    /// Ask the running panel for `width`; `None` when this launch is not
    /// pinned. The config is cloned out first so callers can mutate `self`
    /// after the request.
    fn request_pinned_width(&self, width: crate::pin::PinnedWidth) -> Option<Result<(), String>> {
        if !crate::pin::is_pinned() {
            return None;
        }
        let config = self.config.lock().unwrap().panel.clone();
        Some(crate::pin::apply_pinned_width(&config, width))
    }

    /// Take an accepted target width and assign the matching pinned view, or
    /// flash a refused request's reason and touch nothing (design D2/D3).
    ///
    /// The view fields are written directly: at dispatch time `terminal_width`
    /// still holds the pre-toggle width (the terminal's `Resize` event arrives
    /// later), so on expand `set_panel_focus` would take its mini-view early
    /// return and leave the stored `panel_focus` stale.
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
        if crate::pin::is_pinned() {
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
        let (all_slots, slot_id) = self.queue_play_snapshot(t);
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

    fn queue_play_snapshot(&self, t: usize) -> (Vec<mbv_queue::ExecSlot>, Option<QueueSlotId>) {
        // Snapshot data from the queue before any mutable borrows.
        let queue = self.displayed_queue();
        let all_slots = queue.slot_pairs();
        let slot_id = queue.slot_id_at(t);
        (all_slots, slot_id)
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
