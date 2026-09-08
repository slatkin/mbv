use super::components::{
    ComponentId, QueueColumnResize, QueueComponent, QueueCursorUpdate, QueueIntent, QueueMove,
    QueueRequest,
};
use super::shell::Model;
use super::{PanelFocus, PlaybackState, QueueScope};
use crate::app::notify_actions::ToastSeverity;
use mbv_core::playback_queue::QueueSlotId;

/// The row projection inputs that can change queue rows. Chrome and pause state
/// are delivered independently; pause only affects the paint-time throbber.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::app) struct QueueProjectionFingerprint {
    revision: u64,
    scope: QueueScope,
    active: bool,
    active_target: Option<QueueSlotId>,
    progress_bucket: u16,
}

/// Now-playing liveness frames, shared by the queue row and the playback
/// panel (`App::now_playing_throbber_span`) so both stay in lockstep: the
/// horizontal block ramp (plus blank) with progress to its right. Each
/// surface keeps its own style (queue: aqua liveness role; panel: accent);
/// only the glyph set is shared. The advance cadence lives in `shell_run`.
pub(in crate::app) const NOW_PLAYING_THROBBER_FRAMES: [char; 9] =
    [' ', '▏', '▎', '▍', '▌', '▋', '▊', '▉', '█'];

/// The active row's progress bucket: whole-percent, so animation frames and
fn progress_bucket(playback: PlaybackState) -> u16 {
    if playback.active && playback.position_ticks > 0 && playback.runtime_ticks > 0 {
        (playback.position_ticks * 100 / playback.runtime_ticks).clamp(0, 100) as u16
    } else {
        u16::MAX
    }
}

fn projected_active_target(
    queue: &super::PlayerTab,
    playback: PlaybackState,
    pending_slot: Option<QueueSlotId>,
) -> Option<QueueSlotId> {
    if playback.active {
        queue
            .slots()
            .get(playback.active_idx)
            .map(|slot| slot.slot_id)
    } else {
        pending_slot.filter(|target| queue.slots().iter().any(|slot| slot.slot_id == *target))
    }
}

impl Model {
    pub(super) fn sync_queue(&mut self) {
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
        // tick otherwise).
        if !self.overlay_holds_focus() {
            if queue_focused {
                if self.application.focus() != Some(&id) {
                    self.application.active(&id).expect("activate Queue");
                }
            } else if self.application.focus() == Some(&id) {
                self.application.blur().expect("blur Queue");
            }
        }

        let scope = self.app.viewed_queue_scope();
        let playback = self.app.displayed_queue_playback_state();
        let pending_slot = self
            .app
            .queue_scope_is_playback(scope)
            .then(|| self.app.pending_playback_slot())
            .flatten();
        let fingerprint = {
            let queue = self.app.queue_for_scope(scope);
            QueueProjectionFingerprint {
                revision: queue.revision().raw(),
                scope,
                active: playback.active,
                active_target: projected_active_target(queue, playback, pending_slot),
                progress_bucket: progress_bucket(playback),
            }
        };
        let title = self.app.queue_title_model();
        let title_area = self.app.layout.main.queue_title_area;
        // `sync_queue` runs on every run-loop tick. When nothing the projection
        // depends on changed and no authoritative cursor re-anchor is armed,
        // rebuilding the row vec (slot clone + per-row `format!`) would only
        // reproduce the current content -- skip it (#675).
        let previous = self.last_queue_projection.as_ref();
        let rows_changed = previous != Some(&fingerprint);
        let bucket_only = previous.is_some_and(|old| {
            old.revision == fingerprint.revision
                && old.scope == fingerprint.scope
                && old.active == fingerprint.active
                && old.active_target == fingerprint.active_target
                && old.progress_bucket != fingerprint.progress_bucket
        });

        // Re-anchor only for authoritative content changes; routine updates preserve
        // the component-owned cursor. Cursor and chrome delivery is intentionally
        // independent of the row fingerprint.
        let cursor = match self.app.pending_queue_cursor_reanchor.take() {
            Some(reanchor) if reanchor == scope => {
                QueueCursorUpdate::Set(self.app.queue_for_scope(scope).queue_cursor)
            }
            _ => QueueCursorUpdate::Preserve,
        };
        let slots = (!bucket_only && rows_changed)
            .then(|| self.app.queue_for_scope(scope).slots().to_vec());
        let patch = bucket_only
            .then(|| {
                let queue = self.app.queue_for_scope(scope);
                fingerprint.active_target.and_then(|target| {
                    queue
                        .slots()
                        .iter()
                        .enumerate()
                        .find(|(_, slot)| slot.slot_id == target)
                        .map(|(index, slot)| {
                            (
                                target,
                                crate::app::components::queue::queue_media_row(
                                    slot,
                                    index,
                                    playback,
                                    pending_slot,
                                ),
                            )
                        })
                })
            })
            .flatten();
        if rows_changed {
            self.last_queue_projection = Some(fingerprint);
        }
        let throbber = playback.active.then(|| {
            NOW_PLAYING_THROBBER_FRAMES
                [self.app.now_playing_throbber_index % NOW_PLAYING_THROBBER_FRAMES.len()]
        });
        if let Some(comp) = self.application.get_component_mut(&id) {
            if let Some(queue) = comp.as_any_mut().downcast_mut::<QueueComponent>() {
                queue.set_pending_slot(pending_slot);
                if let Some(slots) = slots {
                    queue.set_rows(slots, playback);
                } else if let Some((target, row)) = patch {
                    queue.set_row_patch(&target, row);
                }
                queue.set_throbber(throbber);
                queue.set_cursor(cursor);
                queue.set_scope_chrome(scope, title);
                queue.set_area(self.app.layout.main.queue_area);
                queue.set_title_area(title_area);
            }
        }
    }

