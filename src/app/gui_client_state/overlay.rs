//! The GUI Client's held overlay state (design D4a) and its key and
//! execution rules: which overlay is open, how its keys consume or drop
//! chords, and how a chosen entry executes.

use mbv_queue::{NamedQueueId, QueueSlotId};
use mbv_ui_model::confirm::ConfirmModal;
use mbv_ui_model::context_menu::ContextMenuEntry;
use mbv_ui_model::daemon_lost::DaemonLostModal;
use mbv_ui_msg::{Key, KeyEvent, KeyModifiers, TrackChipKind};

use super::confirm::confirm_button_key;
use super::daemon_lost::daemon_lost_entry_index;
use super::popover::track_popover_choices;
use super::projection::QueueChooserEntryProjection;
use crate::app::App;

/// The open queue row's menu, held by the Client core while it is up. The
/// entries carry the resolved queue index (split-queue-cursor-ownership D2),
/// so a follow update to the queue cursor cannot redirect an execution, and
/// each entry's key hint.
pub(in crate::app) struct GuiRowMenu {
    /// The row the menu opened on: the pointer's actions button, or the row
    /// the context-menu chord resolved.
    pub(in crate::app) anchor_slot: QueueSlotId,
    pub(in crate::app) cursor: usize,
    /// The first stroke of the canvas's two-stroke `dd` remove hint: the
    /// second stroke within the menu selects the remove entry.
    pub(in crate::app) pending_delete: bool,
    pub(in crate::app) entries: Vec<ContextMenuEntry>,
}

/// The open track chip's popover: which chip and the cursor over its
/// choices. The choice list is the chip's live projection — the state
/// carries no copy, so a track-list change is visible on the next frame and
/// the cursor clamps at use time.
pub(in crate::app) struct TrackPopoverState {
    pub(in crate::app) chip: TrackChipKind,
    pub(in crate::app) cursor: usize,
}

/// The open playback-target chooser (add-tauri-gui-client row 6.4): the
/// cursor over its choices. The choice list is the projection's live
/// `target_choices` — the state carries no copy, so a target arriving or
/// leaving is visible on the next frame and the cursor clamps at use time.
pub(in crate::app) struct TargetChooserState {
    pub(in crate::app) cursor: usize,
}

/// The open Queue chooser (gui-named-queues design D8): the cursor over its
/// entries, and the name open for edit while one is. The entry list is the
/// live projection — the local Owner's Named queues in queue order, then the
/// connected peer — so the state carries no copy, and the cursor clamps
/// against the list it reads.
pub(in crate::app) struct QueueChooserState {
    pub(in crate::app) cursor: usize,
    /// The rename draft the chooser holds while a name is open for edit.
    pub(in crate::app) renaming: Option<QueueRenameDraft>,
}

/// The player panel's context menu (`gui-player-overlay-menu`): the cursor
/// over its one auto-hide entry and the pointer anchor the right-click
/// carried, in window CSS pixels. The menu's single entry toggles the
/// core-owned auto-hide preference, so the state carries no entry list —
/// the frame's playback section carries the checked fact the entry paints.
pub(in crate::app) struct GuiPlayerMenu {
    pub(in crate::app) cursor: usize,
    /// The right-click's window anchor, in CSS pixels.
    pub(in crate::app) x: f32,
    pub(in crate::app) y: f32,
}

/// One open rename draft: the local Named queue it renames and the text the
/// user typed. The queue is held, not the cursor's row, so a list change
/// while the name is open cannot redirect the rename. A draft opens with its
/// whole text selected, so the first typed character replaces the name and
/// Enter keeps it.
pub(in crate::app) struct QueueRenameDraft {
    pub(in crate::app) queue: NamedQueueId,
    pub(in crate::app) text: String,
    /// Whether the whole text is selected: the next character or Backspace
    /// replaces or clears it.
    pub(in crate::app) selected: bool,
}

impl QueueRenameDraft {
    /// A draft over `text` with all of it selected.
    pub(in crate::app) fn selecting(queue: NamedQueueId, text: String) -> Self {
        Self {
            queue,
            text,
            selected: true,
        }
    }
}

