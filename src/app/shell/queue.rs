use super::Model;
use super::{PanelFocus, PlaybackState, QueueScope};
use crate::app::dispatch::notify::ToastSeverity;
use mbv_components::{QueueComponent, QueueCursorUpdate};
use mbv_queue::QueueSlotId;
use mbv_ui_msg::{ComponentId, QueueColumnResize, QueueIntent, QueueMove, QueueRequest};

/// The row projection inputs that can change queue rows. Chrome and pause
/// state are delivered independently; pause only affects the paint-time
/// progress bucket.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::app) struct QueueProjectionFingerprint {
    revision: u64,
    lineage: mbv_queue::QueueLineage,
    scope: QueueScope,
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
    scope: QueueScope,
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
        self.mount_and_focus_queue();
        let update = self.prepare_queue_projection();
        if update.rows_changed {
            self.last_queue_projection = Some(update.fingerprint);
        }
        // The visual slot's image projection (task 3.4, D9): the queue
        // projection — not the painter — issues every fetch for the now-playing
        // item and projects the slot's image state. The fetch and
        // overlay-compose work stays gated on the slot inside the refresh (the
        // slot only paints while playback is active; idle collapse), but the
        // refresh runs on every sync pass so the header-visibility
        // classification follows live App state ungated (design D1, row 1.2).
        let column_resizing = self.queue_column_resizing();
        self.app.refresh_queue_card_image(column_resizing);
        self.push_queue_projection(update);
    }

    /// Whether the queue column's resize drag is in progress. The boundary
    /// component owns the gesture (`changed` persists across ticks until
    /// `DragEnd`), so the shell asks it rather than mirroring the bit into
    /// `App`; the projection reads the resolved value as an input.
    fn queue_column_resizing(&self) -> bool {
        self.application
            .get_component(&ComponentId::QueueBoundary)
            .and_then(|component| {
                component
                    .as_any()
                    .downcast_ref::<mbv_components::QueueBoundaryComponent>()
            })
            .is_some_and(mbv_components::QueueBoundaryComponent::is_resizing)
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
        let scope = self.app.viewed_queue_scope();
        let playback = self.app.displayed_queue_playback_state();
        let pending_slot = self.pending_queue_projection_slot(scope);
        let fingerprint = self.queue_projection_fingerprint(scope, playback, pending_slot);
        let (rows_changed, bucket_only) = self.queue_projection_changes(&fingerprint);
        let cursor = self.queue_projection_cursor(scope);
        let mut update = QueueProjectionUpdate {
            scope,
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

    // The adopted pending slot counts only while it is still in the viewed
    // queue: a stale target must not move now-playing off the observed row.
    fn pending_queue_projection_slot(&self, scope: QueueScope) -> Option<QueueSlotId> {
        self.app
            .queue_scope_is_playback(scope)
            .then(|| self.app.pending_playback_slot())
            .flatten()
            .filter(|target| self.app.queue_for_scope(scope).slot(*target).is_some())
    }

    fn queue_projection_fingerprint(
        &self,
        scope: QueueScope,
        playback: PlaybackState,
        pending_slot: Option<QueueSlotId>,
    ) -> QueueProjectionFingerprint {
        let queue = self.app.queue_for_scope(scope);
        QueueProjectionFingerprint {
            revision: queue.revision().raw(),
            lineage: queue.lineage(),
            scope,
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
                && old.scope == fingerprint.scope
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
    fn queue_projection_cursor(&mut self, scope: QueueScope) -> QueueCursorUpdate {
        match self.app.pending_queue_cursor_reanchor.take() {
            Some(reanchor) if reanchor == scope => {
                QueueCursorUpdate::Set(self.app.queue_for_scope(scope).cursor())
            }
            _ => QueueCursorUpdate::Preserve,
        }
    }

    fn prepare_queue_projection_rows(&self, update: &mut QueueProjectionUpdate) {
        update.slots = (!update.bucket_only && update.rows_changed)
            .then(|| self.app.queue_for_scope(update.scope).slots().to_vec());
        update.patch = update
            .bucket_only
            .then(|| {
                let queue = self.app.queue_for_scope(update.scope);
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
            queue.set_scope(update.scope);
            // The footer pills are all queue concern (playlist source,
            // autosave, the remote-attachment indicator) — never the
            // library column's status bar.
            let remote_pill = self.app.queue_title_model().remote_pill;
            queue.set_status_pills(
                self.app.playlist_status_spans(),
                self.app.autosave_status_spans(),
                remote_pill,
            );
            // The queue panel's placement (task 3.1): computed from the
            // same paint-free checkpoint the draw path consumes (the last
            // published card geometry; see `App::queue_panel_placement`).
            queue.set_frame_focused(matches!(
                self.app.effective_panel_focus(),
                PanelFocus::Queue
            ));
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
            QueueRequest::Scope(scope) => {
                if scope == QueueScope::Local || self.app.has_direct_remote_queue() {
                    self.app.set_queue_scope(scope);
                }
            }
            QueueRequest::Cursor { scope, slot_id } => {
                // Plain navigation: select_queue_slot applies the scope/focus/
                // hold-window side effects; there is no effect to drive with
                // the resolved index (D2), so it is discarded.
                let _ = self.select_queue_slot(scope, slot_id);
            }
            QueueRequest::Play { scope, slot_id } => {
                if let Some(index) = self.select_queue_slot(scope, slot_id) {
                    self.app
                        .dispatch(&crate::app::dispatch::action::Command::QueuePlayCursor(
                            index,
                        ));
                }
            }
            QueueRequest::Remove { scope, slot_id } => {
                if let Some(index) = self.select_queue_slot(scope, slot_id) {
                    self.app.remove_from_queue(index);
                }
            }
            QueueRequest::RemoveSelection { scope, slot_ids } => {
                // One batch edit: the owner applies the whole range before
                // publishing a queue snapshot, so the list does not shrink one
                // row per removal.
                self.app.remove_slots_from_queue(scope, &slot_ids);
            }
            QueueRequest::Move {
                scope,
                slot_id,
                direction,
            } => {
                self.handle_queue_move(scope, slot_id, direction);
            }
            QueueRequest::MoveTo {
                scope,
                slot_id,
                onto,
            } => {
                self.handle_queue_move_to(scope, slot_id, onto);
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
            QueueRequest::Undo { scope } => {
                self.handle_queue_undo(scope);
            }
        }
    }

    fn handle_queue_move(&mut self, scope: QueueScope, slot_id: QueueSlotId, direction: QueueMove) {
        if let Some(index) = self.select_queue_slot(scope, slot_id) {
            match direction {
                QueueMove::Up => self.app.move_queue_item_up(index),
                QueueMove::Down => self.app.move_queue_item_down(index),
            }
        }
    }

    fn handle_queue_move_to(&mut self, scope: QueueScope, slot_id: QueueSlotId, onto: QueueSlotId) {
        let Some(from) = self.select_queue_slot(scope, slot_id) else {
            return;
        };
        let Some(to) = self.slot_index(scope, onto) else {
            return;
        };
        if from != to {
            self.app.move_queue_item_to(scope, from, to);
        }
    }

    fn handle_queue_undo(&mut self, scope: QueueScope) {
        // The component can still be showing (and emit for) `Remote`
        // for a frame after a remote disconnect, before the projection
        // refresh flips it back. Once no direct remote queue exists the
        // visible queue is the Local one, so treat the undo as
        // targeting Local rather than flashing a spurious error.
        let scope = if scope == QueueScope::Remote && !self.app.has_direct_remote_queue() {
            QueueScope::Local
        } else {
            scope
        };
        if scope == QueueScope::Remote {
            self.app.flash(
                "Undo is not supported for remote queue edits".into(),
                ToastSeverity::Error,
            );
        } else {
            self.app.undo_last_queue_edit(scope);
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
                    let status = self.app.player.status.lock().unwrap();
                    (status.active, status.current_idx)
                };
                if active {
                    self.app.playback_queue_mut().set_cursor(current_idx);
                    self.app.set_queue_scope(QueueScope::Remote);
                    // Jump-to-now-playing is an explicit, authoritative move.
                    self.app.pending_queue_cursor_reanchor = Some(self.app.playing_queue_scope());
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
            QueueIntent::Navigate { scope, slot_id } => {
                let Some(cursor) = self.select_queue_slot(scope, slot_id) else {
                    return;
                };
                let Some(item) = self.app.queue_for_scope(scope).emby_item_at(cursor) else {
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

    /// Resolves `slot_id` to its index in `scope`'s queue and applies the
    /// scope/focus/hold-window side effects, returning the resolved index.
    /// The index is the operand for the shell-owned effect the caller is
    /// about to run (D2: `remove_from_queue(index)`, `move_queue_item_up/
    /// down(index)`, `Command::QueuePlayCursor(index)`). The component's
    /// own cursor is authoritative for selection; App's `queue_cursor` is
    /// not written here (task 3.1: the mirror is gone).
    fn select_queue_slot(
        &mut self,
        scope: QueueScope,
        slot_id: mbv_queue::QueueSlotId,
    ) -> Option<usize> {
        if scope == QueueScope::Remote && !self.app.has_direct_remote_queue() {
            return None;
        }
        let index = self.slot_index(scope, slot_id)?;
        self.app.set_queue_scope(scope);
        self.app.set_panel_focus(PanelFocus::Queue);
        self.app.mark_queue_cursor_user_active();
        Some(index)
    }

    /// Position of `slot_id` in `scope`'s queue, if present. Pure lookup with
    /// none of `select_queue_slot`'s scope/focus/hold-window side effects.
    pub(crate) fn slot_index(
        &self,
        scope: QueueScope,
        slot_id: mbv_queue::QueueSlotId,
    ) -> Option<usize> {
        self.app.slot_index(scope, slot_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::tests::{QueueViewTestExt, live_owner_channel, make_app_stub, make_items};
    use mbv_emby_model::test_support::make_item;
    use mbv_ui_msg::Msg;
    use tuirealm::event::{Event, Key, KeyEvent, KeyModifiers};

    #[test]
    fn queue_rows_follow_readopted_owner_snapshot_with_same_wire_revision_issue_836() {
        let mut first = crate::app::tests::emby_unified_state(&make_items(2), 0);
        first.active_slot = None;
        first.revision = 7;
        let mut app = make_app_stub();
        app.local_view = crate::app::QueueView::from_snapshot(&first);
        let mut model = Model::new(app);

        model.sync_queue();
        let component = queue_component(&model);
        assert_eq!(component.projected_row_states().len(), 2);

        let mut replacement = crate::app::tests::emby_unified_state(&make_items(1), 0);
        replacement.active_slot = None;
        replacement.revision = 7;
        model.app.local_view.adopt(
            &replacement,
            crate::app::state::queue_view::AdoptCause::Replacement,
        );
        model.sync_queue();

        let component = queue_component(&model);
        assert_eq!(component.projected_row_states().len(), 1);
    }

    #[test]
    fn queue_projection_uses_adopted_pending_transition_without_progress() {
        // Owns queue-canonical-list, "Queue projection is bounded presentation data".
        let mut snapshot = crate::app::tests::emby_unified_state(&make_items(2), 0);
        snapshot.in_flight_transition = Some(mbv_ctrl::TransitionSummary {
            request_id: 1,
            generation: 1,
            target_slot: 101,
        });
        let mut app = make_app_stub();
        app.local_view = crate::app::QueueView::from_snapshot(&snapshot);
        let mut status = app.player.status.lock().unwrap();
        status.active = true;
        status.current_idx = 0;
        status.position_ticks = 500;
        status.runtime_ticks = 1_000;
        drop(status);

        let displayed = app.displayed_playback_state();
        let rows = mbv_components::queue::queue_media_rows(
            app.local_view.slots(),
            app.displayed_queue_playback_state(),
            app.pending_playback_slot(),
        );

        assert_eq!(displayed.active_idx, Some(1));
        assert_eq!(displayed.position_ticks, 0);
        assert!(matches!(
            &rows[1],
            mbv_render::components::media_list::MediaListRow::Item {
                semantic_state:
                    mbv_render::components::media_list::MediaSemanticState::NowPlaying {
                        progress: None
                    },
                ..
            }
        ));
    }

    #[test]
    fn unchanged_queue_revision_skips_projection_rebuild() {
        let mut app = make_app_stub();
        app.local_view.adopt_items(make_items(2), 0);
        let mut model = Model::new(app);
        model.sync_queue();

        let update = model.prepare_queue_projection();

        assert!(update.slots.is_none());
    }

    #[test]
    fn lineage_change_rebuilds_same_revision_queue_rows_review_p2_1() {
        // A replacement QueueView restarts its private revision mint; lineage
        // still distinguishes its rows from the prior owner's queue.
        let mut state = crate::app::tests::emby_unified_state(&make_items(2), 0);
        state.active_slot = None;
        state.revision = 7;
        let mut app = make_app_stub();
        app.local_view = crate::app::QueueView::from_snapshot(&state);
        let mut model = Model::new(app);
        model.sync_queue();
        let previous_revision = model.app.local_view.revision();

        let mut replacement_items = make_items(2);
        replacement_items[0].id = "replacement".into();
        let mut replacement = crate::app::tests::emby_unified_state(&replacement_items, 0);
        replacement.active_slot = None;
        replacement.revision = 7;
        replacement.lineage = mbv_queue::QueueLineage(1);
        model.app.local_view = crate::app::QueueView::from_snapshot(&replacement);
        assert_eq!(model.app.local_view.revision(), previous_revision);
        let update = model.prepare_queue_projection();

        assert!(update.slots.is_some());
    }

    #[test]
    fn queue_projection_fetches_now_playing_image_once_and_none_on_repaint() {
        // Task 3.4 (D9): the queue projection — not the painter — issues the
        // visual slot's fetch. One fetch per new now-playing key, and a
        // repaint tick (same fingerprint) starts none: the reservation set
        // and the fetch counters stay untouched. The fixture has no Emby
        // client, so `spawn_image_fetch` balances `image_fetches_active`
        // synchronously — the counters cannot see a duplicate spawn; the
        // reservation set and counters together pin the request count.
        let mut app = make_app_stub();
        let items = crate::app::tests::make_items(2);
        app.local_view.adopt_queue_items(
            items
                .into_iter()
                .map(|item| mbv_queue::QueueItem::Emby(Box::new(item)))
                .collect::<Vec<_>>(),
            0,
        );
        app.panel_focus = PanelFocus::Queue;
        app.images.configure_protocol(None, true);
        {
            let mut status = app.player.status.lock().unwrap();
            status.active = true;
            status.current_idx = 0;
        };
        let mut model = Model::new(app);

        model.sync_queue();
        // `make_items` builds Movies, whose card now reserves the landscape
        // key (`{id}:QB`), not the portrait `{id}:P` (commit 55c37ea71).
        assert!(
            model.app.images.is_loading("id0:QB"),
            "the now-playing key must be reserved by the projection push"
        );
        let fetch_work = model.app.images.fetch_work_snapshot();

        // Repaint tick: nothing changed, so the push starts no new fetch.
        model.sync_queue();
        assert_eq!(model.app.images.fetch_work_snapshot(), fetch_work);

        // A new now-playing key reserves exactly one new key.
        model.app.player.status.lock().unwrap().current_idx = 1;
        model.sync_queue();
        assert!(model.app.images.is_loading("id1:QB"));
    }

    #[test]
    fn hidden_visual_slot_skips_artwork_fetch_until_shown() {
        let mut app = make_app_stub();
        app.local_view.adopt_items(make_items(2), 0);
        app.images.configure_protocol(None, true);
        app.visual_slot_hidden = true;
        {
            let mut status = app.player.status.lock().unwrap();
            status.active = true;
            status.current_idx = 0;
        };
        let mut model = Model::new(app);

        model.sync_queue();
        assert_eq!(model.app.images.card_image_fetch_calls(), 0);
        // `make_items` builds Movies → the landscape card key (commit 55c37ea71).
        assert!(!model.app.images.is_loading("id0:QB"));

        model.app.visual_slot_hidden = false;
        model.sync_queue();
        assert!(model.app.images.card_image_fetch_calls() > 0);
        assert!(model.app.images.is_loading("id0:QB"));
    }

    #[test]
    fn queue_arrow_moves_component_cursor_only_not_app_follow() {
        // QueueRequest::Cursor is plain component navigation: arrowing in
        // the mounted component moves only the component's own cursor; App's
        // `queue_cursor` (the shell-owned follow position) is not written
        // (task 3.2 — the mirror in select_queue_slot is gone).
        let mut app = make_app_stub();
        app.local_view.adopt_queue_items(
            vec![
                mbv_queue::QueueItem::Emby(Box::new(make_item("one", "Movie"))),
                mbv_queue::QueueItem::Emby(Box::new(make_item("two", "Movie"))),
            ],
            0,
        );
        app.panel_focus = PanelFocus::Queue;
        let mut model = Model::new(app);
        model.sync_queue();
        let id = ComponentId::Queue;
        let component_cursor = |model: &Model| {
            model
                .application
                .get_component(&id)
                .and_then(|component| {
                    component
                        .as_any()
                        .downcast_ref::<QueueComponent>()
                        .map(QueueComponent::test_cursor)
                })
                .expect("Queue component mounted")
        };
        assert_eq!(component_cursor(&model), 0);
        assert_eq!(model.app.local_view.cursor(), 0);

        let message = model
            .application
            .get_component_mut(&id)
            .expect("Queue component mounted")
            .on(&Event::Keyboard(KeyEvent {
                code: Key::Down,
                modifiers: KeyModifiers::NONE,
            }));
        let Some(Msg::Queue(request @ QueueRequest::Cursor { .. })) = message else {
            panic!("queue navigation must emit a slot cursor request");
        };
        // The request carries the moved-to slot; the component cursor moved.
        model.handle_queue_request(request);
        assert_eq!(
            component_cursor(&model),
            1,
            "component cursor moved to row 1"
        );
        assert_eq!(
            model.app.local_view.cursor(),
            0,
            "QueueRequest::Cursor must not write App's follow cursor"
        );
    }

    fn emby_items(n: usize) -> Vec<mbv_queue::QueueItem> {
        (0..n)
            .map(|i| mbv_queue::QueueItem::Emby(Box::new(make_item(&format!("row-{i}"), "Movie"))))
            .collect()
    }

    fn queue_component(model: &Model) -> &QueueComponent {
        model
            .application
            .get_component(&ComponentId::Queue)
            .and_then(|component| component.as_any().downcast_ref::<QueueComponent>())
            .expect("Queue component mounted")
    }

    #[test]
    fn remote_undo_falls_back_to_local_when_no_direct_remote_queue() {
        // Finding 5: after a remote disconnect the still-mounted component can
        // emit Undo { scope: Remote } for a frame. With no direct remote queue
        // the visible queue is Local, so the Local undo entry is consumed and
        // its inverse edit is sent to the Local owner instead of flashing a
        // spurious remote-undo-unsupported error.
        use crate::app::state::playback::UndoEntry;
        let mut app = make_app_stub();
        let _cmd_rx = live_owner_channel(&mut app);
        app.local_view.adopt_queue_items(emby_items(2), 0);
        app.queue_undo_stack.push(UndoEntry::Remove {
            index: 0,
            item: Box::new(mbv_queue::QueueItem::Emby(Box::new(make_item(
                "restored", "Movie",
            )))),
        });
        let mut model = Model::new(app);
        assert!(!model.app.has_direct_remote_queue());

        model.handle_queue_request(QueueRequest::Undo {
            scope: QueueScope::Remote,
        });

        assert_ne!(
            model.app.status_severity,
            ToastSeverity::Error,
            "no spurious remote-undo-unsupported error"
        );
        assert!(
            model.app.queue_undo_stack.is_empty(),
            "the Local undo entry was consumed"
        );
    }
}
