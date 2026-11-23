//! The presentation fixtures a TUI-staying test drives (`split-tui-gui-clients`
//! design D4b rule 5): the pinned view's width and the request it left in
//! flight, the visual slot the GUI Client's capability gate keeps, the Home
//! Latest launch window the markers are read against, a queued owner's
//! transfers, and the music grouping's terminal states.
//!
//! The tests drove those shapes by writing the fields directly; each seam
//! here names the fixture instead — a pinned view in one width, a launch
//! window opened an interval before the launch, a level whose fill already
//! failed — so those tests keep their claims and reach no field.

use std::time::{Duration, Instant};

use super::super::App;
use mbv_queue::FeedEntry;
use mbv_ui_model::settings::PinnedWidth;

impl App {
    /// Hold `width` as the pinned view's width, as the size setting that asked
    /// for it leaves both ends of the toggle.
    #[cfg(any(test, feature = "test"))]
    pub fn seed_pinned_width_for_test(&mut self, width: PinnedWidth) {
        self.pinned_width = width;
    }

    /// The pinned view's width.
    #[cfg(any(test, feature = "test"))]
    #[must_use]
    pub fn pinned_width_for_test(&self) -> PinnedWidth {
        self.pinned_width
    }

    /// Whether a Pinned request is in flight: the one-in-flight rule the
    /// toggle's second press observes.
    #[cfg(any(test, feature = "test"))]
    #[must_use]
    pub fn pinned_request_pending_for_test(&self) -> bool {
        self.pinned_request_pending.is_some()
    }

    /// Take the launched `GuiCore`'s capability gate into this `App`, as the
    /// GUI Client builds it from its launch location.
    #[cfg(any(test, feature = "test"))]
    pub fn set_gui_client_capabilities_for_test(&mut self) {
        self.set_client_capabilities(super::super::gui_client_capabilities());
    }

    /// Hold `hidden` as the persisted visual-slot hide: the setting the TUI
    /// writes and reads back, which the GUI Client never paints a control for.
    #[cfg(any(test, feature = "test"))]
    pub fn seed_visual_slot_hidden_for_test(&mut self, hidden: bool) {
        self.visual_slot_hidden = hidden;
    }

    /// Whether the persisted visual-slot hide is set.
    #[cfg(any(test, feature = "test"))]
    #[must_use]
    pub fn visual_slot_hidden_for_test(&self) -> bool {
        self.visual_slot_hidden
    }

    /// Whether an event the answer pump deferred is still waiting: the pump
    /// replays it before it reads the live receiver.
    #[cfg(any(test, feature = "test"))]
    #[must_use]
    pub fn deferred_player_events_len_for_test(&self) -> usize {
        self.deferred_player_events.len()
    }

    /// The overlay request the core raised, if one is still pending: the
    /// confirm, menu, or dialog a Client has not mounted yet.
    #[cfg(any(test, feature = "test"))]
    #[must_use]
    pub fn pending_overlay_for_test(&self) -> Option<&mbv_ui_model::overlay::OverlayRequest> {
        self.pending_overlay.as_ref()
    }

    /// Run the pinned-view toggle through the production dispatch, as the
    /// keyboard's pinned key does.
    #[cfg(any(test, feature = "test"))]
    pub fn pinned_view_toggle_for_test(&mut self) {
        self.pinned_view_toggle();
    }

    /// Apply `width` with `applied`'s answer through the production
    /// pinned-view application, as the pin worker's outcome does.
    #[cfg(any(test, feature = "test"))]
    pub fn apply_pinned_view_for_test(&mut self, width: PinnedWidth, applied: Result<(), String>) {
        self.apply_pinned_view(width, applied);
    }

    /// Run the play-collapse step through the production dispatch, as the
    /// play tail does after an accepted play.
    #[cfg(any(test, feature = "test"))]
    pub fn collapse_pinned_view_after_play_for_test(&mut self) {
        self.collapse_pinned_view_after_play();
    }

    /// Drain one pinned-worker outcome through the production pump, as the
    /// tick's pinned drain does.
    #[cfg(any(test, feature = "test"))]
    #[must_use]
    pub fn drain_pinned_requests_for_test(&mut self) -> bool {
        self.drain_pinned_requests()
    }

    /// Record `width` as the applied pinned width through the production
    /// path, as the worker's outcome does.
    #[cfg(any(test, feature = "test"))]
    pub fn note_pinned_width_applied_for_test(&mut self, width: PinnedWidth) {
        self.note_pinned_width_applied(width);
    }

