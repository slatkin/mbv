//! The core thread's pointer routing (design D9): one translated pointer
//! event's canvas work. The event disarms prefix mode, cancels or recognizes
//! the gesture, resolves the pointer's place against the last `LayoutReport`'s
//! hit map, and hands the surface that owns the region to the `App`. The
//! overlay's outside press and the row drag's own gestures run here too, before
//! the generic region path.

use std::time::Instant;

use crate::gui_core::GuiInput;

use mbv_queue::QueueSlotId;
use mbv_surfaces::gesture::PointerGesture;
use mbv_ui_msg::{PointerEvent, PointerKind, Position};

use crate::bridge::layout::{ConfirmTargetId, DaemonLostTargetId, RegionTarget};
use crate::leaves::{
    PlaybackLeaf, QueueLeaf, StatusBarLeaf, TransportAvailability, gesture_point, row_hover_at,
    row_slot_at, seekbar_region,
};

use super::CoreThread;

impl CoreThread {
    /// The Queue row the pointer rests on: the row's own region, or the row
    /// whose hover Play control the pointer rests on, so the control never
    /// clears the hover that paints it (design D6 of `gui-row-hover-actions`).
    pub(super) fn queue_row_at(&self, at: Position) -> Option<QueueSlotId> {
        row_hover_at(&self.regions, at)
    }

    /// Route one translated pointer event (design D4, task row 2.4): disarm
    /// prefix mode first, recognize the gesture over the window's logical
    /// points, resolve the gesture's point against the combined hit map of
    /// the last `LayoutReport`, apply the focus change the matching TUI click
    /// applies, and hand the surface that owns the region to
    /// [`GuiCore::route_gui_pointer`]. A gesture whose point resolves to no
    /// reported region — and a `DragEnd`, which carries no point — resolves
    /// to nothing (`client-interaction-core`: pointer targets resolve from a
    /// semantic hit map); the disarm precedes all of it, so every pointer
    /// event cancels an armed prefix even when nothing resolves.
    pub(super) fn route_pointer(&mut self, event: PointerEvent) -> bool {
        // Any pointer event disarms prefix mode first — the shared contract
        // `GuiCore::route_gui_input`'s pointer arm carries, the same silent
        // disarm every terminal mouse event applies in the TUI. The call
        // runs before the gesture and region resolution, so a hover move, a
        // drag end, or a press on no reported region still cancels an armed
        // prefix; the pointer arm itself delivers nothing else.
        self.core.route_gui_input(GuiInput::Pointer(event), None);
        // A cancelled gesture (the window lost the pointer, or the pointer
        // left the window; never an element's blur, which every press on a
        // row causes) drops the drag state and the double-click pair without
        // a move (add-tauri-gui-client row 6.5, finding
        // P1-2): the release that would have ended the drag never arrives.
        if event.kind == PointerKind::Cancel {
            self.pointer_at = None;
            self.gestures.cancel();
            self.queue.cancel_row_drag();
            self.queue.set_hover(None);
            self.playback.set_scrubber_hover(None);
            return false;
        }
        self.pointer_at = Some(event.position);
        // A new press while a row drag is armed discards the stale drag
        // first: a release the window never saw cannot make the next click's
        // DragEnd move the old slot.
        if matches!(event.kind, PointerKind::Press | PointerKind::RightPress) {
            self.queue.cancel_row_drag();
        }
        // The scrubber's hover preview (design D4: "a move over the
        // scrubber carries the hover"): the event's position resolves
        // against the painted bar through the same fraction rule a seek
        // uses, and a position away from the bar clears it.
        let hover = seekbar_region(&self.regions, event.position)
            .and_then(|bar| self.playback.seek_target(bar, event.position.x));
        self.playback.set_scrubber_hover(hover);
        // The canvas queue's row hover (add-tauri-gui-client row 6.5): the row
        // the pointer rests on. The window paints that row's hover fill.
        let row_hover = self.queue_row_at(event.position);
        self.queue.set_hover(row_hover);
        // An open overlay owns the keyboard and the window's presses: a press
        // outside its own targets dismisses it and does not reach what is
        // painted beneath (the canvas's outside press; the TUI's blocking
        // context menu dismisses the same way).
        if self.press_outside_open_overlay(&event) {
            self.core.dismiss_open_gui_overlay();
            return false;
        }
        let now = Instant::now();
        let Some(gesture) =
            self.gestures
                .recognize(event.kind, event.modifiers, event.position, now)
        else {
            return false;
        };
        // A press names the gesture it recognized and the region it landed
        // on, so a click that does nothing (a double click that never plays)
        // shows in the log where it stopped.
        if matches!(event.kind, PointerKind::Press) {
            let gesture_label = gesture_name(&gesture);
            let region = self.regions.resolve(event.position);
            tracing::info!(
                name: "gui.pointer.press",
                target: "gui_pointer",
                { x = event.position.x, y = event.position.y, gesture = gesture_label, region = ?region },
                "press recognized"
            );
        }
        // The row drag's motion and release address the Queue Surface state
        // directly (add-tauri-gui-client row 6.5, design D5): motion only
        // previews the drop, and the release emits the one move request. A
        // drag the row drag does not own keeps the generic path (the
        // seekbar's scrub is a drag too).
        if matches!(
            gesture,
            PointerGesture::Drag { .. } | PointerGesture::DragEnd
        ) && self.route_row_drag_gesture(gesture)
        {
            return false;
        }
        let Some(point) = gesture_point(&gesture) else {
            return false;
        };
        match self.regions.resolve(point) {
            Some(RegionTarget::Queue(_)) => {
                // The TUI click's effects, run through the Queue leaf: a click,
                // double click, or right click on a queue row dispatches the
                // TUI's `QueueRowClick` and `QueueRowActivate`, which move the
                // Queue's own cursor (the GUI's active row) and its marks. A
                // press stores no focus (ADR 0040): the window has none, and
                // the next key sets the core's Panel focus from its own
                // target. The pill and the wheel take no focus, exactly as the
                // TUI's matching gestures do not.
                let Self {
                    core,
                    queue,
                    regions,
                    ..
                } = self;
                let mut leaf = QueueLeaf::new(queue, regions);
                core.route_gui_pointer(gesture, Some(&mut leaf));
            }
            Some(RegionTarget::QueuePlayback(_)) => {
                // The TUI's playback-panel click applies no focus change, so
                // the core applies none: the press works while the Queue
                // panel holds focus (design D4). The projection's
                // availability gates the transport; the chooser and popover
                // rows resolve their painted indexes against the last
                // sent choice lists, not this read.
                let projection = self.core.app_mut().queue_playback_projection();
                let availability = projection.availability;
                let Self {
                    core,
                    playback,
                    regions,
                    painted,
                    ..
                } = self;
                let mut leaf = PlaybackLeaf::new(
                    playback,
                    regions,
                    TransportAvailability {
                        stop: availability.stop,
                        previous: availability.previous,
                        next: availability.next,
                    },
                    painted,
                );
                core.route_gui_pointer(gesture, Some(&mut leaf));
            }
            Some(RegionTarget::StatusBar(_)) => {
                // The TUI's status-row click applies no focus change either.
                let Self { core, regions, .. } = self;
                let mut leaf = StatusBarLeaf::new(regions);
                core.route_gui_pointer(gesture, Some(&mut leaf));
            }
            // A pre-attach button region is the attach campaign's target: the
            // attached slice resolves nothing for it, even when a stale
            // report carried one — and so does a point in no reported region.
            // A confirm button's press answers the modal through the same
            // execution the button's key hint applies; the outside-press
            // dismissal above already left confirm regions un-dismissed.
            Some(RegionTarget::Confirm(ConfirmTargetId::Button(index))) => {
                self.confirm_button_press(gesture, *index);
            }
            // The daemon-lost dialog's buttons answer through the same
            // execution their key hints apply; the press that lands outside
            // them left the dialog up (it is not dismissible).
            Some(RegionTarget::DaemonLost(DaemonLostTargetId::Button(index))) => {
                return self.daemon_lost_button_press(gesture, *index);
            }
            Some(RegionTarget::Attach(_)) | None => {}
        }
        false
    }

