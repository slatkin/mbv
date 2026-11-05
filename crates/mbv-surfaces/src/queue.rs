//! The Queue Surface state (add-gui-front-end design D5): the Queue's cursor,
//! marks, selection, viewport, and key rules, front-end-neutral. The TUI and
//! GUI Clients hold this state and paint from it; neither reimplements a rule.

use crate::media_list::{MediaList, MediaListOperation, MediaListTransition, RowIntent};
use crate::{HitMap, ListCapacity};
use mbv_queue::QueueSlotId;
use mbv_ui_model::context_menu::ContextMenuTargets;
use mbv_ui_model::media_list::{MediaListRow, SelectionOrigin};
use mbv_ui_model::playback::QueueLink;
use mbv_ui_msg::{
    Key, KeyEvent, KeyModifiers, LeafKeyResult, Msg, QueueColumnResize, QueueIntent, QueueMove,
    QueueRequest, SelectionSummary, ShellRequest,
};

/// Why the shell is pushing a cursor. `Preserve` keeps the user's selection
/// pinned to its slot across a content refresh; `Set` is an authoritative
/// move (follow-the-playhead, jump-to-now-playing, wheel scroll, link switch)
/// that must win over slot-identity reconciliation.
#[derive(Debug)]
pub enum QueueCursorUpdate {
    Preserve,
    Set(usize),
}

/// The semantic target of one painted Queue region (design D6): what a
/// pointer press on the region resolves to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QueueHitTarget {
    /// A queue row painted for this stable slot identity.
    Slot(QueueSlotId),
    /// The playlist pill in the queue footer; a press there opens the
    /// playlists overlay.
    PlaylistPill,
}

/// One frame's Queue interaction report (design D6): how many whole queue
/// rows the painted list fits, and the painted regions a pointer press
/// resolves against. A Client derives it from its own painted geometry and
/// hands it to the Client core; the core reads it and never computes
/// geometry itself.
#[derive(Debug, Clone, PartialEq)]
pub struct QueueFrameReport {
    /// The whole queue rows the painted list fits. It is the same viewport
    /// height the Surface state clamps its resting scroll with, so the core
    /// derives the same paging stride the TUI Client shows.
    pub capacity: ListCapacity,
    /// The frame's painted regions and their semantic targets. Rows resolve
    /// to their stable slot; the playlist pill resolves to the pill target.
    pub hit_map: HitMap<QueueHitTarget>,
}

/// The Queue's Surface state: the canonical media-list owner over the queue's
/// stable slot identities, the queue link the key rules address, and the
/// painted viewport height the Client reports each frame.
#[derive(Debug)]
pub struct QueueSurface {
    list: MediaList<QueueSlotId>,
    link: QueueLink,
    /// The painted viewport height the Client reports; the page stride's
    /// source and the resting scroll clamp's bound.
    viewport_height: usize,
}

impl Default for QueueSurface {
    fn default() -> Self {
        Self::new()
    }
}

impl QueueSurface {
    #[must_use]
    pub fn new() -> Self {
        Self {
            list: MediaList::new(),
            link: QueueLink::Local,
            viewport_height: 0,
        }
    }

    /// The canonical owner, for content plumbing (content set, row patch) the
    /// Client performs and for the reads painting needs.
    #[must_use]
    pub fn list(&self) -> &MediaList<QueueSlotId> {
        &self.list
    }

    /// The canonical owner, mutably, for the same plumbing.
    pub fn list_mut(&mut self) -> &mut MediaList<QueueSlotId> {
        &mut self.list
    }

    /// The queue link the key rules address.
    #[must_use]
    pub fn link(&self) -> QueueLink {
        self.link
    }

    /// Deliver the current link independently of row delivery. Returns
    /// whether the link switched (the resting scroll reset then, and the
    /// Client invalidates its painted geometry).
    pub fn set_link(&mut self, link: QueueLink) -> bool {
        let changed = link != self.link;
        if changed {
            self.list.set_scroll(0);
        }
        self.link = link;
        changed
    }

