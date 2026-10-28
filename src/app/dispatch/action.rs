//! Command seam between key-event translation (`input.rs`) and effects
//! (`actions.rs`, `player.rs`). See issue #78.
//!
//! Router-owned key resolution lives in the central keyboard policy
//! (`key_policy.rs`): the split transport actions match their configured
//! chords there and bind their fixed payloads in `command_for_policy`. This
//! module keeps the `Command` variants, the idle-feed link's availability
//! condition, and `dispatch`'s state transitions.
//!
//! `Command` is a two-level enum: the outer level says who owns the command
//! (the shell `Model` or the `App`), and the inner level groups the `App`
//! commands by domain. Each domain owns one typed intent enum, so
//! `App::dispatch` has one arm per domain and each sub-dispatcher matches
//! its own intent exhaustively.
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

/// A UI command: the seam between key-event translation (`input.rs`) and
/// effects (`actions.rs`, `player.rs`).
///
/// The outer level says who owns the command: the shell `Model` handles
/// `Shell`, the `App` handles `App`. The inner level groups the `App`
/// commands by domain, so each sub-dispatcher matches one typed intent
/// instead of re-grouping a flat variant list.
#[derive(Debug, Clone, PartialEq)]
pub(in crate::app) enum Command {
    /// Commands only the shell `Model` handles.
    Shell(ShellCommand),
    /// Commands the `App` dispatches to its own state transitions.
    App(AppCommand),
}

/// The commands the shell `Model` handles before the `App` sees them. The
/// `App` cannot receive one, so it needs no arm for it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(in crate::app) enum ShellCommand {
    /// Model-owned because mounting Help belongs to the `TuiRealm` shell.
    OpenHelp,
    /// The podcast Latest pill is component-local state, so the refresh
    /// carries the owner's pill answer for the lazy destination shelf fetch
    /// (spec `destination-latest-modes`, "Destination Latest lists load
    /// lazily").
    RefreshCurrentView,
}

/// The commands the `App` dispatches to its own state transitions. The
/// domain grouping is the type: each arm of `App::dispatch` names one
/// domain, and each sub-dispatcher matches one typed intent.
#[derive(Debug, Clone, PartialEq)]
pub(in crate::app) enum AppCommand {
    Visual(VisualIntent),
    Playback(PlaybackIntent),
    Navigation(NavigationIntent),
    Panel(PanelIntent),
    /// Activate the item at the given queue index: `Enter` on the queue
    /// tab, or a double-click on a queue row (`handle_mouse`'s
    /// `is_double`/queue branch — the two were already made to match in
    /// a70ad7a, before either went through `Command`; this variant is the
    /// single implementation both now share). It seeks to the top if it's
    /// the already-playing audio item, jumps to it if it's elsewhere in the
    /// active playback queue, or replaces the local playback queue and plays
    /// from this index if the visible queue isn't the one currently playing.
    /// The target index is carried explicitly: the shell resolves the slot the
    /// user selected and passes it
    /// rather than this command re-reading `queue.cursor()` as an
    /// ambient argument channel.
    QueuePlayCursor(usize),
    /// Quit the client through the normal dirty-queue/prefs shutdown path.
    Quit,
}

/// The visual (non-playback) intents: the idle-feed link and the visualizer
/// slot.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(in crate::app) enum VisualIntent {
    OpenIdleFeedLink,
    ToggleVisualizer,
    ToggleVisualSlotHidden,
}

/// The playback intents.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(in crate::app) enum PlaybackIntent {
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
}

/// The destination-independent routing intents.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(in crate::app) enum NavigationIntent {
    NextLibraryTab,
    PreviousLibraryTab,
    SetLibraryTab(usize),
    ForceClear,
    /// Raise the "Clear queue?" confirmation prompt (the global `c` binding).
    RequestClearQueue,
    ToggleSettings,
    OpenSessions,
    OpenPlaylists,
    OpenSearch,
}

/// The panel intents.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(in crate::app) enum PanelIntent {
    /// `x`: cycle the Power View layout through both, queue-only, and
    /// library-only (see `PanelMode`); below the mini-view threshold it
    /// toggles queue-only and library-only.
    CyclePanelMode,
    FocusPanel(mbv_ui_model::settings::PanelFocus),
}

impl From<ShellCommand> for Command {
    fn from(command: ShellCommand) -> Self {
        Command::Shell(command)
    }
}

impl From<AppCommand> for Command {
    fn from(command: AppCommand) -> Self {
        Command::App(command)
    }
}

impl From<VisualIntent> for Command {
    fn from(intent: VisualIntent) -> Self {
        Command::App(AppCommand::Visual(intent))
    }
}

impl From<PlaybackIntent> for Command {
    fn from(intent: PlaybackIntent) -> Self {
        Command::App(AppCommand::Playback(intent))
    }
}

impl From<NavigationIntent> for Command {
    fn from(intent: NavigationIntent) -> Self {
        Command::App(AppCommand::Navigation(intent))
    }
}

