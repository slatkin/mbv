use crate::app::dispatch::notify::ToastSeverity;
use crate::app::dispatch::queue::QueueOpEdit;
#[cfg(test)]
use crate::app::tests::QueueViewTestExt;
use crate::app::{
    App, LocalPlaybackTarget, PanelFocus, PendingQueueAction, PlaybackTarget, RemotePlaybackTarget,
};
pub(in crate::app) use mbv_ctrl::player::CONNECTION_LOST_MESSAGE;
use mbv_ctrl::player::PlayerCommand;
use mbv_emby_model::EmbyItem;
use mbv_ids::ItemId;
use mbv_queue::QueueItem;
use mbv_ui_model::ui_util::natural_sort_key;

/// Classification for an explicit Emby play against the attached owner.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::app) enum PlaybackEligibility {
    Ineligible,
    WhollyUnplayable { unplayable_count: usize },
    Mixed { unplayable_count: usize },
    WhollyPlayable,
}

fn unavailable_suffix(count: usize) -> String {
    format!(
        " ({} item{} unavailable to the audio-only owner)",
        count,
        if count == 1 { "" } else { "s" }
    )
}

/// Format the existing playback request toast with the mixed-selection count.
fn playback_request_message(label: &str, mixed_unplayable: Option<usize>) -> String {
    match mixed_unplayable {
        Some(count) => format!("Requesting playback: {label}{}", unavailable_suffix(count)),
        None => format!("Requesting playback: {label}"),
    }
}

fn classify_playback_eligibility(
    attached: bool,
    library_route: bool,
    owner_is_audio_only: bool,
    items: &[EmbyItem],
) -> PlaybackEligibility {
    if !attached || library_route || !owner_is_audio_only {
        return PlaybackEligibility::Ineligible;
    }
    let unplayable_count = items
        .iter()
        .filter(|item| !item.media_type.eq_ignore_ascii_case("Audio"))
        .count();
    if unplayable_count == 0 {
        PlaybackEligibility::WhollyPlayable
    } else if unplayable_count == items.len() {
        PlaybackEligibility::WhollyUnplayable { unplayable_count }
    } else {
        PlaybackEligibility::Mixed { unplayable_count }
    }
}

impl App {
    /// Return the audio-only fall-through decision for an explicit Emby play.
    /// Empty/unknown selections and Library routes remain on today's path.
    pub(in crate::app) fn playback_eligibility(&self, items: &[EmbyItem]) -> PlaybackEligibility {
        let attached = true;
        let owner_is_audio_only = if self.connected_session_id.is_some() {
            self.session_owner_is_audio_only()
        } else {
            self.player.owner_is_audio_only()
        };
        classify_playback_eligibility(
            attached,
            self.active_route.is_some(),
            owner_is_audio_only,
            items,
        )
    }

    fn defer_local_play(
        &mut self,
        items: Vec<EmbyItem>,
        start_idx: usize,
        source: mbv_queue::QueueSource,
    ) {
        let label = items
            .get(start_idx)
            .or_else(|| items.first())
            .map(EmbyItem::playback_label)
            .unwrap_or_default();
        self.queue_deferrals
            .hold_local_play(PendingQueueAction::PlayItems {
                items,
                start_idx,
                source,
            });
        self.ask_confirm(mbv_ui_model::confirm::ConfirmModal::two_button(
            format!("Play \"{label}\" on this machine instead?"),
            "Play here",
            "Cancel",
            mbv_ui_model::confirm::ConfirmAction::PlayLocallyInstead,
        ));
    }

    /// Fetch the episodes of `item`'s series starting at `item`. `None` when
    /// Emby is unavailable; a short list means the caller's path decides what
    /// that means (the guard falls back to the single item, the play path
    /// reports it and stops).
    fn series_episodes_from(&self, item: &EmbyItem) -> Option<Vec<EmbyItem>> {
        let client = self.emby_client()?;
        let episodes = client.lock().unwrap().get_episodes_from(
            &ItemId::new(item.series_id.as_str()),
            &ItemId::new(item.id.as_str()),
        );
        Some(episodes)
    }
}

