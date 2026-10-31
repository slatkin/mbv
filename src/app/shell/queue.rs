use super::Model;
use super::{PanelFocus, PlaybackState, QueueLink};
use crate::app::dispatch::notify::ToastSeverity;
use mbv_components::{QueueComponent, QueueCursorUpdate};
use mbv_queue::QueueSlotId;
use mbv_ui_model::playback_target::NowPlayingStatus;
use mbv_ui_msg::{ComponentId, QueueColumnResize, QueueIntent, QueueMove, QueueRequest};

/// The row projection inputs that can change queue rows. Chrome and pause
/// state are delivered independently; pause only affects the paint-time
/// progress bucket.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::app) struct QueueProjectionFingerprint {
    revision: u64,
    lineage: mbv_queue::QueueLineage,
    link: QueueLink,
    active: bool,
    active_target: Option<QueueSlotId>,
    /// The adopted pending transition awaiting playback-owner observation.
    /// It moves the now-playing row before the owner reports it, so it remains
    /// part of the fingerprint even while `active_target` is unchanged.
    pending_target: Option<QueueSlotId>,
    progress_bucket: u16,
}

fn progress_bucket(playback: PlaybackState) -> u16 {
    if playback.active && playback.position_ticks > 0 && playback.runtime_ticks > 0 {
        u16::try_from((playback.position_ticks * 100 / playback.runtime_ticks).clamp(0, 100))
            .expect("clamped progress bucket fits in u16")
    } else {
        u16::MAX
    }
}

/// The observed active slot, or the owner's pending slot when idle.
fn projected_active_target(
    queue: &super::QueueView,
    playback: PlaybackState,
    pending_slot: Option<QueueSlotId>,
) -> Option<QueueSlotId> {
    if playback.active {
        playback
            .active_idx
            .and_then(|idx| queue.slots().get(idx))
            .map(|slot| slot.slot_id)
    } else {
        pending_slot
    }
}

struct QueueProjectionUpdate {
    link: QueueLink,
    playback: PlaybackState,
    pending_slot: Option<QueueSlotId>,
    fingerprint: QueueProjectionFingerprint,
    rows_changed: bool,
    bucket_only: bool,
    cursor: QueueCursorUpdate,
    slots: Option<Vec<mbv_queue::QueueSlot>>,
    patch: Option<(
        QueueSlotId,
        mbv_render::components::media_list::MediaListRow<QueueSlotId>,
    )>,
}

impl Model {
    pub(in crate::app) fn sync_queue(&mut self) {
        // D2 (`hide-empty-queue-column`): when the queue column is hidden
        // because the displayed queue emptied, the stored focus cannot stay
        // on Queue -- it would resurface when the queue refills and yank
        // focus away from the library the user is browsing. Assign the field
        // directly: `set_panel_focus` would run the queue initial-item path
        // for a queue that is about to be left.
        if self.app.queue_column_hidden_empty() && self.app.panel_focus == PanelFocus::Queue {
            self.app.panel_focus = PanelFocus::Library;
        }
        self.mount_and_focus_queue();
        let update = self.prepare_queue_projection();
        if update.rows_changed {
            self.last_queue_projection = Some(update.fingerprint);
        }
        // The visual slot's image projection (task 3.4, D9): the queue
        // projection — not the painter — issues every fetch for the now-playing
        // item and projects the slot's image state. The slot only paints while
        // playback is active (idle collapse), so the projection follows the
        // same gate the card's render used.
        if self.app.visual_slot_shown() {
            let resizing = self.refresh_resize_drag_hold();
            self.app.refresh_queue_card_image(resizing);
        } else if self.app.now_playing_status() != NowPlayingStatus::Idle {
            // The slot is hidden while playing (the user's hide toggle): the
            // artwork can no longer carry the title, so the header is its
            // only home again — the carried-over site must not leave the
            // header on its brand row over a slot that paints nothing.
            self.app.queue_card_projection.title_site =
                mbv_ui_model::playback::NowPlayingTitleSite::Header;
        }
        self.push_queue_projection(update);
    }