/// The GUI Client's open overlay (design D4a): the row menu, a track chip's
/// popover, the playback-target chooser, the Queue chooser, the shared
/// confirmation modal, or the daemon-lost recovery dialog. `App::overlay`
/// holds the open one.
pub(in crate::app) enum GuiOverlay {
    RowMenu(GuiRowMenu),
    /// The player panel's context menu (the panel's right-click): the
    /// auto-hide entry the panel's bottom overlay paints as a checkbox
    /// no longer.
    PlayerMenu(GuiPlayerMenu),
    TrackPopover(TrackPopoverState),
    TargetChooser(TargetChooserState),
    /// The Queue chooser (gui-named-queues design D8): the dropdown that
    /// picks which queue the Queue panel shows.
    QueueChooser(QueueChooserState),
    /// The shared confirmation modal (`App::ask_confirm` raised it): the
    /// same `ConfirmModal` the TUI's mounted Confirm component holds. The
    /// overlay owns the keyboard while it is up, and its buttons report
    /// press regions the core resolves.
    Confirm(ConfirmModal),
    /// The daemon-lost recovery dialog (`App::raise_daemon_lost_modal`
    /// raised it, `daemon-disconnect-handling`: "An unannounced connection
    /// loss raises a recovery dialog"): the same `DaemonLostModal` the
    /// TUI's mounted `DaemonLost` component holds. The dialog blocks every
    /// other input until one of its choices runs, and its buttons report
    /// press regions the core resolves.
    DaemonLost(DaemonLostModal),
}

/// One key's outcome over the open overlay: consumed with nothing to run,
/// dismissed, executing the entry at an index, or confirming the rename the
/// Queue chooser held.
enum OverlayKeyOutcome {
    Consumed,
    Dismiss,
    Execute(usize),
    /// The Queue chooser's confirmed name: the queue it renames and the text
    /// the user typed, sent as one `NamedQueueRename`.
    RenameQueue {
        queue: NamedQueueId,
        name: String,
    },
}

impl GuiRowMenu {
    /// The row menu's key rules: unmodified Up/Down move the wrapping cursor,
    /// Enter selects the cursor entry, a hint chord selects its entry
    /// directly, the two-stroke `dd` selects the remove entry, and every
    /// other chord is dropped — the overlay owns the keyboard while it is
    /// open, exactly as the TUI's mounted blocking overlay does. Returns the
    /// entry index to execute.
    fn apply_key(&mut self, key: KeyEvent) -> OverlayKeyOutcome {
        match key.code {
            Key::Up if key.modifiers.is_empty() => {
                self.move_cursor(false);
                OverlayKeyOutcome::Consumed
            }
            Key::Down if key.modifiers.is_empty() => {
                self.move_cursor(true);
                OverlayKeyOutcome::Consumed
            }
            Key::Enter if key.modifiers.is_empty() => OverlayKeyOutcome::Execute(self.cursor),
            Key::Esc if key.modifiers.is_empty() => OverlayKeyOutcome::Dismiss,
            Key::Char('d') if key.modifiers.is_empty() => {
                if self.pending_delete {
                    self.pending_delete = false;
                    self.direct_select("dd")
                } else {
                    self.pending_delete = true;
                    OverlayKeyOutcome::Consumed
                }
            }
            Key::Char(c) if key.modifiers.is_empty() => {
                self.pending_delete = false;
                self.direct_select(&c.to_string())
            }
            _ => {
                self.pending_delete = false;
                OverlayKeyOutcome::Consumed
            }
        }
    }

    /// The entry whose key hint names this chord, if one does: the direct
    /// select of the canvas's hinted menu.
    fn direct_select(&mut self, chord: &str) -> OverlayKeyOutcome {
        match self
            .entries
            .iter()
            .position(|entry| entry.key_hint == Some(chord))
        {
            Some(index) => OverlayKeyOutcome::Execute(index),
            None => OverlayKeyOutcome::Consumed,
        }
    }

    /// Move the wrapping cursor one entry, as the TUI menu's cursor moves.
    fn move_cursor(&mut self, down: bool) {
        move_overlay_cursor(&mut self.cursor, down, self.entries.len());
    }
}

/// One chooser-style overlay list's key rules: unmodified Up/Down move the
/// wrapping cursor over `choices`, Enter chooses the cursor choice, Esc
/// dismisses, and every other chord is dropped — the overlay owns the
/// keyboard while it is open, exactly as the TUI's mounted blocking overlay
/// does. Returns the choice index to execute.
fn apply_overlay_list_key(cursor: &mut usize, key: KeyEvent, choices: usize) -> OverlayKeyOutcome {
    match key.code {
        Key::Up if key.modifiers.is_empty() => {
            move_overlay_cursor(cursor, false, choices);
            OverlayKeyOutcome::Consumed
        }
        Key::Down if key.modifiers.is_empty() => {
            move_overlay_cursor(cursor, true, choices);
            OverlayKeyOutcome::Consumed
        }
        Key::Enter if key.modifiers.is_empty() => OverlayKeyOutcome::Execute(*cursor),
        Key::Esc if key.modifiers.is_empty() => OverlayKeyOutcome::Dismiss,
        _ => OverlayKeyOutcome::Consumed,
    }
}