    /// Deliver an authoritative cursor command independently of row delivery.
    /// This is the adjudicated Queue shell-push seam (design.md D5): the shell
    /// owns the Queue cursor command, so this one numeric re-anchor is
    /// sanctioned rather than delegated. The resting scroll-to-cursor clamp
    /// runs only inside the `Set` arm; `Preserve` leaves a freely scrolled
    /// viewport where the wheel left it. Returns whether the clamp moved the
    /// resting scroll (the Client invalidates its painted geometry then).
    pub fn set_cursor(&mut self, cursor: &QueueCursorUpdate) -> bool {
        let QueueCursorUpdate::Set(idx) = cursor else {
            return false;
        };
        self.list.select_index(*idx);
        let scroll = self.list.scroll();
        let clamped = scroll.min(self.list.cursor());
        if clamped != scroll {
            self.list.set_scroll(clamped);
            return true;
        }
        false
    }

    /// Report the painted viewport height and clamp the resting scroll to it.
    /// Returns whether the clamp moved the offset (the Client invalidates its
    /// painted geometry then).
    pub fn set_viewport_height(&mut self, height: usize) -> bool {
        self.viewport_height = height;
        let offset = self.list.resolve_viewport(height.max(1)).offset;
        if self.list.scroll() != offset {
            self.list.set_scroll(offset);
            return true;
        }
        false
    }

    /// Offer one already-normalized row-local input whose wheel clamp needs
    /// the Client's painted viewport length (design D3). Key rules delegate
    /// through [`Self::key`], which needs no painted geometry.
    pub fn delegate_operation_with_viewport_len(
        &mut self,
        operation: MediaListOperation<QueueSlotId>,
        viewport_len: usize,
    ) -> MediaListTransition<QueueSlotId> {
        self.list
            .delegate_operation_with_viewport_len(operation, viewport_len)
    }

    /// The Queue's selection summary: the mark count under the Queue origin.
    #[must_use]
    pub fn selection_summary(&self) -> SelectionSummary {
        SelectionSummary {
            count: self.list.multi_selection().len(),
            origin: SelectionOrigin::Queue,
        }
    }

    /// Stable targets currently marked for a bulk action.
    #[must_use]
    pub fn multi_selection(&self) -> &[QueueSlotId] {
        self.list.multi_selection()
    }

    /// The stable identity under the cursor.
    #[must_use]
    pub fn selected_target(&self) -> Option<&QueueSlotId> {
        self.list.selected_target()
    }

    /// Exit Visual mode and discard all marked targets.
    pub fn clear_selection(&mut self) {
        self.list.clear_selection();
    }

    /// Content-preserving presentation reset: keep the rows, drop the marks,
    /// and return the cursor and viewport to their initial position.
    pub fn reset_presentation(&mut self) {
        self.list.reset_presentation();
    }

    /// Every display row in paint order.
    #[must_use]
    pub fn rows(&self) -> &[MediaListRow<QueueSlotId>] {
        self.list.rows()
    }