impl App {
    /// Cast takes priority over an attached Emby session: the two are
    /// mutually exclusive attachment slots (see `remote_slot_state.rs`), but
    /// this ordering keeps the seam correct even if that invariant is ever
    /// relaxed.
    pub(in crate::app) fn playback_target(&self) -> PlaybackTarget {
        if self.cast_attachment.is_some() {
            return PlaybackTarget::Cast(crate::app::CastPlaybackTarget);
        }
        match self.connected_session_id.clone() {
            Some(session_id) => PlaybackTarget::Remote(RemotePlaybackTarget { session_id }),
            None => PlaybackTarget::Local(LocalPlaybackTarget),
        }
    }

    pub(in crate::app) fn playback_display_target(&self) -> PlaybackTarget {
        if self.cast_attachment.is_some() || self.connected_session_state.is_some() {
            self.playback_target()
        } else {
            PlaybackTarget::Local(LocalPlaybackTarget)
        }
    }

    pub(in crate::app) fn playback_indicator_target(&self) -> PlaybackTarget {
        let local_active = self.player.status_snapshot().active;
        if local_active {
            PlaybackTarget::Local(LocalPlaybackTarget)
        } else {
            self.playback_display_target()
        }
    }
}

impl App {
    pub(in crate::app) fn remote_audio_indexes(&self) -> Vec<i64> {
        self.connected_session_state
            .as_ref()
            .map(|state| {
                state
                    .media_info
                    .audio_streams
                    .iter()
                    .map(|stream| stream.index)
                    .collect()
            })
            .unwrap_or_default()
    }

    pub(in crate::app) fn remote_subtitle_indexes(&self) -> Vec<i64> {
        self.connected_session_state
            .as_ref()
            .map(|state| {
                state
                    .media_info
                    .subtitle_streams
                    .iter()
                    .map(|stream| stream.index)
                    .collect()
            })
            .unwrap_or_default()
    }

    pub(in crate::app) fn lib_page_size(&self) -> usize {
        // The library list is rendered into the right panel; use the panel
        // height directly (rows are single-line; subtract 1 for the
        // count/search header line).
        (self.layout.left_area.height as usize)
            .saturating_sub(1)
            .max(1)
    }

    /// Resolve the item the library panel currently selects. `cursor` is the
    /// resolved index the caller owns (component-resolved for the generic
    /// browser, or the App nav-level cursor on the legacy context-menu/mouse
    /// paths) — never re-read from `BrowseLevel` (task 4.3, R1).
    pub(in crate::app) fn current_lib_item(
        &self,
        lib_idx: usize,
        cursor: usize,
    ) -> Option<EmbyItem> {
        let lib = self.libs.get(lib_idx)?;
        if lib.nav_stack.is_empty() {
            Some(lib.library.clone())
        } else {
            if self.is_feed_home_video_group_view(lib_idx) {
                return self.selected_feed_home_video_item(lib_idx);
            }
            let lvl = lib.nav_stack.last()?;
            lvl.items.get(cursor).cloned()
        }
    }