/// Move the wrapping cursor one choice within a list of `choices`.
fn move_overlay_cursor(cursor: &mut usize, down: bool, choices: usize) {
    if choices == 0 {
        return;
    }
    let last = choices - 1;
    *cursor = if down {
        if *cursor >= last { 0 } else { *cursor + 1 }
    } else if *cursor == 0 {
        last
    } else {
        *cursor - 1
    };
}

impl QueueChooserState {
    /// The Queue chooser's key rules (design D8, spec `gui-queue-chooser`
    /// "The open chooser takes its keys"): unmodified Up/Down move the
    /// wrapping cursor over `entries`, Enter shows the entry under the
    /// cursor, `r` opens the name of the local queue under the cursor for
    /// edit, `t` and Esc close the chooser, and every other chord is dropped —
    /// the overlay owns the keyboard while it is open. While a name is open
    /// for edit, Enter confirms it, Esc cancels the edit and keeps the
    /// chooser open, and the typed text appends and pops as the Queue
    /// filter's draft does.
    fn apply_key(
        &mut self,
        key: KeyEvent,
        entries: &[QueueChooserEntryProjection],
    ) -> OverlayKeyOutcome {
        if let Some(draft) = &mut self.renaming {
            return match apply_rename_key(draft, key) {
                RenameKeyOutcome::Keep => OverlayKeyOutcome::Consumed,
                RenameKeyOutcome::Cancel => {
                    self.renaming = None;
                    OverlayKeyOutcome::Consumed
                }
                RenameKeyOutcome::Confirm { queue, name } => {
                    OverlayKeyOutcome::RenameQueue { queue, name }
                }
            };
        }
        // The list is live, so a queue that closed since the last frame can
        // leave the cursor past its end: it clamps before it is read.
        self.cursor = self.cursor.min(entries.len().saturating_sub(1));
        match key.code {
            Key::Char('r') if key.modifiers.is_empty() => {
                self.open_rename_draft(entries);
                OverlayKeyOutcome::Consumed
            }
            // The spec binds `t` to open and close the chooser, and the
            // toggle it opened with is that same chord.
            Key::Char('t') if key.modifiers.is_empty() => OverlayKeyOutcome::Dismiss,
            _ => apply_overlay_list_key(&mut self.cursor, key, entries.len()),
        }
    }

    /// Open the queue under the cursor for edit, keeping its current name as
    /// the draft's text, so Enter accepts it and Backspace trims it. A peer
    /// entry is not a queue of any owner with a name to change, so it opens
    /// no draft.
    fn open_rename_draft(&mut self, entries: &[QueueChooserEntryProjection]) {
        let Some(entry) = entries.get(self.cursor) else {
            return;
        };
        let Some(queue) = entry.queue else {
            return;
        };
        self.renaming = Some(QueueRenameDraft::selecting(queue, entry.label.clone()));
    }
}

/// One rename draft's key outcome: keep editing, cancel the edit, or confirm
/// the name.
enum RenameKeyOutcome {
    Keep,
    Cancel,
    Confirm { queue: NamedQueueId, name: String },
}

/// One rename draft's key rules: Enter confirms a name that holds more than
/// whitespace, Esc cancels the edit, Backspace pops, and unmodified and
/// Shift-folded characters append — the Queue filter's draft pattern. While
/// the whole text is selected (a draft opens so), the first character
/// replaces it and Backspace clears it. Every other chord is consumed and
/// dropped, and keeps the selection.
fn apply_rename_key(draft: &mut QueueRenameDraft, key: KeyEvent) -> RenameKeyOutcome {
    match key.code {
        Key::Enter if key.modifiers.is_empty() => {
            if draft.text.trim().is_empty() {
                return RenameKeyOutcome::Keep;
            }
            RenameKeyOutcome::Confirm {
                queue: draft.queue,
                name: draft.text.clone(),
            }
        }
        Key::Esc if key.modifiers.is_empty() => RenameKeyOutcome::Cancel,
        Key::Backspace if key.modifiers.is_empty() => {
            if draft.selected {
                draft.text.clear();
                draft.selected = false;
            } else {
                draft.text.pop();
            }
            RenameKeyOutcome::Keep
        }
        Key::Char(c)
            if !key.modifiers.contains(KeyModifiers::CONTROL)
                && !key.modifiers.contains(KeyModifiers::ALT) =>
        {
            if draft.selected {
                draft.text.clear();
                draft.selected = false;
            }
            draft.text.push(c);
            RenameKeyOutcome::Keep
        }
        _ => RenameKeyOutcome::Keep,
    }
}