    /// Focus the queue's initial item through the production path, as a
    /// focus move into the Queue does.
    #[cfg(any(test, feature = "test"))]
    pub fn focus_queue_initial_item_for_test(&mut self) {
        self.focus_queue_initial_item();
    }

    /// The exit message a Client prints after its terminal is restored, if
    /// one is still pending: the failure a teardown recorded.
    #[cfg(any(test, feature = "test"))]
    #[must_use]
    pub fn pending_exit_message_for_test(&self) -> Option<&str> {
        self.pending_exit_message.as_deref()
    }

    /// Drop the overlay request the core raised without answering it, as the
    /// shell's own modal dismissal before a direct confirm answer does.
    #[cfg(any(test, feature = "test"))]
    pub fn clear_pending_overlay_for_test(&mut self) {
        self.pending_overlay = None;
    }

    /// Take the overlay request the core raised and has not handed to its
    /// Client yet (`add-gui-settings-screen` design D2), as a Client's own
    /// mount pass does.
    #[cfg(any(test, feature = "test"))]
    #[must_use]
    pub fn take_pending_overlay_request_for_test(
        &mut self,
    ) -> Option<mbv_ui_model::overlay::OverlayRequest> {
        self.take_pending_overlay()
    }

    /// Whether the Feeds tab's refresh is in flight: the F5 on the Feeds
    /// destination.
    #[cfg(any(test, feature = "test"))]
    #[must_use]
    pub fn feeds_refresh_in_flight_for_test(&self) -> bool {
        self.feed_tab.loading
    }

    /// How many subscription fetches the Feeds refresh still waits on.
    #[cfg(any(test, feature = "test"))]
    #[must_use]
    pub fn feeds_pending_results_for_test(&self) -> usize {
        self.feed_tab.pending_results
    }

    /// Whether a queue transfer is staged behind its confirm prompt: the link
    /// the cancel answer drops.
    #[cfg(any(test, feature = "test"))]
    #[must_use]
    pub fn queue_transfer_is_staged_for_test(&self) -> bool {
        self.staged_queue_transfer.is_some()
    }

    /// Hold a staged Hand off to `sess` over `endpoint` with no held link, as
    /// a connected Hand off confirm re-stages the transfer after the peer
    /// link died: the shape the copy's own link gate reads.
    #[cfg(any(test, feature = "test"))]
    pub fn seed_staged_hand_off_transfer_for_test(
        &mut self,
        sess: mbv_emby::SessionInfo,
        endpoint: mbv_remote_player::DaemonEndpoint,
    ) {
        self.staged_queue_transfer = Some(crate::app::state::queue_transfer::StagedQueueTransfer {
            direction: crate::app::state::queue_transfer::QueueTransferDirection::HandOff,
            target: mbv_ui_model::panel_targets::SessionTargetKey::Emby(sess.device_name.clone()),
            sess,
            endpoint,
            link: None,
        });
    }

    /// Hold the staged Hand off the settlement test settles: the local queue's
    /// copy to the peer `name` already started, waiting on the start slot
    /// `start_slot` to be observed playing.
    #[cfg(any(test, feature = "test"))]
    pub fn stage_hand_off_settlement_for_test(&mut self, name: &str, start_slot: u64) {
        self.pending_queue_transfer =
            Some(crate::app::state::queue_transfer::PendingQueueTransfer {
                direction: crate::app::state::queue_transfer::QueueTransferDirection::HandOff,
                target: mbv_ui_model::panel_targets::SessionTargetKey::Emby(name.to_string()),
                start_slot: mbv_queue::QueueSlotId::from_raw(start_slot),
                source_link: None,
            });
    }

    /// Seed the joined-music fixture the Work's group level reads: every album
    /// of the open album level, and the group level above it, before the shell
    /// starts a grouping pass.
    #[cfg(any(test, feature = "test"))]
    pub fn seed_music_albums_for_test(&mut self, albums: Vec<mbv_emby_model::EmbyItem>) {
        let Some(tab) = self.libs.first_mut() else {
            return;
        };
        let Some(album_level) = tab.nav_stack.last_mut() else {
            return;
        };
        album_level.items = albums;
    }

    /// Note that `level_id`'s artist fill already failed, as a browse-triggered
    /// upgrade that got no answer leaves it.
    #[cfg(any(test, feature = "test"))]
    pub fn mark_album_artist_level_failed_for_test(&mut self, level_id: &str) {
        self.album_artist_levels.insert(
            level_id.to_string(),
            crate::app::state::app_struct::LevelFillState::Failed,
        );
    }