    pub(in crate::app) fn play_items_routed(
        &mut self,
        items: Vec<EmbyItem>,
        start_idx: usize,
        queue_source: mbv_queue::QueueSource,
    ) {
        if self.player.is_remote_disconnected() {
            self.flash(CONNECTION_LOST_MESSAGE.into(), ToastSeverity::Warning);
            return;
        }
        let mixed_unplayable = match self.playback_eligibility(&items) {
            PlaybackEligibility::WhollyUnplayable { .. } => {
                self.defer_local_play(items, start_idx, queue_source.clone());
                return;
            }
            PlaybackEligibility::Mixed { unplayable_count } => Some(unplayable_count),
            PlaybackEligibility::Ineligible | PlaybackEligibility::WhollyPlayable => None,
        };
        if let Some(item) = items.get(start_idx).or_else(|| items.first()) {
            tracing::info!(name: "library_route.queue_replace.requested", target: "library_route", item = %item.id, item_name = %item.name, "queue replacement requested");
            if self.in_non_library_thin_client_mode() {
                tracing::info!(name: "library_route.queue_replace.bypassed", target: "library_route", item = %item.id, item_name = %item.name, reason = "non_library_thin_client_owns_playback", "queue replacement route bypassed");
            } else {
                let item = item.clone();
                self.apply_route_for_playback(&item);
            }
        }
        let direct_remote = self.has_direct_remote_queue();
        if !direct_remote {
            self.on_queue_replace_silent();
        }
        self.set_queue_scope(self.playing_queue_scope());
        // Keep library focus when playing from the library panel.
        if !matches!(self.effective_panel_focus(), PanelFocus::Library) {
            self.set_panel_focus(PanelFocus::Queue);
        }
        // Attached Emby session: the queue is installed on the Player owner
        // through the idle-load path (ruling on row 5.3: the owner's Replace
        // op always starts playback, so it must not be used while another
        // target owns playback), then playback starts on the session.
        if let Some(ref conn_id) = self.connected_session_id.clone() {
            if !self.load_idle_queue_on_owner(
                items
                    .iter()
                    .map(|item| QueueItem::Emby(Box::new(item.clone())))
                    .collect(),
                start_idx,
                queue_source.clone(),
            ) {
                return;
            }
            self.clear_playback_overlays();
            let id = conn_id.clone();
            let label = items
                .get(start_idx)
                .map(mbv_emby_model::EmbyItem::playback_label)
                .unwrap_or_default();
            self.flash(
                playback_request_message(&label, mixed_unplayable),
                ToastSeverity::Neutral,
            );
            self.submit_attached_sequence(&id, &items, start_idx);
            return;
        }
        if direct_remote && let Some(item) = items.get(start_idx) {
            self.flash(
                playback_request_message(&item.playback_label(), mixed_unplayable),
                ToastSeverity::Neutral,
            );
        }
        // Row 5.3 (design D6): the replacement is an answered owner op — the
        // owner replaces its canonical queue, begins playback at `start_idx`,
        // and the Client adopts the resulting snapshot; the Client holds no
        // editable queue of its own.
        let sent = self.replace_emby_queue_on_owner(
            self.playing_queue_scope(),
            items,
            start_idx,
            queue_source,
        );
        if sent == QueueOpEdit::NotApplied {
            return;
        }
        let _ = self
            .player
            .send_command(PlayerCommand::SetMute(self.mute_on));
        if let Some(count) = mixed_unplayable
            && !direct_remote
        {
            self.flash(
                format!("Playback started{}", unavailable_suffix(count)),
                ToastSeverity::Neutral,
            );
        }
    }

    pub(in crate::app) fn play_item(&mut self, item: EmbyItem) {
        if self.player.is_remote_disconnected() {
            self.flash(CONNECTION_LOST_MESSAGE.into(), ToastSeverity::Warning);
            return;
        }
        tracing::info!(name: "library_route.play.requested", target: "library_route", item = %item.id, item_name = %item.name, "play requested");
        if matches!(
            self.playback_eligibility(std::slice::from_ref(&item)),
            PlaybackEligibility::WhollyUnplayable { .. }
        ) {
            self.defer_unplayable_play_item(item);
            return;
        }
        if self.in_non_library_thin_client_mode() {
            tracing::info!(name: "library_route.play.bypassed", target: "library_route", item = %item.id, item_name = %item.name, reason = "non_library_thin_client_owns_playback", "play route bypassed");
        } else {
            self.apply_route_for_playback(&item);
        }
        let direct_remote = self.has_direct_remote_queue();
        if !direct_remote {
            self.on_queue_replace_silent();
        }
        // Keep library focus when playing from the library panel.
        if !matches!(self.effective_panel_focus(), PanelFocus::Library) {
            self.set_panel_focus(PanelFocus::Queue);
        }
        let label = item.playback_label();
        if self.play_on_connected_session(&item, &label) {
            return;
        }
        if self.play_series_continuation(&item, direct_remote) {
            return;
        }
        self.play_single_item(item, direct_remote, &label);
    }

    /// Plays the rest of a series when "always play next" is on and the item
    /// belongs to a series with more than one episode. Returns `true` when the
    /// attempt was terminal (dispatched, or already flashed), `false` to let
    /// the caller fall through to the single-item path.
    fn play_series_continuation(&mut self, item: &EmbyItem, direct_remote: bool) -> bool {
        if item.series_id.is_empty() || !self.player.always_play_next {
            return false;
        }
        let Some(episodes) = self.series_episodes_from(item) else {
            self.flash("Emby is unavailable".into(), ToastSeverity::Warning);
            return true;
        };
        if episodes.len() <= 1 {
            return false;
        }
        if !direct_remote {
            self.on_queue_replace_silent();
        }
        // Row 5.3 (design D6): the replacement is an answered owner
        // op; the Client holds no editable queue of its own.
        let sent = self.replace_emby_queue_on_owner(
            self.playing_queue_scope(),
            episodes,
            0,
            mbv_queue::QueueSource::Series,
        );
        if sent == QueueOpEdit::NotApplied {
            return true;
        }
        let _ = self
            .player
            .send_command(PlayerCommand::SetMute(self.mute_on));
        true
    }

