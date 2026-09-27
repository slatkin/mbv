use crate::app::dispatch::notify::ToastSeverity;
use crate::app::infra::ui_util::natural_sort_key;
use crate::app::{
    App, LocalPlaybackTarget, PanelFocus, PendingQueueAction, PlaybackTarget, RemotePlaybackTarget,
};
use mbv_core::api::EmbyItem;
use mbv_core::playback_queue::{QueueItem, QueueItemContentId};
use mbv_core::player::PlayerCommand;
pub(in crate::app) use mbv_core::player::CONNECTION_LOST_MESSAGE;
use mbv_ids::ItemId;
use std::sync::Arc;

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

fn daemon_endpoint_name(endpoint: &mbv_core::remote_player::DaemonEndpoint) -> String {
    match endpoint {
        mbv_core::remote_player::DaemonEndpoint::Local => "the local daemon".into(),
        mbv_core::remote_player::DaemonEndpoint::Unix(path) => {
            format!("the Unix socket {}", path.display())
        }
        mbv_core::remote_player::DaemonEndpoint::Tcp(address) => address.to_string(),
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
        let attached = self.connected_session_id.is_some() || self.player.is_remote();
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
        source: crate::config::QueueSource,
    ) {
        let label = items
            .get(start_idx)
            .or_else(|| items.first())
            .map(EmbyItem::playback_label)
            .unwrap_or_default();
        self.pending_local_play = Some(PendingQueueAction::PlayItems {
            items,
            start_idx,
            source,
            autostart: true,
        });
        let owner = self
            .connected_session_state
            .as_ref()
            .map(|session| session.device_name.clone())
            .or_else(|| self.remote.direct_remote_label.clone())
            .or_else(|| self.player_endpoint.as_ref().map(daemon_endpoint_name))
            .unwrap_or_else(|| "this owner".into());
        self.ask_confirm(crate::app::state::types::confirm::ConfirmModal {
            title: format!(" Play locally instead of {owner} "),
            message: format!("Play \"{label}\" on this machine instead?"),
            hint: "[y] Play here    [n] Cancel".into(),
            on_confirm: crate::app::state::types::confirm::ConfirmAction::PlayLocallyInstead,
        });
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

/// Where playback should resume within a restored queue. Prefers locating
/// `last_played_content_id` by identity (robust to the saved `cursor` index having
/// drifted, e.g. if the list was edited before the last save) and falls back
/// to the saved cursor only when there's no last-played id to anchor on.
pub(crate) fn queue_restore_cursor(
    items: &[QueueItem],
    saved_cursor: usize,
    last_played_content_id: Option<&QueueItemContentId>,
    legacy_last_played_item_id: Option<&str>,
    last_played_completed: bool,
) -> usize {
    let fallback = saved_cursor.min(items.len().saturating_sub(1));
    let identity = last_played_content_id.cloned().or_else(|| {
        let id = legacy_last_played_item_id?;
        let mut matches = items.iter().filter(|item| item.id() == id);
        let first = matches.next()?;
        if matches.next().is_some() {
            None
        } else {
            Some(first.content_id())
        }
    });
    let Some(identity) = identity else {
        return fallback;
    };
    // If the last-played item is no longer in the restored list (e.g. it was
    // removed from the queue before quitting), fall back to the saved cursor
    // rather than silently jumping to the front of the queue.
    let mut matches = items
        .iter()
        .enumerate()
        .filter(|(_, item)| item.content_id() == identity);
    let Some((idx, _)) = matches.next() else {
        return fallback;
    };
    if matches.next().is_some() {
        return fallback;
    }
    if last_played_completed {
        (idx + 1).min(items.len().saturating_sub(1))
    } else {
        idx
    }
}

impl App {
    /// Submit the already-replaced tab queue without re-minting slot ids.
    /// The tab's canonical pairs are the identity source for both local and
    /// direct-remote owners; fresh `play_queue` projections would diverge
    /// from monotonic tab ids after a replacement.
    pub(in crate::app) fn submit_tab_queue(
        &mut self,
        scope: crate::app::QueueScope,
        start_idx: usize,
        source: crate::config::QueueSource,
    ) -> bool {
        if self.player.is_remote_disconnected() {
            self.flash(CONNECTION_LOST_MESSAGE.into(), ToastSeverity::Warning);
            return false;
        }
        let slots = self.queue_for_scope(scope).all_queue_slots();
        if slots.is_empty() {
            return false;
        }
        let headless = slots.iter().all(|slot| slot.item.is_audio());
        let sent = self.player.submit_queue_slots(
            slots,
            start_idx,
            source,
            self.emby_snapshot().map(Arc::new),
            headless,
            self.ui_volume,
        );
        if !sent && self.player.is_remote_disconnected() {
            self.flash(CONNECTION_LOST_MESSAGE.into(), ToastSeverity::Warning);
        }
        if sent && matches!(scope, crate::app::QueueScope::Local) && !self.player.is_remote() {
            self.stamp_queue_generation(scope);
        }
        sent
    }

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
        let local_active = self.player.status.lock().unwrap().active;
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
        queue_source: crate::config::QueueSource,
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
            log::info!(target: "library_route", "user action=queue-replace item_id={:?} item_name={:?}", item.id, item.name);
            if self.in_non_library_thin_client_mode() {
                log::info!(target: "library_route", "route bypass action=queue-replace item_id={:?} item_name={:?} reason=non-library thin-client owns playback", item.id, item.name);
            } else {
                let item = item.clone();
                self.apply_route_for_playback(&item);
            }
        }
        let direct_remote = self.has_direct_remote_queue();
        if !direct_remote {
            self.on_queue_replace_silent();
        }
        self.set_queue_source_if_not_local_daemon(queue_source.clone());
        self.set_queue_scope(self.playing_queue_scope());
        // Keep library focus when playing from the library panel.
        if !matches!(self.effective_panel_focus(), PanelFocus::Library) {
            self.set_panel_focus(PanelFocus::Queue);
        }
        if let Some(ref conn_id) = self.connected_session_id.clone() {
            self.clear_playback_overlays();
            let id = conn_id.clone();
            let label = items
                .get(start_idx)
                .map(mbv_core::api::EmbyItem::playback_label)
                .unwrap_or_default();
            self.flash(
                playback_request_message(&label, mixed_unplayable),
                ToastSeverity::Neutral,
            );
            self.submit_attached_sequence(&id, &items, start_idx);
            return;
        }
        if direct_remote {
            if let Some(item) = items.get(start_idx) {
                self.flash(
                    playback_request_message(&item.playback_label(), mixed_unplayable),
                    ToastSeverity::Neutral,
                );
            }
        }
        self.submit_tab_queue(self.playing_queue_scope(), start_idx, queue_source);
        self.player
            .send_command(PlayerCommand::SetMute(self.mute_on));
        if let Some(count) = mixed_unplayable {
            if !direct_remote {
                self.flash(
                    format!("Playback started{}", unavailable_suffix(count)),
                    ToastSeverity::Neutral,
                );
            }
        }
    }

    pub(in crate::app) fn play_item(&mut self, item: EmbyItem) {
        if self.player.is_remote_disconnected() {
            self.flash(CONNECTION_LOST_MESSAGE.into(), ToastSeverity::Warning);
            return;
        }
        log::info!(target: "library_route", "user action=play item_id={:?} item_name={:?}", item.id, item.name);
        if matches!(
            self.playback_eligibility(std::slice::from_ref(&item)),
            PlaybackEligibility::WhollyUnplayable { .. }
        ) {
            // The deferred play must carry what the ordinary path would
            // submit: session control plays the single item, but the
            // direct-remote/local path expands the series continuation
            // first, so deferring one episode would drop the rest of the
            // series on confirmation.
            if self.connected_session_id.is_none()
                && !item.series_id.is_empty()
                && self.player.always_play_next
            {
                if let Some(episodes) = self
                    .series_episodes_from(&item)
                    .filter(|episodes| episodes.len() > 1)
                {
                    self.defer_local_play(episodes, 0, crate::config::QueueSource::Series);
                    return;
                }
            }
            self.defer_local_play(vec![item], 0, self.queue_source.clone());
            return;
        }
        if self.in_non_library_thin_client_mode() {
            log::info!(target: "library_route", "route bypass action=play item_id={:?} item_name={:?} reason=non-library thin-client owns playback", item.id, item.name);
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
        if let Some(ref conn_id) = self.connected_session_id.clone() {
            self.advance_queue_epoch();
            self.clear_playback_overlays();
            let id = conn_id.clone();
            let item_id = item.id.clone();
            let start_ticks = item.playback_position_ticks;
            self.flash(
                format!("Requesting playback: {label}"),
                ToastSeverity::Neutral,
            );
            self.do_session_command(move |c| c.session_play(&id, &item_id, start_ticks));
            return;
        }
        if !item.series_id.is_empty() && self.player.always_play_next {
            let Some(episodes) = self.series_episodes_from(&item) else {
                self.flash("Emby is unavailable".into(), ToastSeverity::Warning);
                return;
            };
            if episodes.len() > 1 {
                if !direct_remote {
                    self.on_queue_replace_silent();
                }
                self.replace_playback_queue(episodes.clone(), 0);
                let source = crate::config::QueueSource::Series;
                self.set_queue_source_if_not_local_daemon(source.clone());
                self.submit_tab_queue(self.playing_queue_scope(), 0, source);
                self.player
                    .send_command(PlayerCommand::SetMute(self.mute_on));
                if !self.has_direct_remote_queue() {
                    self.save_queue_state();
                }
                return;
            }
        }
        self.replace_playback_queue(vec![item.clone()], 0);
        if direct_remote {
            self.flash(
                format!("Requesting playback: {label}"),
                ToastSeverity::Neutral,
            );
        }
        self.submit_tab_queue(
            self.playing_queue_scope(),
            0,
            crate::config::QueueSource::Unknown,
        );
        self.player
            .send_command(PlayerCommand::SetMute(self.mute_on));
    }

    pub(in crate::app) fn do_enqueue_folder(&mut self, item: &mbv_core::api::EmbyItem) {
        log::info!(target: "library_route", "user action=enqueue item_id={:?} item_name={:?}", item.id, item.name);
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
                let scope = self.viewed_queue_scope();
                let previous_dirty = self.queue_dirty;
                let previous_queue = self.queue_for_scope(scope).clone();
                let appended_slots = self.queue_for_scope_mut(scope).append_items(items);
                if self.local_queue_metadata_applies(scope) {
                    self.queue_dirty = true;
                }
                if self.sync_playback_queue_items_after_append(scope, appended_slots) {
                    self.persist_local_queue_state_if_needed(scope);
                    self.advance_queue_epoch();
                } else {
                    self.queue_dirty = previous_dirty;
                    *self.queue_for_scope_mut(scope) = previous_queue;
                }
            }
            Err(e) => {
                drop(client);
                self.flash(format!("Couldn't enqueue items: {e}"), ToastSeverity::Error);
            }
        }
    }

    /// Shared tail for submitting a single `QueueItem` to the canonical queue
    /// (Task 8.1): play looks up an existing slot by `content_id()`, appends
    /// if absent, sets cursor/active slot, and submits the full queue to the
    /// player, rolling the queue back and flashing on rejection; enqueue
    /// appends without starting playback and syncs/persists like the library
    /// enqueue path. Callers resolve their own provider-specific
    /// selection/admission ahead of the call. Returns whether the submit
    /// succeeded.
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
            let previous_dirty = self.queue_dirty;
            let previous_queue = self.queue_for_scope(scope).clone();
            let appended_slot = self.queue_for_scope_mut(scope).append_item(item);
            if self.local_queue_metadata_applies(scope) {
                self.queue_dirty = true;
            }
            if self.sync_playback_queue_items_after_append(scope, vec![appended_slot]) {
                self.persist_local_queue_state_if_needed(scope);
                self.advance_queue_epoch();
                return true;
            }
            self.queue_dirty = previous_dirty;
            *self.queue_for_scope_mut(scope) = previous_queue;
            return false;
        }
        if !self.is_cast_attached() && self.player.is_remote_disconnected() {
            self.flash(CONNECTION_LOST_MESSAGE.into(), ToastSeverity::Warning);
            return false;
        }
        let previous_queue = self.queue_for_scope(scope).clone();
        let existing_index = self
            .queue_for_scope(scope)
            .slots()
            .iter()
            .position(|slot| slot.item.content_id() == item.content_id());
        let selected_index = existing_index.unwrap_or_else(|| {
            self.queue_for_scope_mut(scope).queue.append(item.clone());
            self.queue_for_scope(scope).total_queue_len() - 1
        });
        let selected_slot = self
            .queue_for_scope(scope)
            .slot_id_at(selected_index)
            .expect("selected queue slot disappeared");
        {
            let queue = self.queue_for_scope_mut(scope);
            queue.queue_cursor = selected_index;
            let _ = queue.queue.set_active_slot(selected_slot);
        }
        let all_slots = self.queue_for_scope(scope).all_queue_slots();
        // While a cast target is attached, playing a selection dispatches it
        // to the receiver instead of the local player (cast-session-control
        // "Attaching to a cast target does not engage the local player").
        // `submit_queue_slots`/local playback state below is never touched on this
        // path.
        if self.is_cast_attached() {
            let all_items = self.queue_for_scope(scope).all_queue_items();
            self.dispatch_selection_to_cast(all_items, selected_index);
            self.set_queue_scope(scope);
            if !matches!(self.effective_panel_focus(), PanelFocus::Library) {
                self.set_panel_focus(PanelFocus::Queue);
            }
            return true;
        }
        if self.player.is_remote_disconnected() {
            *self.queue_for_scope_mut(scope) = previous_queue;
            self.flash(CONNECTION_LOST_MESSAGE.into(), ToastSeverity::Warning);
            return false;
        }
        let audio_only = all_slots.iter().all(|slot| slot.item.is_audio());
        let submitted = self.player.submit_queue_slots(
            all_slots,
            selected_index,
            self.queue_source.clone(),
            None,
            audio_only,
            self.ui_volume,
        );
        if !submitted {
            *self.queue_for_scope_mut(scope) = previous_queue;
            self.flash(
                if self.player.is_remote_disconnected() {
                    CONNECTION_LOST_MESSAGE
                } else {
                    "Playback owner rejected this item"
                }
                .into(),
                if self.player.is_remote_disconnected() {
                    ToastSeverity::Warning
                } else {
                    ToastSeverity::Error
                },
            );
            return false;
        }
        self.set_queue_scope(scope);
        if !matches!(self.effective_panel_focus(), PanelFocus::Library) {
            self.set_panel_focus(PanelFocus::Queue);
        }
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