    /// No selectable rows at all.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.list.is_empty()
    }

    /// The cursor as an index into the selectable rows.
    #[must_use]
    pub const fn cursor(&self) -> usize {
        self.list.cursor()
    }

    /// The resting scroll offset (pre height-aware clamp).
    #[must_use]
    pub const fn scroll(&self) -> usize {
        self.list.scroll()
    }

    /// The painted viewport height the Client last reported (the capacity
    /// report's source: the whole queue rows the painted list fits).
    #[must_use]
    pub const fn viewport_height(&self) -> usize {
        self.viewport_height
    }

    /// The Queue's key rules over the neutral `KeyEvent` (moved from the TUI
    /// adapter's `queue/keys.rs`, add-gui-front-end row 4.3). The neutral
    /// vocabulary folds a Shift+uppercase press into its lowercase letter
    /// with SHIFT held; the SHIFT guards below keep the match set the
    /// `TuiRealm` rules had, where only the literal lowercase char matched.
    pub fn key(&mut self, key: KeyEvent) -> LeafKeyResult {
        match self.handle_key(key) {
            Some(message) => LeafKeyResult::Consumed(Some(Box::new(message))),
            None if Self::consumes_without_action(&key) => LeafKeyResult::Consumed(None),
            None => LeafKeyResult::Unhandled,
        }
    }

    /// The keys the Queue consumes without producing a message: the list
    /// navigation keys, and the letters its intents read. Only the literal
    /// lowercase chars match, so a Shift-modified letter press stays
    /// unhandled exactly as the `TuiRealm` rules left it.
    fn consumes_without_action(key: &KeyEvent) -> bool {
        match key.code {
            Key::Up
            | Key::Down
            | Key::PageUp
            | Key::PageDown
            | Key::Home
            | Key::End
            | Key::Left
            | Key::Right
            | Key::Enter
            | Key::Delete
            | Key::Char('.') => true,
            Key::Char('i' | 'p' | 's' | 'c' | 'z') => !key.modifiers.contains(KeyModifiers::SHIFT),
            _ => false,
        }
    }

    fn handle_key(&mut self, key: KeyEvent) -> Option<Msg> {
        if self.list.handle_visual_key(&key).is_some() {
            return Some(Msg::Shell(Box::new(ShellRequest::SelectionProjection(
                self.selection_summary(),
            ))));
        }
        if let Some(message) = Self::handle_resize_key(&key) {
            return Some(message);
        }
        match key.code {
            Key::Up | Key::Down => self.handle_vertical_key(&key),
            Key::PageUp | Key::PageDown | Key::Home | Key::End => self.handle_navigation_key(&key),
            Key::Enter => self.activate_selected_row(),
            Key::Delete => self.delete_selection(),
            Key::Char('z')
                if key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::SHIFT) =>
            {
                Some(Msg::Queue(QueueRequest::Undo { link: self.link }))
            }
            Key::Char('.') if key.modifiers.is_empty() => Some(self.open_context_menu()),
            Key::Char('i') if !key.modifiers.contains(KeyModifiers::SHIFT) => {
                self.navigate_to_selection()
            }
            Key::Char('p') if !key.modifiers.contains(KeyModifiers::SHIFT) => Some(Msg::Shell(
                Box::new(ShellRequest::QueueIntent(QueueIntent::PlayNow)),
            )),
            Key::Char('s')
                if key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::SHIFT) =>
            {
                Some(Msg::Shell(Box::new(ShellRequest::QueueIntent(
                    QueueIntent::SavePlaylist,
                ))))
            }
            Key::Char('c')
                if !key.modifiers.contains(KeyModifiers::ALT)
                    && !key.modifiers.contains(KeyModifiers::SHIFT) =>
            {
                Some(Msg::Shell(Box::new(ShellRequest::QueueIntent(
                    QueueIntent::Clear,
                ))))
            }
            _ => None,
        }
    }

    fn handle_resize_key(key: &KeyEvent) -> Option<Msg> {
        if !matches!(key.code, Key::Left | Key::Right) || key.modifiers != KeyModifiers::SHIFT {
            return None;
        }
        let resize = if key.code == Key::Left {
            QueueColumnResize::Narrower
        } else {
            QueueColumnResize::Wider
        };
        Some(Msg::Shell(Box::new(ShellRequest::QueueIntent(
            QueueIntent::ResizeColumn(resize),
        ))))
    }

    fn handle_vertical_key(&mut self, key: &KeyEvent) -> Option<Msg> {
        if key.modifiers.is_empty() {
            return match key.code {
                Key::Up => self.move_cursor(-1),
                Key::Down => self.move_cursor(1),
                _ => None,
            };
        }
        if key.modifiers.contains(KeyModifiers::SHIFT) {
            let direction = if key.code == Key::Up {
                QueueMove::Up
            } else {
                QueueMove::Down
            };
            return self.selected_slot().map(|(link, slot_id)| {
                Msg::Queue(QueueRequest::Move {
                    link,
                    slot_id,
                    direction,
                })
            });
        }
        None
    }

    fn handle_navigation_key(&mut self, key: &KeyEvent) -> Option<Msg> {
        if !key.modifiers.is_empty() {
            return None;
        }
        match key.code {
            Key::PageUp => self.move_cursor(-self.page_stride()),
            Key::PageDown => self.move_cursor(self.page_stride()),
            Key::Home => {
                self.list.delegate_operation(MediaListOperation::First);
                self.cursor_message()
            }
            Key::End => {
                self.list.delegate_operation(MediaListOperation::Last);
                self.cursor_message()
            }
            _ => None,
        }
    }

    /// The page stride: one row short of the reported viewport height, at
    /// least one row — the same stride the adapter's content height produced.
    fn page_stride(&self) -> i64 {
        let stride = self.viewport_height.saturating_sub(1).max(1);
        i64::try_from(stride).unwrap_or(i64::MAX)
    }

    fn move_cursor(&mut self, delta: i64) -> Option<Msg> {
        let outcome = self
            .list
            .delegate_operation(MediaListOperation::Move(delta));
        if outcome.selected_target.is_some() {
            self.cursor_message()
        } else {
            None
        }
    }

    fn activate_selected_row(&mut self) -> Option<Msg> {
        match self
            .list
            .delegate_operation(MediaListOperation::ActivateCurrent)
            .external_intent
        {
            Some(RowIntent::Activate(slot_id)) => Some(Msg::Queue(QueueRequest::Play {
                link: self.link,
                slot_id,
            })),
            _ => None,
        }
    }

    fn delete_selection(&mut self) -> Option<Msg> {
        if !self.list.multi_selection().is_empty() {
            let slot_ids = self.list.multi_selection().to_vec();
            self.list.clear_selection();
            return Some(Msg::Queue(QueueRequest::RemoveSelection {
                link: self.link,
                slot_ids,
            }));
        }
        self.selected_slot()
            .map(|(link, slot_id)| Msg::Queue(QueueRequest::Remove { link, slot_id }))
    }

    fn open_context_menu(&mut self) -> Msg {
        // `.` is a selection-dependent chord owned by the focused surface.
        match self
            .list
            .delegate_operation(MediaListOperation::ContextCurrent)
            .external_intent
        {
            Some(RowIntent::Context(slot_id)) => Msg::Shell(Box::new(
                ShellRequest::RowContextMenu(ContextMenuTargets::Queue(vec![slot_id]), None),
            )),
            Some(RowIntent::ContextSelection(slot_ids)) => Msg::Shell(Box::new(
                ShellRequest::RowContextMenu(ContextMenuTargets::Queue(slot_ids), None),
            )),
            _ => Msg::Shell(Box::new(ShellRequest::RowContextMenu(
                ContextMenuTargets::Queue(vec![]),
                None,
            ))),
        }
    }

    fn navigate_to_selection(&self) -> Option<Msg> {
        self.selected_slot().map(|(link, slot_id)| {
            Msg::Shell(Box::new(ShellRequest::QueueIntent(QueueIntent::Navigate {
                link,
                slot_id,
            })))
        })
    }

    /// The link/slot pair for the Queue's current selection.
    fn selected_slot(&self) -> Option<(QueueLink, QueueSlotId)> {
        self.list
            .selected_target()
            .map(|&slot_id| (self.link, slot_id))
    }

    fn cursor_message(&self) -> Option<Msg> {
        self.list.selected_target().map(|&slot_id| {
            Msg::Queue(QueueRequest::Cursor {
                link: self.link,
                slot_id,
            })
        })
    }
}