    fn mount_and_focus_queue(&mut self) {
        let id = ComponentId::Queue;
        if !self.application.mounted(&id) {
            self.application
                .mount(id.clone(), Box::new(QueueComponent::new()), vec![])
                .expect("mount Queue");
        }

        let queue_focused = matches!(self.app.effective_panel_focus(), PanelFocus::Queue)
            && !self.blocking_overlay_active();
        // A mounted sidebar/modal/popup owns focus while it is up; re-activating
        // Queue here would steal the keypress it needs to close itself (mini
        // view keeps `effective_panel_focus` on Queue, so this pass fires every
        // tick otherwise). Prefix-armed capture (design D6, task 6.1) holds
        // focus off while armed, so this pass must not re-activate mid-capture.
        if !self.overlay_holds_focus() && !self.app.prefix_armed {
            if queue_focused {
                if self.application.focus() != Some(&id) {
                    self.application.active(&id).expect("activate Queue");
                }
            } else if self.application.focus() == Some(&id) {
                self.application.blur().expect("blur Queue");
            }
        }
    }

    fn prepare_queue_projection(&mut self) -> QueueProjectionUpdate {
        let link = self.app.playing_queue_link();
        let playback = self.app.displayed_queue_playback_state();
        let pending_slot = self.pending_queue_projection_slot(link);
        let fingerprint = self.queue_projection_fingerprint(link, playback, pending_slot);
        let (rows_changed, bucket_only) = self.queue_projection_changes(&fingerprint);
        let cursor = self.queue_projection_cursor(link);
        let mut update = QueueProjectionUpdate {
            link,
            playback,
            pending_slot,
            fingerprint,
            rows_changed,
            bucket_only,
            cursor,
            slots: None,
            patch: None,
        };
        self.prepare_queue_projection_rows(&mut update);
        update
    }

    // The adopted pending slot counts only while it is still in the shown
    // queue: a stale target must not move now-playing off the observed row.
    fn pending_queue_projection_slot(&self, link: QueueLink) -> Option<QueueSlotId> {
        self.app
            .pending_playback_slot()
            .filter(|target| self.app.queue_for_link(link).slot(*target).is_some())
    }

    fn queue_projection_fingerprint(
        &self,
        link: QueueLink,
        playback: PlaybackState,
        pending_slot: Option<QueueSlotId>,
    ) -> QueueProjectionFingerprint {
        let queue = self.app.queue_for_link(link);
        QueueProjectionFingerprint {
            revision: queue.revision().raw(),
            lineage: queue.lineage(),
            link,
            active: playback.active,
            active_target: projected_active_target(queue, playback, pending_slot),
            pending_target: pending_slot,
            progress_bucket: progress_bucket(playback),
        }
    }

    // `sync_queue` runs on every run-loop tick. When nothing the projection
    // depends on changed and no authoritative cursor re-anchor is armed,
    // rebuilding rows would only reproduce current content -- skip it (#675).
    fn queue_projection_changes(&self, fingerprint: &QueueProjectionFingerprint) -> (bool, bool) {
        let previous = self.last_queue_projection.as_ref();
        let rows_changed = previous != Some(fingerprint);
        let bucket_only = previous.is_some_and(|old| {
            old.revision == fingerprint.revision
                && old.lineage == fingerprint.lineage
                && old.link == fingerprint.link
                && old.active == fingerprint.active
                && old.active_target == fingerprint.active_target
                && old.pending_target == fingerprint.pending_target
                && old.progress_bucket != fingerprint.progress_bucket
        });
        (rows_changed, bucket_only)
    }

    // Re-anchor only for authoritative content changes; routine updates preserve
    // the component-owned cursor. Cursor and chrome delivery is intentionally
    // independent of the row fingerprint.
    fn queue_projection_cursor(&mut self, link: QueueLink) -> QueueCursorUpdate {
        match self.app.pending_queue_cursor_reanchor.take() {
            Some(reanchor) if reanchor == link => {
                QueueCursorUpdate::Set(self.app.queue_for_link(link).cursor())
            }
            _ => QueueCursorUpdate::Preserve,
        }
    }

    fn prepare_queue_projection_rows(&self, update: &mut QueueProjectionUpdate) {
        update.slots = (!update.bucket_only && update.rows_changed)
            .then(|| self.app.queue_for_link(update.link).slots().to_vec());
        update.patch = update
            .bucket_only
            .then(|| {
                let queue = self.app.queue_for_link(update.link);
                let target = update.fingerprint.active_target?;
                let index = queue.slot_index(target)?;
                let slot = queue.slots().get(index)?;
                Some((
                    target,
                    mbv_components::queue::queue_media_row(
                        slot,
                        index,
                        update.playback,
                        update.pending_slot,
                    ),
                ))
            })
            .flatten();
    }