    /// Submits a single-item queue replacement, then mutes the local player.
    fn play_single_item(&mut self, item: EmbyItem, direct_remote: bool, label: &str) {
        if direct_remote {
            self.flash(
                format!("Requesting playback: {label}"),
                ToastSeverity::Neutral,
            );
        }
        // Row 5.3 (design D6): the replacement is an answered owner op; the
        // Client holds no editable queue of its own.
        let sent = self.replace_emby_queue_on_owner(
            self.playing_queue_scope(),
            vec![item],
            0,
            mbv_queue::QueueSource::Unknown,
        );
        if sent == QueueOpEdit::NotApplied {
            return;
        }
        let _ = self
            .player
            .send_command(PlayerCommand::SetMute(self.mute_on));
    }

    /// Routes a play request through the connected Emby session when one is
    /// attached. Returns `false` when playback is local and the caller should
    /// fall through to the queue-replacement path.
    fn play_on_connected_session(&mut self, item: &EmbyItem, label: &str) -> bool {
        let Some(conn_id) = self.connected_session_id.clone() else {
            return false;
        };
        self.advance_queue_epoch();
        self.clear_playback_overlays();
        let item_id = item.id.clone();
        let start_ticks = item.playback_position_ticks;
        self.flash(
            format!("Requesting playback: {label}"),
            ToastSeverity::Neutral,
        );
        self.do_session_command(move |c| c.session_play(&conn_id, &item_id, start_ticks));
        true
    }

    /// Defers an unplayable item for confirmation. The deferred play must carry
    /// what the ordinary path would submit: session control plays the single
    /// item, but the direct-remote/local path expands the series continuation
    /// first, so deferring one episode would drop the rest of the series on
    /// confirmation.
    fn defer_unplayable_play_item(&mut self, item: EmbyItem) {
        if self.connected_session_id.is_none()
            && !item.series_id.is_empty()
            && self.player.always_play_next
            && let Some(episodes) = self
                .series_episodes_from(&item)
                .filter(|episodes| episodes.len() > 1)
        {
            self.defer_local_play(episodes, 0, mbv_queue::QueueSource::Series);
            return;
        }
        self.defer_local_play(vec![item], 0, self.playback_queue().source().clone());
    }

    pub(in crate::app) fn do_enqueue_folder(&mut self, item: &mbv_emby_model::EmbyItem) {
        tracing::info!(name: "library_route.enqueue.requested", target: "library_route", item = %item.id, item_name = %item.name, "enqueue requested");
        let resolved = self.resolve_route_for_enqueue_folder(item);
        if self.enqueue_route_conflict(resolved.as_ref()) {
            return;
        }
        let Some(client) = self.emby_client() else {
            self.flash("Emby is unavailable".into(), ToastSeverity::Warning);
            return;
        };
        let client = client.lock().unwrap();
        match client.get_all_playable_recursive(&item.id) {
            Ok(mut items) => {
                items.retain(|i| !i.is_folder);
                items.sort_by_key(|a| natural_sort_key(a.sort_key()));
                let count = items.len();
                drop(client);
                if count == 0 {
                    self.flash("Nothing to enqueue".into(), ToastSeverity::Error);
                    return;
                }
                // Row 5.3 (design D6): the folder's items reach the viewed
                // queue's owner as one answered Append op; the Client holds
                // no editable queue, so the entries appear only when the
                // owner's answer is adopted.
                let scope = self.viewed_queue_scope();
                self.append_on_owner(
                    scope,
                    items
                        .into_iter()
                        .map(|item| QueueItem::Emby(Box::new(item)))
                        .collect(),
                );
            }
            Err(e) => {
                drop(client);
                self.flash(format!("Couldn't enqueue items: {e}"), ToastSeverity::Error);
            }
        }
    }