impl App {
    /// Whether an overlay holds the GUI Client's keyboard (the row menu, a
    /// track popover, the playback-target chooser, or the Queue chooser). The
    /// GUI Client's core reads it to dismiss the overlay on an outside press.
    #[must_use]
    pub fn gui_overlay_is_open(&self) -> bool {
        self.gui.overlay_is_open()
    }

    /// The open overlay's key rules: the overlay owns the keyboard while it
    /// is open — a key it binds runs its action, any other key is dropped,
    /// and no surface sees a chord. Returns whether the Client's run should
    /// end, which only the daemon-lost dialog's quit choice asks for.
    pub(in crate::app) fn apply_gui_overlay_key(&mut self, key: KeyEvent) -> bool {
        // The confirmation modal answers before the overlay borrow, because
        // its answer replaces the overlay it reads.
        if matches!(self.gui.overlay, Some(GuiOverlay::Confirm(_))) {
            return self.apply_gui_confirm_key(key);
        }
        // The popover's cursor clamps against its live choice list, read
        // before the overlay borrow so the two reads cannot interleave.
        let popover_chip = match &self.gui.overlay {
            Some(GuiOverlay::TrackPopover(popover)) => Some(popover.chip),
            _ => None,
        };
        let popover_choices = popover_chip
            .and_then(|chip| self.track_popover_choices(chip))
            .map_or(0, |choices| choices.len());
        // The chooser's cursor clamps against the same live list the frame
        // projects, read before the overlay borrow as the popover's is.
        let chooser_choices = if matches!(self.gui.overlay, Some(GuiOverlay::TargetChooser(_))) {
            self.playback_target_choices().len()
        } else {
            0
        };
        // The Queue chooser's cursor clamps against its live entry list — the
        // same list the frame projects — read before the overlay borrow too.
        let queue_choices = if matches!(self.gui.overlay, Some(GuiOverlay::QueueChooser(_))) {
            self.queue_chooser_entries()
        } else {
            Vec::new()
        };
        // The daemon-lost dialog's chords name its painted choices; read
        // before the overlay borrow, as the two cursors above are.
        let daemon_lost_entry = match &self.gui.overlay {
            Some(GuiOverlay::DaemonLost(_)) => daemon_lost_entry_index(key.code),
            _ => None,
        };
        let outcome = match &mut self.gui.overlay {
            Some(GuiOverlay::RowMenu(menu)) => menu.apply_key(key),
            Some(GuiOverlay::PlayerMenu(menu)) => apply_overlay_list_key(&mut menu.cursor, key, 1),
            Some(GuiOverlay::TrackPopover(popover)) => {
                apply_overlay_list_key(&mut popover.cursor, key, popover_choices)
            }
            Some(GuiOverlay::TargetChooser(chooser)) => {
                apply_overlay_list_key(&mut chooser.cursor, key, chooser_choices)
            }
            Some(GuiOverlay::QueueChooser(chooser)) => chooser.apply_key(key, &queue_choices),
            // The confirmation modal answered above, before the overlay
            // borrow; this arm keeps the match exhaustive.
            Some(GuiOverlay::Confirm(_)) => OverlayKeyOutcome::Consumed,
            // The daemon-lost dialog binds its three recovery choices and
            // nothing else: the TUI's component binds no default answer, and
            // the dialog blocks every other key until a choice runs
            // (`daemon-disconnect-handling`: "SHALL NOT act on other key
            // presses while the dialog is open"). Esc included — there is no
            // dismissing it back to a dead Owner.
            Some(GuiOverlay::DaemonLost(_)) => match daemon_lost_entry {
                Some(index) => OverlayKeyOutcome::Execute(index),
                None => OverlayKeyOutcome::Consumed,
            },
            None => return false,
        };
        match outcome {
            OverlayKeyOutcome::Consumed => false,
            OverlayKeyOutcome::Dismiss => {
                self.gui.overlay = None;
                false
            }
            OverlayKeyOutcome::Execute(index) => self.execute_gui_overlay_entry(index),
            OverlayKeyOutcome::RenameQueue { queue, name } => {
                self.rename_named_queue(queue, &name);
                false
            }
        }
    }