    fn push_queue_projection(&mut self, update: QueueProjectionUpdate) {
        let id = ComponentId::Queue;
        if let Some(comp) = self.application.get_component_mut(&id)
            && let Some(queue) = comp.as_any_mut().downcast_mut::<QueueComponent>()
        {
            queue.set_pending_slot(update.pending_slot);
            if let Some(slots) = update.slots {
                queue.set_rows(&slots, update.playback);
            } else if let Some((target, row)) = update.patch {
                queue.set_row_patch(target, row);
            }
            queue.set_cursor(&update.cursor);
            queue.set_link(update.link);
            // The footer pills are all queue concern (owner indicator, playlist
            // source, autosave prefix, dirty suffix) — never the library
            // column's status bar. The owner pill always names the shown
            // Player owner (design D7), so it flips in the same sync pass the
            // shown link flips; the playback target's host lives on the queue
            // header row.
            queue.set_status_pills(
                self.app.queue_owner_status_spans(),
                self.app.playlist_status_spans(),
                self.app.autosave_status_spans(),
                self.app.dirty_status_spans(),
            );
            // The queue panel's placement (task 3.1): computed from the
            // same paint-free checkpoint the draw path consumes (the last
            // published card geometry; see `App::queue_panel_placement`).
            // The frame is an appearance surface: unpinned mini view always
            // rests; a pinned mini view follows the window focus, while 80+
            // columns ignores window focus.
            queue.set_frame_focused(self.app.panel_appearance_focus(PanelFocus::Queue));
            queue.set_mini_view(self.app.is_mini_view());
            queue.set_area(self.app.queue_panel_placement().panel_area);
        }
    }

    pub(in crate::app) fn render_queue_boundary_at(
        &mut self,
        frame: &mut ratatui::Frame,
        area: ratatui::layout::Rect,
    ) {
        let id = ComponentId::QueueBoundary;
        if self.application.mounted(&id) {
            self.application.view(&id, frame, area);
        }
    }

    pub(in crate::app) fn render_queue_panel_at(
        &mut self,
        frame: &mut ratatui::Frame,
        placement: ratatui::layout::Rect,
    ) {
        let id = ComponentId::Queue;
        if !self.application.mounted(&id) {
            return;
        }
        // The queue panel paints its whole surface (frame, title, status,
        // list) at the placement computed by the root loop (task 3.1).
        if placement.width == 0 || placement.height == 0 {
            return;
        }
        if let Some(comp) = self.application.get_component_mut(&id)
            && let Some(queue) = comp.as_any_mut().downcast_mut::<QueueComponent>()
        {
            queue.set_area(placement);
        }
        self.application.view(&id, frame, placement);
    }

    pub(in crate::app) fn handle_queue_request(&mut self, request: QueueRequest) {
        match request {
            QueueRequest::Cursor { link, slot_id } => {
                // Plain navigation: select_queue_slot applies the link/focus/
                // hold-window side effects; there is no effect to drive with
                // the resolved index (D2), so it is discarded.
                let _ = self.select_queue_slot(link, slot_id);
            }
            QueueRequest::Play { link, slot_id } => {
                if let Some(index) = self.select_queue_slot(link, slot_id) {
                    self.app
                        .dispatch(&crate::app::dispatch::action::AppCommand::QueuePlayCursor(
                            index,
                        ));
                }
            }
            QueueRequest::Remove { link, slot_id } => {
                if let Some(index) = self.select_queue_slot(link, slot_id) {
                    self.app.remove_from_queue(index);
                }
            }
            QueueRequest::RemoveSelection { link, slot_ids } => {
                // One batch edit: the owner applies the whole range before
                // publishing a queue snapshot, so the list does not shrink one
                // row per removal.
                self.app.remove_slots_from_queue(link, &slot_ids);
            }
            QueueRequest::Move {
                link,
                slot_id,
                direction,
            } => {
                self.handle_queue_move(link, slot_id, direction);
            }
            QueueRequest::MoveTo {
                link,
                slot_id,
                onto,
            } => {
                self.handle_queue_move_to(link, slot_id, onto);
            }
            QueueRequest::ResizeColumnLive(width) => {
                self.app.queue_column_width = width;
            }
            // The boundary only emits End after a Live move, so persist
            // unconditionally rather than tracking the drag's start width.
            QueueRequest::ResizeColumnEnd(width) => {
                self.app.queue_column_width = width;
                self.app.save_prefs();
            }
            QueueRequest::Undo { link } => {
                self.handle_queue_undo(link);
            }
        }
    }

    fn handle_queue_move(&mut self, link: QueueLink, slot_id: QueueSlotId, direction: QueueMove) {
        if let Some(index) = self.select_queue_slot(link, slot_id) {
            match direction {
                QueueMove::Up => self.app.move_queue_item_up(index),
                QueueMove::Down => self.app.move_queue_item_down(index),
            }
        }
    }