impl From<PanelIntent> for Command {
    fn from(intent: PanelIntent) -> Self {
        Command::App(AppCommand::Panel(intent))
    }
}

/// Resolve the idle-feed link shortcut. The link's eligibility is its own
/// condition, separate from the transport bindings (`open_idle_feed_link`
/// keeps its own gate), so `o` remains available to the view
/// when no link is displayed. A daemon-backed
/// player is still an idle feed view when no Emby session is connected, so the
/// playback backend and connected-session gates stay separate here. The
/// `playback_panel_present_idle` input is the Queue playback panel's presence
/// with nothing playing: the panel is mounted in every
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
            Some(VisualIntent::OpenIdleFeedLink.into())
        }
        // A key-chord matcher, not a request dispatch: the wildcard is every
        // key that is not the bound chord.
        // ast-grep-ignore: mbv-dispatch-no-wildcard-arm
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
        // The jump is an answered owner operation, so
        // the playing slot changes only through the owner's answer.
        let sent = self.queue_op(
            self.playing_queue_link(),
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

    /// Own the state transitions for an `AppCommand`. Returns whether the app
    /// should quit (`true` only for `AppCommand::Quit`'s non-prompting path;
    /// `false` for every other variant).
    ///
    /// For most playback variants this means picking a remote-session
    /// command vs. a local `Player` command, matching the divergent behavior
    /// `handle_playback_key` had inline (including its known bugs — see issue
    /// #78 follow-up).
    pub(in crate::app) fn dispatch(&mut self, command: &AppCommand) -> bool {
        match command {
            AppCommand::Visual(intent) => self.dispatch_visual_command(*intent),
            AppCommand::Playback(intent) => self.dispatch_playback_command(*intent),
            AppCommand::Navigation(intent) => self.dispatch_navigation_command(*intent),
            AppCommand::Panel(intent) => self.dispatch_panel_command(*intent),
            AppCommand::QueuePlayCursor(index) => self.dispatch_queue_play_cursor(*index),
            AppCommand::Quit => return self.try_quit(),
        }
        false
    }

    fn dispatch_visual_command(&mut self, intent: VisualIntent) {
        match intent {
            VisualIntent::OpenIdleFeedLink => self.open_idle_feed_link(),
            VisualIntent::ToggleVisualizer => self.toggle_visualizer(),
            VisualIntent::ToggleVisualSlotHidden => {
                self.set_visual_slot_hidden(!self.visual_slot_hidden);
            }
        }
    }

    fn dispatch_playback_command(&mut self, intent: PlaybackIntent) {
        // Issue #940: name every dispatched transport command, so a dead
        // input window shows whether the dispatch happened at all.
        tracing::debug!(
            name: "transport.dispatch",
            target: "transport",
            { command = ?intent },
            "transport command dispatched"
        );
        match intent {
            PlaybackIntent::TogglePlayPause => {
                crate::app::LocalPlaybackTarget::toggle_play_pause(self);
            }
            PlaybackIntent::Stop => crate::app::LocalPlaybackTarget::stop(self),
            PlaybackIntent::SeekRelative(delta) => {
                crate::app::LocalPlaybackTarget::seek_relative(self, delta);
            }
            PlaybackIntent::NextTrack => crate::app::LocalPlaybackTarget::jump_track(self, 1),
            PlaybackIntent::PreviousTrack => crate::app::LocalPlaybackTarget::jump_track(self, -1),
            PlaybackIntent::CycleOrToggleSubtitle => {
                self.cycle_sub();
            }
            PlaybackIntent::AdjustVolume(delta) => {
                self.adjust_volume(delta);
            }
            PlaybackIntent::ToggleMute => {
                crate::app::LocalPlaybackTarget::toggle_command_mute(self);
            }
            PlaybackIntent::CycleAudio => self.cycle_audio(),
        }
    }

    fn dispatch_navigation_command(&mut self, intent: NavigationIntent) {
        match intent {
            NavigationIntent::NextLibraryTab => self.library_tab_next(),
            NavigationIntent::PreviousLibraryTab => self.library_tab_prev(),
            NavigationIntent::SetLibraryTab(index) => {
                if index < self.tab_count() {
                    self.set_library_tab(index);
                }
            }
            NavigationIntent::ForceClear => self.force_clear = true,
            NavigationIntent::RequestClearQueue => self.request_clear_queue(),
            NavigationIntent::ToggleSettings => {
                self.request_sidebar_toggle(mbv_ui_model::overlay::SidebarId::Settings);
            }
            NavigationIntent::OpenSessions => {
                self.request_sidebar_toggle(mbv_ui_model::overlay::SidebarId::Sessions);
            }
            NavigationIntent::OpenPlaylists => {
                self.request_sidebar_toggle(mbv_ui_model::overlay::SidebarId::Playlists);
            }
            NavigationIntent::OpenSearch => self.open_search_sidebar(),
        }
    }

    fn dispatch_panel_command(&mut self, intent: PanelIntent) {
        match intent {
            PanelIntent::FocusPanel(focus) => {
                self.set_panel_focus(focus);
                // No card-checkpoint reset: `last_card_*` is the measured
                // image geometry and focus does not change the artwork
                // (hide-queue-visuals-when-idle design). Clearing it here
                // made every focus switch reserve the fallback rectangle
                // for a frame -- the now-playing panel visibly grew then
                // shrank between image and seekbar.
            }
            PanelIntent::CyclePanelMode => self.cycle_panel_mode(),
        }
    }

    /// `panel_mode_cycle_x` while pinned: flip the running panel between its
    /// two pinned views, one
    /// `set-size` request per press. The expanded width shows library-only
    /// with library focus; the collapsed width shows queue-only with queue
    /// focus and re-runs the mini-view initial-item path. A refused target
    /// width keeps the current width and flashes the reason.
    /// `cycle_panel_mode` guards pinned-ness, so the not-pinned arm here
    /// is unreachable.
    pub(crate) fn pinned_view_toggle(&mut self) {
        self.request_pinned_view(self.pinned_width.toggled());
    }

    /// Send one pinned view `set-size` request for `width` and hand the
    /// pending record its outcome; on a synchronous refusal, apply the
    /// rejection at once. Shared by `pinned_view_toggle` (the `x` toggle,
    /// target `pinned_width.toggled()`) and
    /// [`App::collapse_pinned_view_after_play`] (target `Collapsed`).
    fn request_pinned_view(&mut self, width: mbv_ui_model::settings::PinnedWidth) {
        // One request in flight: a press while pinwin is still working is
        // ignored, with no queue (design D4/D6).
        if self.pinned_request_pending.is_some() {
            return;
        }
        let config = self.config.lock().unwrap().panel.clone();
        let pending = crate::app::state::pinned_request::PinnedRequestPending::View(width);
        if let Err(reason) = self.begin_pinned_request(
            pending,
            &config,
            crate::pin::PinnedSettingRequest::Width(width),
        ) {
            self.apply_pinned_view(width, Err(reason));
        }
    }

    /// Collapse the pinned panel to its queue view after a user tail's
    /// accepted queue replace. Every user tail that starts playback calls
    /// this after the accepted replace — `play_items_routed`,
    /// `play_series_continuation`, `play_single_item`, `submit_queue_item`
    /// (its `start_playback` arm), and `start_pending_queue_playback`; a new
    /// user play tail must too. The websocket Play handler never does
    /// (pinned-play-collapses-to-queue design D1). Only the expanded width
    /// collapses, so the collapsed-width and not-pinned plays do nothing;
    /// the pending guard lives in [`App::request_pinned_view`], which caps
    /// a multi-item play at one request (design D4).
    pub(in crate::app) fn collapse_pinned_view_after_play(&mut self) {
        if self.pinned_width == mbv_ui_model::settings::PinnedWidth::Expanded {
            self.request_pinned_view(mbv_ui_model::settings::PinnedWidth::Collapsed);
        }
    }

    /// Take an accepted target width and assign the matching pinned view, or
    /// flash a refused request's reason and touch nothing.
    ///
    /// The view fields are written directly: at dispatch time `terminal_width`
    /// still holds the pre-toggle width (the terminal's `Resize` event arrives
    /// later), so on expand `set_panel_focus` would take its mini-view early
    /// return and leave the stored `panel_focus` stale.
    pub(in crate::app) fn apply_pinned_view(
        &mut self,
        width: mbv_ui_model::settings::PinnedWidth,
        applied: Result<(), String>,
    ) {
        if let Err(reason) = applied {
            self.flash(reason, ToastSeverity::Warning);
            return;
        }
        self.note_pinned_width_applied(width);
        let (mode, focus) = match width {
            mbv_ui_model::settings::PinnedWidth::Expanded => (
                mbv_ui_model::settings::PanelMode::LibraryOnly,
                mbv_ui_model::settings::PanelFocus::Library,
            ),
            mbv_ui_model::settings::PinnedWidth::Collapsed => (
                mbv_ui_model::settings::PanelMode::QueueOnly,
                mbv_ui_model::settings::PanelFocus::Queue,
            ),
        };
        self.panel_mode = mode;
        self.mini_view_focus = focus;
        self.panel_focus = focus;
        if width == mbv_ui_model::settings::PinnedWidth::Collapsed {
            self.focus_queue_initial_item();
        }
    }

    fn cycle_panel_mode(&mut self) {
        // A pinned panel binds `x` to its two view states; the three-state
        // cycle and the
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
    /// Own the `AppCommand::QueuePlayCursor` state transitions (extracted from
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
            active && self.viewed_queue_link() == self.playing_queue_link() && t != current_idx;
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
        let link = self.viewed_queue_link();
        let st = self.player.status_snapshot();
        let active = st.active;
        let current_idx = st.current_idx;
        if active && self.queue_link_is_playback(link) {
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