    /// Deliver a gesture the row drag owns to the Queue leaf: motion previews
    /// the drop and the release sends the move request, both through the leaf
    /// entry so a message the drag arm returns is dispatched, never dropped.
    /// Returns whether the drag owned the gesture; `false` leaves it to the
    /// generic region path (a drag whose press anchor landed off the rows is
    /// no row drag — the seekbar's scrub keeps the generic path).
    fn route_row_drag_gesture(&mut self, gesture: PointerGesture<Position>) -> bool {
        if self.queue.row_drag().is_none() {
            // A drag begins on the first motion after a press on a row (user
            // request 2026-11-23: the rows carry no grip): the gesture's press
            // anchor names the row the press landed on. A press with no
            // motion arms no drag, so a plain click paints no drag chrome.
            let PointerGesture::Drag { from, .. } = gesture else {
                return false;
            };
            let Some(slot) = row_slot_at(&self.regions, from) else {
                return false;
            };
            if !self.queue.begin_row_drag(slot) {
                return false;
            }
        }
        let Self {
            core,
            queue,
            regions,
            ..
        } = self;
        let mut leaf = QueueLeaf::new(queue, regions);
        core.route_gui_pointer(gesture, Some(&mut leaf));
        true
    }
}

/// The gesture's name for the press log.
fn gesture_name(gesture: &PointerGesture<Position>) -> &'static str {
    match gesture {
        PointerGesture::Click { .. } => "click",
        PointerGesture::DoubleClick(_) => "double-click",
        PointerGesture::RightClick(_) => "right-click",
        PointerGesture::Scroll { .. } => "scroll",
        PointerGesture::Drag { .. } => "drag",
        PointerGesture::DragEnd => "drag-end",
    }
}
