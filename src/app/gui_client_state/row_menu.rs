//! The queue row menu's App-side open and entry activation (the
//! context-menu key and right-click, and the `ContextMenuSelect` request's
//! App-side effect, add-tauri-gui-client row 6.5).

use mbv_queue::QueueSlotId;
use mbv_ui_model::context_menu::{ContextAction, ContextMenuEntry};

use super::overlay::{GuiOverlay, GuiRowMenu};
use crate::app::App;

impl App {
    /// Open the GUI Client's row menu over a queue row. `targets` are the
    /// rows the opening gesture resolved — the queue surface's context-menu
    /// key or right-click, which name one row (the first of a marked
    /// selection). A multi-selection takes the shared selection menu the
    /// TUI's right-click builds (its bulk `Remove` names every marked
    /// slot); a single row takes the canvas's per-row entries: Play now,
    /// Play next, the mark entry the row's own state picks, the go-to
    /// entry, Remove, and Clear queue — the mark and go-to entries only
    /// for a non-music Emby row, per the TUI queue menu's rules.
    pub(in crate::app) fn open_gui_row_menu(&mut self, targets: &[QueueSlotId]) {
        if targets.len() > 1 {
            let Some(menu) = self.build_queue_selection_menu(targets) else {
                return;
            };
            self.gui_overlay = Some(GuiOverlay::RowMenu(GuiRowMenu {
                anchor_slot: targets[0],
                cursor: 0,
                pending_delete: false,
                entries: menu.entries,
            }));
            return;
        }
        let Some(slot) = targets.first().copied() else {
            return;
        };
        let Some(index) = self.displayed_queue().slot_index(slot) else {
            return;
        };
        let item = self.displayed_queue().emby_item_at(index);
        let mut entries = Vec::with_capacity(5);
        entries.push(ContextMenuEntry {
            label: "Play now",
            action: Some(ContextAction::PlayQueue(index)),
            key_hint: Some("Enter"),
        });
        entries.push(ContextMenuEntry {
            label: "Play next",
            action: Some(ContextAction::PlayNext(index)),
            key_hint: Some("n"),
        });
        if let Some(item) = item
            && !item.is_music()
        {
            let (label, action) = if App::context_menu_play_state(item) {
                (
                    "Mark unwatched",
                    ContextAction::MarkUnplayed(item.id.clone()),
                )
            } else {
                ("Mark watched", ContextAction::MarkPlayed(item.id.clone()))
            };
            entries.push(ContextMenuEntry {
                label,
                action: Some(action),
                key_hint: Some("w"),
            });
            entries.push(ContextMenuEntry {
                label: "Go to episode",
                action: Some(ContextAction::GoToLibrary(
                    item.id.clone(),
                    item.item_type.clone(),
                )),
                key_hint: Some("g"),
            });
        }
        entries.push(ContextMenuEntry {
            label: "Remove from queue",
            action: Some(ContextAction::RemoveFromQueue(index)),
            key_hint: Some("dd"),
        });
        // The clear entry names the whole queue, not the row: the same
        // prompt the `c` chord raises (`App::request_clear_queue`). An
        // empty queue has nothing to clear, so the entry does not list.
        if self
            .queue_for_link(self.playing_queue_link())
            .total_queue_len()
            > 0
        {
            entries.push(ContextMenuEntry {
                label: "Clear queue",
                action: Some(ContextAction::ClearQueue),
                key_hint: Some("c"),
            });
        }
        self.gui_overlay = Some(GuiOverlay::RowMenu(GuiRowMenu {
            anchor_slot: slot,
            cursor: 0,
            pending_delete: false,
            entries,
        }));
    }

    /// Activate the open row menu's entry at `index` (the
    /// `ContextMenuSelect` request's App-side effect, add-tauri-gui-client row
    /// 6.5): the same execution the menu's Enter applies
    /// (`execute_gui_overlay_entry`), gated on the open overlay being the row
    /// menu — a stale entry press while another overlay or none holds the
    /// slot executes nothing. The menu's entries carry their resolved queue
    /// targets, so the index executes what the frame painted.
    pub(in crate::app) fn select_gui_row_menu_entry(&mut self, index: usize) {
        if matches!(self.gui_overlay, Some(GuiOverlay::RowMenu(_))) {
            self.execute_gui_overlay_entry(index);
        }
    }
}