    /// Shared tail for submitting a single `QueueItem` to the canonical queue:
    /// play replaces the playing-target queue with exactly this item and starts
    /// it — the same single-item semantics as the Emby browse play path
    /// (`play_item`), so every destination's Enter behaves alike — through the
    /// answered `Replace` op on capable owners and the same replace in its
    /// legacy wire form (no op id, no answer to wait for) on legacy owners —
    /// and enqueue appends without starting playback. While a cast target is
    /// attached, the selection is dispatched to the receiver instead of the
    /// local player (cast-session-control): the owner's `Replace` op always
    /// starts playback, so capable owners get the owner queue loaded without
    /// starting it through the idle-load route first, and the dispatch waits
    /// for the owner's accepted load answer; legacy owners are dispatched
    /// with no owner call at all. The Client holds no editable
    /// queue (row 5.3, design D6), so the view changes only through the
    /// owner's answer.
    /// Callers resolve their own provider-specific selection/admission ahead
    /// of the call. Returns whether the submit succeeded.
    pub(in crate::app) fn submit_queue_item(
        &mut self,
        item: QueueItem,
        start_playback: bool,
    ) -> bool {
        let scope = if start_playback {
            self.playing_queue_scope()
        } else {
            self.viewed_queue_scope()
        };
        if !start_playback {
            return self.append_on_owner(scope, vec![item]);
        }
        if !self.is_cast_attached() && self.player.is_remote_disconnected() {
            self.flash(CONNECTION_LOST_MESSAGE.into(), ToastSeverity::Warning);
            return false;
        }
        if !self.has_direct_remote_queue() {
            self.on_queue_replace_silent();
        }
        let answered = self
            .queue_link(scope)
            .0
            .remote()
            .supports_answered_queue_ops();
        // While a cast target is attached, playing a selection dispatches it
        // to the receiver instead of the local player (cast-session-control
        // "Attaching to a cast target does not engage the local player") and
        // must never begin local playback of it.
        if self.is_cast_attached() {
            if answered {
                // The receiver is handed the owner-accepted queue: the idle
                // load's correlated answer gates the dispatch, so a rejected
                // or unanswered load (another idle load pending, a stale
                // owner) flashes and dispatches nothing.
                let Some(request_id) = self.send_idle_queue_load(
                    vec![item.clone()],
                    0,
                    mbv_queue::QueueSource::Unknown,
                ) else {
                    return false;
                };
                if !self.await_queue_load(request_id) {
                    return false;
                }
            }
            self.dispatch_selection_to_cast(vec![item], 0);
        } else {
            // Row 5.3 (design D6): play replaces the owner queue with exactly
            // this item and starts it, as an answered op when the owner
            // negotiates `answered-queue-ops` (which also cold-starts a run
            // when none is alive); legacy owners get the same replace in its
            // legacy wire form.
            let sent =
                self.replace_queue_on_owner(scope, vec![item], 0, mbv_queue::QueueSource::Unknown);
            if sent == QueueOpEdit::NotApplied {
                return false;
            }
            if answered {
                let _ = self
                    .player
                    .send_command(PlayerCommand::SetMute(self.mute_on));
            }
        }
        self.set_queue_scope(scope);
        if !matches!(self.effective_panel_focus(), PanelFocus::Library) {
            self.set_panel_focus(PanelFocus::Queue);
        }
        true
    }

    pub(in crate::app) fn append_on_owner(
        &mut self,
        scope: crate::app::QueueScope,
        items: Vec<QueueItem>,
    ) -> bool {
        // Entries appear only when the owner's answered Append is adopted.
        let sent = self.queue_op(
            scope,
            mbv_remote_player::QueueOp::Append {
                items,
                before: None,
            },
        );
        if sent == QueueOpEdit::NotApplied {
            return false;
        }
        if self.local_queue_metadata_applies(scope) {
            self.queue_dirty = true;
        }
        self.advance_queue_epoch();
        true
    }
}

#[cfg(test)]
mod letter_tests;
#[cfg(test)]
mod queue_enrich_tests;
#[cfg(test)]
mod queue_state_control_tests;
#[cfg(test)]
mod queue_state_tests;
#[cfg(test)]
mod queue_tests;
#[cfg(test)]
mod route_tests;
#[cfg(test)]
mod tests;
