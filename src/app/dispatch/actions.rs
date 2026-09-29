use crate::app::dispatch::notify::ToastSeverity;
use crate::app::dispatch::queue::QueueOpEdit;
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

fn daemon_endpoint_name(endpoint: &mbv_remote_player::DaemonEndpoint) -> String {
    match endpoint {
        mbv_remote_player::DaemonEndpoint::Local => "the local daemon".into(),
        mbv_remote_player::DaemonEndpoint::Unix(path) => {
            format!("the Unix socket {}", path.display())
        }
        mbv_remote_player::DaemonEndpoint::Tcp(address) => address.to_string(),
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
        let attached = self.connected_session_id.is_some() || self.player.as_remote().is_some();
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
                autostart: true,
            });
        let owner = self
            .connected_session_state
            .as_ref()
            .map(|session| session.device_name.clone())
            .or_else(|| self.remote.direct_remote_label.clone())
            .or_else(|| self.player_endpoint.as_ref().map(daemon_endpoint_name))
            .unwrap_or_else(|| "this owner".into());
        self.ask_confirm(mbv_ui_model::confirm::ConfirmModal {
            title: format!(" Play locally instead of {owner} "),
            message: format!("Play \"{label}\" on this machine instead?"),
            hint: "[y] Play here    [n] Cancel".into(),
            on_confirm: mbv_ui_model::confirm::ConfirmAction::PlayLocallyInstead,
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
            if !self.load_idle_queue_on_owner(items.clone(), start_idx, queue_source.clone()) {
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
        let sent = self.replace_queue_on_owner(
            self.playing_queue_scope(),
            items
                .iter()
                .map(|i| QueueItem::Emby(Box::new(i.clone())))
                .collect(),
            start_idx,
            queue_source,
        );
        if sent == QueueOpEdit::NotApplied {
            return;
        }
        self.player
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
            // The deferred play must carry what the ordinary path would
            // submit: session control plays the single item, but the
            // direct-remote/local path expands the series continuation
            // first, so deferring one episode would drop the rest of the
            // series on confirmation.
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
                // Row 5.3 (design D6): the replacement is an answered owner
                // op; the Client holds no editable queue of its own.
                let sent = self.replace_queue_on_owner(
                    self.playing_queue_scope(),
                    episodes
                        .iter()
                        .map(|i| QueueItem::Emby(Box::new(i.clone())))
                        .collect(),
                    0,
                    mbv_queue::QueueSource::Series,
                );
                if sent == QueueOpEdit::NotApplied {
                    return;
                }
                self.player
                    .send_command(PlayerCommand::SetMute(self.mute_on));
                return;
            }
        }
        if direct_remote {
            self.flash(
                format!("Requesting playback: {label}"),
                ToastSeverity::Neutral,
            );
        }
        // Row 5.3 (design D6): the replacement is an answered owner op; the
        // Client holds no editable queue of its own.
        let sent = self.replace_queue_on_owner(
            self.playing_queue_scope(),
            vec![QueueItem::Emby(Box::new(item))],
            0,
            mbv_queue::QueueSource::Unknown,
        );
        if sent == QueueOpEdit::NotApplied {
            return;
        }
        self.player
            .send_command(PlayerCommand::SetMute(self.mute_on));
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
                let sent = self.queue_op(
                    scope,
                    mbv_remote_player::QueueOp::Append {
                        items: items
                            .into_iter()
                            .map(|i| mbv_queue::QueueItem::Emby(Box::new(i)))
                            .collect(),
                        before: None,
                    },
                );
                if sent != QueueOpEdit::NotApplied {
                    if self.local_queue_metadata_applies(scope) {
                        self.queue_dirty = true;
                    }
                    self.advance_queue_epoch();
                }
            }
            Err(e) => {
                drop(client);
                self.flash(format!("Couldn't enqueue items: {e}"), ToastSeverity::Error);
            }
        }
    }

    /// Shared tail for submitting a single `QueueItem` to the canonical queue:
    /// play resolves the played entry against the owner's last accepted state
    /// — an existing slot is started through the answered `PlaySlot` op, an
    /// absent one is first appended through the answered `Append` op — and a
    /// cast receiver is dispatched the owner-accepted queue; enqueue appends
    /// without starting playback. The Client holds no editable queue (row
    /// 5.3, design D6), so the view changes only through the owner's answer.
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
            // Row 5.3 (design D6): the enqueue reaches the scope's Player
            // owner as an answered Append op; the Client holds no editable
            // queue, so the entry appears only when the owner's answer is
            // adopted.
            let sent = self.queue_op(
                scope,
                mbv_remote_player::QueueOp::Append {
                    items: vec![item],
                    before: None,
                },
            );
            if sent != QueueOpEdit::NotApplied {
                if self.local_queue_metadata_applies(scope) {
                    self.queue_dirty = true;
                }
                self.advance_queue_epoch();
                return true;
            }
            return false;
        }
        if !self.is_cast_attached() && self.player.is_remote_disconnected() {
            self.flash(CONNECTION_LOST_MESSAGE.into(), ToastSeverity::Warning);
            return false;
        }
        // Legacy owners have no answered Append to resolve the played slot
        // from, so they keep the legacy whole-queue submission (the legacy
        // form of replace-and-play); capable owners get the answered ops.
        let answered = self
            .queue_link(scope)
            .0
            .as_remote()
            .is_some_and(|remote| remote.supports_answered_queue_ops());
        if !answered {
            return self.submit_queue_item_legacy_play(&item, scope);
        }
        // Resolve the played entry against the owner's last accepted state.
        let existing_index = self
            .queue_for_scope(scope)
            .slots()
            .iter()
            .position(|slot| slot.item.content_id() == item.content_id());
        let selected_slot = if let Some(index) = existing_index {
            self.queue_for_scope(scope).slot_id_at(index)
        } else {
            // The item is new to the owner's queue: append it as an
            // answered op, then start the slot the answer adopted at the
            // tail.
            let sent = self.queue_op(
                scope,
                mbv_remote_player::QueueOp::Append {
                    items: vec![item],
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
            let last = self
                .queue_for_scope(scope)
                .total_queue_len()
                .saturating_sub(1);
            self.queue_for_scope(scope).slot_id_at(last)
        };
        let Some(selected_slot) = selected_slot else {
            return false;
        };
        // While a cast target is attached, playing a selection dispatches it
        // to the receiver instead of the local player (cast-session-control
        // "Attaching to a cast target does not engage the local player").
        // The receiver is handed the owner-accepted queue, so the played
        // entry must already be an owner-accepted slot.
        if self.is_cast_attached() {
            let selected_index = self
                .queue_for_scope(scope)
                .slots()
                .iter()
                .position(|slot| slot.slot_id == selected_slot);
            let Some(selected_index) = selected_index else {
                return false;
            };
            let all_items = self.queue_for_scope(scope).all_queue_items();
            self.dispatch_selection_to_cast(all_items, selected_index);
            self.set_queue_scope(scope);
            if !matches!(self.effective_panel_focus(), PanelFocus::Library) {
                self.set_panel_focus(PanelFocus::Queue);
            }
            return true;
        }
        // Row 5.3 (design D6): starting the entry is an answered PlaySlot op
        // against the owner that holds the queue.
        let sent = self.queue_op(
            scope,
            mbv_remote_player::QueueOp::PlaySlot {
                slot_id: mbv_ctrl::slot_id_to_u64(selected_slot),
            },
        );
        if sent == QueueOpEdit::NotApplied {
            return false;
        }
        self.set_queue_scope(scope);
        if !matches!(self.effective_panel_focus(), PanelFocus::Library) {
            self.set_panel_focus(PanelFocus::Queue);
        }
        true
    }

    /// The legacy-owner play path for a single submitted item (row 5.3):
    /// append-if-absent against the displayed queue, start the whole queue at
    /// the played entry, and dispatch a cast attachment its selection — the
    /// legacy wire forms, with the same rollback and flash behaviour.
    fn submit_queue_item_legacy_play(
        &mut self,
        item: &QueueItem,
        scope: crate::app::QueueScope,
    ) -> bool {
        let mut items = self
            .queue_for_scope(scope)
            .slots()
            .iter()
            .map(|slot| slot.item.clone())
            .collect::<Vec<_>>();
        let selected_index = items
            .iter()
            .position(|queued| queued.content_id() == item.content_id())
            .unwrap_or_else(|| {
                items.push(item.clone());
                items.len() - 1
            });
        if self.is_cast_attached() {
            self.dispatch_selection_to_cast(items, selected_index);
        } else {
            let source = self.queue_for_scope(scope).source().clone();
            if self.replace_queue_on_owner(scope, items, selected_index, source)
                == QueueOpEdit::NotApplied
            {
                return false;
            }
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