    /// Note that `level_id`'s artist fill already landed with
    /// `orphan_risk` left as the warm-up reported it.
    #[cfg(any(test, feature = "test"))]
    pub fn mark_album_artist_level_filled_for_test(&mut self, level_id: &str, orphan_risk: bool) {
        self.album_artist_levels.insert(
            level_id.to_string(),
            crate::app::state::app_struct::LevelFillState::Filled { orphan_risk },
        );
    }

    /// Seed the album-artist caches and the queued warm-ups a Service removal
    /// clears.
    #[cfg(any(test, feature = "test"))]
    pub fn seed_album_artist_state_for_test(
        &mut self,
        cached: &[(&str, &str)],
        filled_level: &str,
        queued_warmup: &str,
        in_flight_warmup: &str,
    ) {
        for (album_id, artist) in cached {
            self.album_artist_cache
                .insert((*album_id).to_string(), (*artist).to_string());
        }
        self.mark_album_artist_level_filled_for_test(filled_level, false);
        self.pending_level_artist_warmups
            .push_back(queued_warmup.to_string());
        self.level_artist_warmups_in_flight
            .insert(in_flight_warmup.to_string());
    }

    /// The artist cache, level fills, queued warm-ups, and in-flight warm-ups a
    /// Service removal leaves, read as one tuple: whether each is empty.
    #[cfg(any(test, feature = "test"))]
    #[must_use]
    pub fn album_artist_state_is_cleared_for_test(&self) -> (bool, bool, bool, bool) {
        (
            self.album_artist_cache.is_empty(),
            self.album_artist_levels.is_empty(),
            self.pending_level_artist_warmups.is_empty(),
            self.level_artist_warmups_in_flight.is_empty(),
        )
    }

    /// The album-artist cache's answer for `album_id`, as a landed fill left
    /// it.
    #[cfg(any(test, feature = "test"))]
    #[must_use]
    pub fn album_artist_cache_for_test(&self, album_id: &str) -> Option<&str> {
        self.album_artist_cache.get(album_id).map(String::as_str)
    }

    /// Whether the album-artist cache holds nothing.
    #[cfg(any(test, feature = "test"))]
    #[must_use]
    pub fn album_artist_cache_is_empty_for_test(&self) -> bool {
        self.album_artist_cache.is_empty()
    }

    /// The terminal state of `level_id`'s artist fill, as a browse-triggered
    /// upgrade and its answer leave it: whether any fill is recorded, and the
    /// orphan risk a landed one carries.
    #[cfg(any(test, feature = "test"))]
    #[must_use]
    pub fn album_artist_level_state_for_test(&self, level_id: &str) -> Option<(bool, bool)> {
        self.album_artist_levels
            .get(level_id)
            .map(|fill| match fill {
                crate::app::state::app_struct::LevelFillState::Loading { orphan_risk } => {
                    (true, *orphan_risk)
                }
                crate::app::state::app_struct::LevelFillState::Filled { orphan_risk } => {
                    (false, *orphan_risk)
                }
                crate::app::state::app_struct::LevelFillState::Failed => (false, false),
            })
    }

    /// Whether `level_id`'s artist fill landed at all: the fill a later
    /// candidate never repeats.
    #[cfg(any(test, feature = "test"))]
    #[must_use]
    pub fn album_artist_level_filled_for_test(&self, level_id: &str) -> bool {
        matches!(
            self.album_artist_levels.get(level_id),
            Some(crate::app::state::app_struct::LevelFillState::Filled { .. })
        )
    }

    /// Whether `level_id`'s artist fill failed.
    #[cfg(any(test, feature = "test"))]
    #[must_use]
    pub fn album_artist_level_failed_for_test(&self, level_id: &str) -> bool {
        matches!(
            self.album_artist_levels.get(level_id),
            Some(crate::app::state::app_struct::LevelFillState::Failed)
        )
    }

    /// Start or supersede the Emby library at `index`'s music grouping pass
    /// through the production path, as a browse into the grouped album level
    /// does.
    #[cfg(any(test, feature = "test"))]
    pub fn start_or_supersede_music_grouping_for_test(&mut self, index: usize) {
        self.start_or_supersede_music_grouping(index);
    }

    /// Advance the music grouping candidate with an artist answer through the
    /// production path, as a landed artist lookup does.
    #[cfg(any(test, feature = "test"))]
    pub fn advance_music_grouping_candidates_for_test(&mut self, album_id: &str, artist: &str) {
        self.advance_music_grouping_candidates(album_id, artist);
    }