    pub(super) fn render_queue_boundary(&mut self, frame: &mut ratatui::Frame) {
        let id = ComponentId::QueueBoundary;
        if self.application.mounted(&id) {
            self.application
                .view(&id, frame, self.app.layout.main.queue_boundary_area);
        }
    }

    pub(super) fn render_queue_component(&mut self, frame: &mut ratatui::Frame) {
        let id = ComponentId::Queue;
        if !self.application.mounted(&id) {
            return;
        }
        if let Some(comp) = self.application.get_component_mut(&id) {
            if let Some(queue) = comp.as_any_mut().downcast_mut::<QueueComponent>() {
                queue.set_area(self.app.layout.main.queue_area);
                queue.set_title_area(self.app.layout.main.queue_title_area);
            }
        }
        self.application
            .view(&id, frame, self.app.layout.main.queue_area);
        self.app.layout.main.queue_selected_item_rect = self
            .application
            .get_component(&id)
            .and_then(|comp| comp.as_any().downcast_ref::<QueueComponent>())
            .and_then(QueueComponent::selected_row_rect);
    }

    pub(super) fn handle_queue_request(&mut self, request: QueueRequest) {
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
                        .dispatch(super::action::Command::QueuePlayCursor(index));
                }
            }
            QueueRequest::Remove { scope, slot_id } => {
                if let Some(index) = self.select_queue_slot(scope, slot_id) {
                    self.app.remove_from_queue(index);
                }
            }
            QueueRequest::Move {
                scope,
                slot_id,
                direction,
            } => {
                if let Some(index) = self.select_queue_slot(scope, slot_id) {
                    match direction {
                        QueueMove::Up => self.app.move_queue_item_up(index),
                        QueueMove::Down => self.app.move_queue_item_down(index),
                    }
                }
            }
            QueueRequest::MoveTo {
                scope,
                slot_id,
                onto,
            } => {
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
            QueueRequest::ResizeColumnLive(width) => {
                self.queue_resize_start_width
                    .get_or_insert(self.app.queue_column_width);
                self.app.queue_column_width = width;
            }
            QueueRequest::ResizeColumnEnd(width) => {
                let Some(start_width) = self.queue_resize_start_width.take() else {
                    return;
                };
                if start_width != width {
                    self.app.queue_column_width = width;
                    self.app.save_prefs();
                }
            }
            QueueRequest::Undo { scope } => {
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
        }
    }

    pub(super) fn handle_queue_intent(&mut self, intent: QueueIntent) {
        match intent {
            QueueIntent::Clear => self.app.request_clear_queue(),
            QueueIntent::ResizeColumn(direction) => {
                if self.app.effective_panel_mode() == super::PanelMode::Both {
                    self.app
                        .resize_queue_column(direction == QueueColumnResize::Wider);
                }
            }
            QueueIntent::StopRemoteTracking => {
                if self.app.remote_tracker.is_some() {
                    self.app.stop_remote_tracking();
                }
            }
            QueueIntent::ReanchorRemoteTracking => {
                if self.app.remote_tracker.is_some() {
                    self.app.reanchor_remote_tracking();
                }
            }
            QueueIntent::PlayNow => {
                let (active, current_idx) = {
                    let status = self.app.player.status.lock().unwrap();
                    (status.active, status.current_idx)
                };
                if active {
                    self.app.playback_queue_mut().queue_cursor = current_idx;
                    if self.app.player.is_remote() {
                        self.app.set_queue_scope(QueueScope::Remote);
                    }
                    // Jump-to-now-playing is an explicit, authoritative move.
                    self.app.pending_queue_cursor_reanchor = Some(self.app.playing_queue_scope());
                } else {
                    self.app
                        .flash("Nothing is playing".into(), ToastSeverity::Error);
                }
            }
            QueueIntent::SavePlaylist => {
                if self.app.player_tab.total_queue_len() > 0 {
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
        slot_id: mbv_core::playback_queue::QueueSlotId,
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
    fn slot_index(
        &self,
        scope: QueueScope,
        slot_id: mbv_core::playback_queue::QueueSlotId,
    ) -> Option<usize> {
        self.app
            .queue_for_scope(scope)
            .slots()
            .iter()
            .position(|slot| slot.slot_id == slot_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::components::{Msg, QueueRequest};
    use crate::app::tests::{make_app_stub, make_item, make_remote_app_stub};
    use tuirealm::event::{Event, Key, KeyEvent, KeyModifiers};

    #[test]
    fn queue_arrow_moves_component_cursor_only_not_app_follow() {
        // QueueRequest::Cursor is plain component navigation: arrowing in
        // the mounted component moves only the component's own cursor; App's
        // `queue_cursor` (the shell-owned follow position) is not written
        // (task 3.2 — the mirror in select_queue_slot is gone).
        let mut app = make_app_stub();
        app.player_tab.set_queue_items(
            vec![
                mbv_core::playback_queue::QueueItem::Emby(Box::new(make_item("one", "Movie"))),
                mbv_core::playback_queue::QueueItem::Emby(Box::new(make_item("two", "Movie"))),
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
        assert_eq!(model.app.player_tab.queue_cursor, 0);

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
            model.app.player_tab.queue_cursor, 0,
            "QueueRequest::Cursor must not write App's follow cursor"
        );
    }

    fn emby_items(n: usize) -> Vec<mbv_core::playback_queue::QueueItem> {
        (0..n)
            .map(|i| {
                mbv_core::playback_queue::QueueItem::Emby(Box::new(make_item(
                    &format!("row-{i}"),
                    "Movie",
                )))
            })
            .collect()
    }

    fn queue_cursor(model: &Model) -> usize {
        model
            .application
            .get_component(&ComponentId::Queue)
            .and_then(|c| c.as_any().downcast_ref::<QueueComponent>())
            .map(QueueComponent::test_cursor)
            .expect("Queue component mounted")
    }

    fn press_down(model: &mut Model) {
        model
            .application
            .get_component_mut(&ComponentId::Queue)
            .expect("Queue component mounted")
            .on(&Event::Keyboard(KeyEvent {
                code: Key::Down,
                modifiers: KeyModifiers::NONE,
            }));
    }

    #[test]
    fn observed_refresh_preserves_user_cursor() {
        let mut app = make_app_stub();
        app.player_tab.set_queue_items(emby_items(3), 0);
        app.panel_focus = PanelFocus::Queue;
        let mut model = Model::new(app);
        model.sync_queue();
        press_down(&mut model);
        assert_eq!(queue_cursor(&model), 1);

        model.sync_queue();
        assert_eq!(queue_cursor(&model), 1);
        assert!(model.app.pending_queue_cursor_reanchor.is_none());
    }

    #[test]
    fn projection_gate_still_sees_a_queue_mutation() {
        // The #675 fingerprint gate must not starve real content changes:
        // a structural mutation bumps QueueRevision, so the next sync_queue
        // rebuilds the rows even though the gate skipped the idle ticks before.
        let mut app = make_app_stub();
        app.player_tab.set_queue_items(emby_items(3), 0);
        app.panel_focus = PanelFocus::Queue;
        let mut model = Model::new(app);
        model.sync_queue();
        model.sync_queue(); // idle tick: gated
        assert_eq!(queue_cursor(&model), 0);

        // Select row 1, then reorder that slot to the end in place. This bumps
        // QueueRevision but leaves slot count, scope, playback scalars and the
        // title untouched -- so only `revision` in the fingerprint can catch it.
        press_down(&mut model);
        assert_eq!(queue_cursor(&model), 1);
        let slot = model.app.player_tab.slot_id_at(1).unwrap();
        model.app.player_tab.move_slot(slot, 2);
        model.sync_queue();
        assert_eq!(
            queue_cursor(&model),
            2,
            "the gate rebuilt rows on a revision-only change"
        );
    }

    #[test]
    fn cursor_reanchor_is_scope_aware() {
        let mut app = make_remote_app_stub(
            crate::app::tests::make_items(3),
            crate::app::tests::make_items(3),
        );
        app.queue_scope = QueueScope::Local;
        app.panel_focus = PanelFocus::Queue;
        let mut model = Model::new(app);
        model.sync_queue();

        model.app.player_tab.queue_cursor = 2;
        model.app.pending_queue_cursor_reanchor = Some(QueueScope::Remote);
        model.sync_queue();
        assert_eq!(queue_cursor(&model), 0);
        assert!(model.app.pending_queue_cursor_reanchor.is_none());

        model.app.pending_queue_cursor_reanchor = Some(QueueScope::Local);
        model.sync_queue();
        assert_eq!(queue_cursor(&model), 2);
        assert!(model.app.pending_queue_cursor_reanchor.is_none());
    }

    #[test]
    fn full_replacement_reanchors_instead_of_preserving() {
        let mut app = make_app_stub();
        app.player_tab.set_queue_items(emby_items(3), 0);
        app.panel_focus = PanelFocus::Queue;
        let mut model = Model::new(app);
        model.sync_queue();

        press_down(&mut model);
        press_down(&mut model);
        assert_eq!(queue_cursor(&model), 2);

        model
            .app
            .replace_playback_queue(crate::app::tests::make_items(4), 1);
        assert_eq!(
            model.app.pending_queue_cursor_reanchor,
            Some(QueueScope::Local),
            "a replacement arms a re-anchor"
        );

        model.sync_queue();
        assert_eq!(queue_cursor(&model), 1);
    }

    #[test]
    fn remote_undo_falls_back_to_local_when_no_direct_remote_queue() {
        // Finding 5: after a remote disconnect the still-mounted component can
        // emit Undo { scope: Remote } for a frame. With no direct remote queue
        // the visible queue is Local, so undo the Local edit instead of
        // flashing an error.
        use crate::app::types_playback::UndoEntry;
        let mut app = make_app_stub();
        app.player_tab.set_queue_items(emby_items(2), 0);
        app.queue_undo_stack.push(UndoEntry::Remove(
            0,
            mbv_core::playback_queue::QueueItem::Emby(Box::new(make_item("restored", "Movie"))),
        ));
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
        assert_eq!(
            model.app.player_tab.total_queue_len(),
            3,
            "the removed item was restored to the Local queue"
        );
    }
}