    /// The shared confirmation modal's key rules: Enter accepts, Esc
    /// cancels, and the three-way dirty-playlist prompt's `d` discards — the
    /// TUI's Confirm component (`confirm_intent_for_key`) rule for rule.
    /// Every other chord is consumed and the modal stays up.
    fn apply_gui_confirm_key(&mut self, key: KeyEvent) -> bool {
        let Some(GuiOverlay::Confirm(modal)) = &self.gui.overlay else {
            return false;
        };
        let action = modal.on_confirm.clone();
        let answering = match key.code {
            Key::Enter => Some(KeyEvent::new(Key::Enter, KeyModifiers::NONE)),
            Key::Esc => Some(KeyEvent::new(Key::Esc, KeyModifiers::NONE)),
            Key::Char('d')
                if key.modifiers.is_empty()
                    && matches!(
                        action,
                        mbv_ui_model::confirm::ConfirmAction::DiscardOrSaveDirtyPlaylist
                    ) =>
            {
                Some(KeyEvent::new(Key::Char('d'), KeyModifiers::NONE))
            }
            _ => None,
        };
        if let Some(key) = answering {
            self.gui.overlay = None;
            self.apply_confirm_action(action, key);
        }
        false
    }

    /// Execute the open overlay's entry at `index`, then close the overlay —
    /// the TUI menu's Select flow (dismiss, then dispatch), with the row
    /// menu's action carrying its own resolved queue index (D2). Returns
    /// whether the Client's run should end, which only the daemon-lost
    /// dialog's quit choice asks for.
    pub(super) fn execute_gui_overlay_entry(&mut self, index: usize) -> bool {
        // The daemon-lost dialog answers through the shared `App` action and
        // keeps its own held state while the answer runs: a failed restart
        // writes its diagnostic into that same state, so the overlay is not
        // taken here.
        if matches!(self.gui.overlay, Some(GuiOverlay::DaemonLost(_))) {
            return self.apply_gui_daemon_lost_choice(index);
        }
        let Some(overlay) = self.gui.overlay.take() else {
            return false;
        };
        match overlay {
            GuiOverlay::RowMenu(menu) => {
                let Some(entry) = menu.entries.into_iter().nth(index) else {
                    return false;
                };
                let Some(action) = entry.action else {
                    return false;
                };
                // The GUI row menu carries no bulk action (the canvas's menu
                // is a per-row menu), so no selection cleanup joins the
                // dispatch.
                self.execute_context_action(Some(action), None);
            }
            // The player menu's single entry toggles the auto-hide
            // preference, the same effect the entry's press applies through
            // its select request.
            GuiOverlay::PlayerMenu(_) => self.toggle_gui_player_auto_hide(),
            GuiOverlay::TrackPopover(popover) => {
                let Some(chips) = self.track_chips() else {
                    return false;
                };
                let choices = track_popover_choices(popover.chip, &chips);
                let Some(choice) = choices.get(index) else {
                    return false;
                };
                self.apply_track_choice(popover.chip, choice.id);
            }
            // A chooser Enter selects the cursor choice's output: the same
            // dispatch the TUI's Sessions sidebar applies on its selected
            // row — resolve the choice's stable key against the current
            // target snapshot, and do nothing when the target is gone
            // (design D6's shell-side resolution, applied Client-side).
            GuiOverlay::TargetChooser(_) => {
                let Some(choice) = self.playback_target_choices().get(index).cloned() else {
                    return false;
                };
                self.select_chooser_target(&choice.key);
            }
            // A Queue chooser Enter shows the entry under the cursor: the same
            // show its press runs, and no playback starts, stops, or moves
            // (spec `gui-queue-chooser` "The chooser lists local queues and
            // connected peers").
            GuiOverlay::QueueChooser(_) => {
                let Some(entry) = self.queue_chooser_entries().into_iter().nth(index) else {
                    return false;
                };
                match entry.queue {
                    Some(queue) => self.show_named_queue(queue),
                    None => self.show_peer_entry(),
                }
            }
            // A confirm button's press runs the same execution the button's
            // key hint applies: resolve the hint to its chord and answer
            // through the shared confirm dispatcher, then close the modal.
            GuiOverlay::Confirm(modal) => {
                let Some(button) = modal.buttons.get(index) else {
                    return false;
                };
                let Some(key) = confirm_button_key(&button.keys) else {
                    return false;
                };
                let action = modal.on_confirm.clone();
                self.gui.overlay = None;
                self.apply_confirm_action(action, KeyEvent::new(key, KeyModifiers::NONE));
            }
            GuiOverlay::DaemonLost(_) => return false,
        }
        false
    }
}