    /// Force-settle the expired music grouping candidates through the
    /// production path, as the shell's tick does.
    #[cfg(any(test, feature = "test"))]
    pub fn expire_music_grouping_candidates_for_test(&mut self) {
        self.expire_music_grouping_candidates();
    }

    /// The music grouping state of the Emby library at `index`'s open browse
    /// level: the catalogue a start or an advance of the grouping pass
    /// leaves.
    #[cfg(any(test, feature = "test"))]
    #[must_use]
    pub fn open_music_grouping_state_for_test(
        &self,
        index: usize,
    ) -> Option<&mbv_ui_model::music_grouping::MusicGroupingState> {
        self.libs
            .get(index)?
            .nav_stack
            .last()?
            .music_grouping
            .as_ref()
    }

    /// Hold a music grouping candidate whose lookup window has already
    /// elapsed, as a silent artist lookup that started `by` ago sits: the
    /// state the expiry pass force-settles.
    ///
    /// # Panics
    ///
    /// Panics when the platform clock cannot reach back by `by`, which a
    /// test's lookup window never causes.
    #[cfg(any(test, feature = "test"))]
    pub fn backdate_music_grouping_candidate_for_test(&mut self, index: usize, by: Duration) {
        if let Some(candidate) = self
            .libs
            .get_mut(index)
            .and_then(|tab| tab.nav_stack.last_mut())
            .and_then(|level| level.music_grouping.as_mut())
            .and_then(|state| state.candidate.as_mut())
        {
            candidate.created_at = Instant::now()
                .checked_sub(by)
                .expect("the clock reaches back by the test's lookup window");
        }
    }

    /// Note that `level_id`'s artist fill is in flight with `orphan_risk` as
    /// the warm-up reported it, as a browse-triggered fill that got no answer
    /// yet sits.
    #[cfg(any(test, feature = "test"))]
    pub fn mark_album_artist_level_loading_for_test(&mut self, level_id: &str, orphan_risk: bool) {
        self.album_artist_levels.insert(
            level_id.to_string(),
            crate::app::state::app_struct::LevelFillState::Loading { orphan_risk },
        );
    }

    /// The terminal state of `level_id`'s artist fill itself, as a browse
    /// upgrade and its answer leave it: the exact fill the next candidate
    /// reads.
    #[cfg(any(test, feature = "test"))]
    #[must_use]
    pub fn album_artist_level_fill_for_test(
        &self,
        level_id: &str,
    ) -> Option<crate::app::state::app_struct::LevelFillState> {
        self.album_artist_levels.get(level_id).copied()
    }

    /// The music grouping state of `level`'s browse level in the Emby library
    /// at `index`: the catalogue a start or an advance of the grouping pass
    /// leaves.
    #[cfg(any(test, feature = "test"))]
    #[must_use]
    pub fn music_grouping_state_for_test(
        &self,
        index: usize,
        level: usize,
    ) -> Option<&mbv_ui_model::music_grouping::MusicGroupingState> {
        self.libs
            .get(index)?
            .nav_stack
            .get(level)?
            .music_grouping
            .as_ref()
    }
    /// Rebuild the Emby library tabs from `all_views` through the production
    /// path, as a catalog arrival does.
    #[cfg(any(test, feature = "test"))]
    pub fn rebuild_library_tabs_from_views_for_test(
        &mut self,
        all_views: &[mbv_emby_model::EmbyItem],
    ) {
        self.rebuild_library_tabs_from_views(all_views);
    }

    /// Resolve the item the Emby library at `index` selects at `cursor`
    /// through the production resolution, as a context menu or a mouse press
    /// does.
    #[cfg(any(test, feature = "test"))]
    #[must_use]
    pub fn current_lib_item_for_test(
        &self,
        index: usize,
        cursor: usize,
    ) -> Option<mbv_emby_model::EmbyItem> {
        self.current_lib_item(index, cursor)
    }

    /// Whether the Emby library at `index`'s first browse level holds no
    /// letter pill, as a Latest-restored open leaves it.
    #[cfg(any(test, feature = "test"))]
    #[must_use]
    pub fn root_letter_filter_is_unset_for_test(&self, index: usize) -> bool {
        self.libs
            .get(index)
            .and_then(|tab| tab.nav_stack.first())
            .is_none_or(|level| level.letter_filter.is_none())
    }

    /// Hold `entries` as the Feeds tab's fetched entries, as a feed fetch
    /// leaves them: the rows the destination's Latest marker is read
    /// against.
    #[cfg(any(test, feature = "test"))]
    pub fn seed_feed_entries_for_test(&mut self, entries: Vec<FeedEntry>) {
        self.feed_tab.all_entries = entries;
    }
}