    fn handle_queue_move_to(&mut self, link: QueueLink, slot_id: QueueSlotId, onto: QueueSlotId) {
        let Some(from) = self.select_queue_slot(link, slot_id) else {
            return;
        };
        let Some(to) = self.slot_index(link, onto) else {
            return;
        };
        if from != to {
            self.app.move_queue_item_to(link, from, to);
        }
    }

    fn handle_queue_undo(&mut self, link: QueueLink) {
        // The component can still be showing (and emit for) `Peer`
        // for a frame after a remote disconnect, before the projection
        // refresh flips it back. Once no direct remote queue exists the
        // visible queue is the Local one, so treat the undo as
        // targeting Local rather than flashing a spurious error.
        let link = if link == QueueLink::Peer && !self.app.has_direct_remote_queue() {
            QueueLink::Local
        } else {
            link
        };
        if link == QueueLink::Peer {
            self.app.flash(
                "Undo is not supported for remote queue edits".into(),
                ToastSeverity::Error,
            );
        } else {
            self.app.undo_last_queue_edit(link);
        }
    }

    pub(in crate::app) fn handle_queue_intent(&mut self, intent: QueueIntent) {
        match intent {
            QueueIntent::Clear => self.app.request_clear_queue(),
            QueueIntent::ResizeColumn(direction) => {
                if self.app.effective_panel_mode() == super::PanelMode::Both {
                    self.app
                        .resize_queue_column(direction == QueueColumnResize::Wider);
                }
            }
            QueueIntent::PlayNow => {
                let (active, current_idx) = {
                    let status = self.app.player.status_snapshot();
                    (status.active, status.current_idx)
                };
                if active {
                    self.app.playback_queue_mut().set_cursor(current_idx);
                    // Jump-to-now-playing is an explicit, authoritative move.
                    self.app.pending_queue_cursor_reanchor = Some(self.app.playing_queue_link());
                } else {
                    self.app
                        .flash("Nothing is playing".into(), ToastSeverity::Error);
                }
            }
            QueueIntent::SavePlaylist => {
                if self.app.local_view.total_queue_len() > 0 {
                    self.app
                        .open_save_playlist_dialog(super::SavePlaylistDialog {
                            input: self.app.queue_playlist_name().to_string(),
                            stage: super::SavePlaylistStage::EnterName,
                        });
                }
            }
            QueueIntent::Navigate { link, slot_id } => {
                let Some(cursor) = self.select_queue_slot(link, slot_id) else {
                    return;
                };
                let Some(item) = self.app.queue_for_link(link).emby_item_at(cursor) else {
                    return;
                };
                let item_id = item.id.clone();
                let item_type = item.item_type.clone();
                let libs = self
                    .app
                    .libs
                    .iter()
                    .enumerate()
                    .map(|(i, lib)| {
                        (
                            i,
                            lib.library.id.clone(),
                            lib.library.collection_type.clone(),
                        )
                    })
                    .collect();
                self.app.spawn_navigate_to_item(item_id, item_type, libs);
            }
        }
    }

    /// Resolves `slot_id` to its index in `link`'s queue and applies the
    /// link/focus/hold-window side effects, returning the resolved index.
    /// The index is the operand for the shell-owned effect the caller is
    /// about to run (D2: `remove_from_queue(index)`, `move_queue_item_up/
    /// down(index)`, `AppCommand::QueuePlayCursor(index)`). The component's
    /// own cursor is authoritative for selection; App's `queue_cursor` is
    /// not written here (task 3.1: the mirror is gone).
    fn select_queue_slot(
        &mut self,
        link: QueueLink,
        slot_id: mbv_queue::QueueSlotId,
    ) -> Option<usize> {
        if link == QueueLink::Peer && !self.app.has_direct_remote_queue() {
            return None;
        }
        let index = self.slot_index(link, slot_id)?;
        self.app.set_panel_focus(PanelFocus::Queue);
        self.app.mark_queue_cursor_user_active();
        Some(index)
    }

    /// Position of `slot_id` in `link`'s queue, if present. Pure lookup with
    /// none of `select_queue_slot`'s link/focus/hold-window side effects.
    pub(crate) fn slot_index(
        &self,
        link: QueueLink,
        slot_id: mbv_queue::QueueSlotId,
    ) -> Option<usize> {
        self.app.slot_index(link, slot_id)
    }
}

#[cfg(test)]
mod tests;
