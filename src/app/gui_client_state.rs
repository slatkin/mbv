//! The GUI Client's Client-core presentation state (add-tauri-gui-client row
//! 6.2): the canvas panel's collapse and queue-fold toggles, the queue rows'
//! right column, the queue filter, and the open menu or popover (design
//! D4a's "new core state" rows).
//!
//! The Client core holds the interaction state (the `gui-client` spec), so
//! every value here lives on the `App` — not in the window content — and a
//! window-content reload keeps it. The TUI Client never reads any of it. The
//! GUI-only chords route through the one keyboard policy (`key_policy.rs`'s
//! GUI-only gate); the open overlay's key rules are the GUI's leaf
//! equivalent for the TUI's mounted overlays: while the overlay is open the
//! router resolves the chord first (the overlay's focus fact closes the
//! global bindings), and these rules — not a surface's — consume or drop it.

use mbv_queue::QueueSlotId;
use mbv_ui_model::context_menu::{ContextAction, ContextMenuEntry};
use mbv_ui_model::panel_targets::SessionTargetKey;
use mbv_ui_msg::{Key, KeyEvent, KeyModifiers, TrackChipKind};

use crate::app::App;
use crate::app::queue_playback_projection::{
    PlaybackTargetChoice, QueuePlaybackProjection, TrackChip, TrackChips,
};
use crate::app::queue_summary_projection::QueueRightColumn;
use crate::app::state::panel_targets::resolve_session_target;

/// The canvas panel's collapse and queue-fold facts (design D10): the front
/// end picks the panel layout from its container size plus these two
/// toggles. The TUI Client has no canvas panel, so it never reads them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PanelTogglesProjection {
    /// The `z` toggle: the panel collapsed to the 72 px rail.
    pub collapsed: bool,
    /// The `q` toggle: the expanded panel's queue folded to its one-line
    /// summary.
    pub queue_folded: bool,
}

/// One row of the open row menu as the frame carries it: the label, the key
/// chord that selects the entry directly (the canvas's key hint) when the
/// entry has one, and whether the entry is destructive.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RowMenuEntryProjection {
    pub label: String,
    pub key_hint: Option<String>,
    /// The canvas paints the entry in its destructive tone under a divider
    /// (design DR6). The flag is GUI-only and derives from the entry's
    /// action, so no shared `ContextMenuEntry` change and no TUI paint
    /// changes with it.
    pub destructive: bool,
}

/// One choice of an open track popover, as the frame carries it: the mpv
/// track id a choice sends, its label, whether a subtitle track is forced,
/// and whether it is the track playing now.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrackPopoverChoice {
    pub id: i64,
    pub label: String,
    pub forced: bool,
    pub chosen: bool,
}

/// The GUI Client's open overlay, as the frame carries it (design D4).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GuiOverlayProjection {
    /// The queue row's menu: the anchor row, the menu cursor, and the
    /// entries, each with its key hint.
    RowMenu {
        /// The row the menu opened on (`QueueSlotId`), so the window anchors
        /// the menu on it instead of guessing from hover or cursor.
        anchor_slot: QueueSlotId,
        cursor: usize,
        entries: Vec<RowMenuEntryProjection>,
    },
    /// A track chip's popover: the chip it belongs to, the cursor, and the
    /// chip's choices — the subtitle popover lists Off first (the
    /// `gui-client` spec).
    TrackPopover {
        chip: TrackChipKind,
        cursor: usize,
        choices: Vec<TrackPopoverChoice>,
    },
    /// The playback-target chooser (add-tauri-gui-client row 6.4): the
    /// cursor and the cast/session choices, the same list the playback
    /// section's `target_choices` carries.
    TargetChooser {
        cursor: usize,
        choices: Vec<PlaybackTargetChoice>,
    },
}

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

/// The GUI Client's open overlay (design D4a): the row menu, a track chip's
/// popover, or the playback-target chooser. `App::gui_overlay` holds the
/// open one.
pub(in crate::app) enum GuiOverlay {
    RowMenu(GuiRowMenu),
    TrackPopover(TrackPopoverState),
    TargetChooser(TargetChooserState),
}

/// One key's outcome over the open overlay: consumed with nothing to run,
/// dismissed, or executing the entry at an index.
enum OverlayKeyOutcome {
    Consumed,
    Dismiss,
    Execute(usize),
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

impl App {
    /// The canvas panel's collapse and queue-fold facts (design D10).
    #[must_use]
    pub fn panel_toggles_projection(&self) -> PanelTogglesProjection {
        PanelTogglesProjection {
            collapsed: self.panel_collapsed,
            queue_folded: self.queue_folded,
        }
    }

    /// The open overlay as the frame carries it (design D4), built from the
    /// same playback projection read the frame carries, so the popover's
    /// choices and the track chips cannot disagree inside one frame.
    /// `None` while no overlay is open, and for a popover whose chips the
    /// local player no longer holds.
    #[must_use]
    pub fn gui_overlay_projection(
        &self,
        playback: &QueuePlaybackProjection,
    ) -> Option<GuiOverlayProjection> {
        match &self.gui_overlay {
            Some(GuiOverlay::RowMenu(menu)) => Some(GuiOverlayProjection::RowMenu {
                anchor_slot: menu.anchor_slot,
                cursor: menu.cursor,
                entries: menu
                    .entries
                    .iter()
                    .map(|entry| RowMenuEntryProjection {
                        label: entry.label.to_string(),
                        key_hint: entry.key_hint.map(str::to_string),
                        // From the action, not the label or the position: the
                        // menu's remove entry is always last today, and the
                        // paint must not depend on that.
                        destructive: matches!(
                            entry.action.as_ref(),
                            Some(ContextAction::RemoveFromQueue(_))
                        ),
                    })
                    .collect(),
            }),
            Some(GuiOverlay::TrackPopover(popover)) => {
                let chips = playback.track_chips.as_ref()?;
                Some(GuiOverlayProjection::TrackPopover {
                    chip: popover.chip,
                    cursor: popover.cursor,
                    choices: track_popover_choices(popover.chip, chips),
                })
            }
            // The chooser's choices are the same `target_choices` the
            // playback section carries, so the chooser and the header's
            // target fact cannot disagree inside one frame.
            Some(GuiOverlay::TargetChooser(chooser)) => Some(GuiOverlayProjection::TargetChooser {
                cursor: chooser.cursor,
                choices: playback.target_choices.clone(),
            }),
            None => None,
        }
    }

    /// Open the GUI Client's row menu over a queue row. `targets` are the
    /// rows the opening gesture resolved — the queue surface's context-menu
    /// key or right-click, which name one row (the first of a marked
    /// selection). The menu carries the canvas's entries: Play now, Play
    /// next, the mark entry the row's own state picks, the go-to entry, and
    /// Remove — the mark and go-to entries only for a non-music Emby row,
    /// per the TUI queue menu's rules.
    pub(in crate::app) fn open_gui_row_menu(&mut self, targets: &[QueueSlotId]) {
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
        self.gui_overlay = Some(GuiOverlay::RowMenu(GuiRowMenu {
            anchor_slot: slot,
            cursor: 0,
            pending_delete: false,
            entries,
        }));
    }

    /// Toggle a track chip's popover (the `OpenTrackPopover` request's
    /// App-side effect): the chip's press opens its popover, and the same
    /// press on the open popover's chip closes it, as the target chooser's
    /// press toggles (add-tauri-gui-client row 6.4). The popover opens only
    /// over real choices: the chips' source is the local player's stream,
    /// so an inactive player or a remote output has nothing to list. The
    /// cursor starts on the chosen choice.
    pub(in crate::app) fn toggle_gui_track_popover(&mut self, chip: TrackChipKind) {
        if matches!(&self.gui_overlay, Some(GuiOverlay::TrackPopover(popover)) if popover.chip == chip)
        {
            self.gui_overlay = None;
            return;
        }
        let Some(chips) = self.track_chips() else {
            return;
        };
        let choices = track_popover_choices(chip, &chips);
        let cursor = choices.iter().position(|choice| choice.chosen).unwrap_or(0);
        self.gui_overlay = Some(GuiOverlay::TrackPopover(TrackPopoverState { chip, cursor }));
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

    /// Activate the open track popover's painted choice by its track id (the
    /// `TrackPopoverSelect` request's App-side effect, add-tauri-gui-client
    /// rows 6.4 and 6.5). The press names the mpv id the frame painted, so a
    /// track-list change between the paint and the press selects the painted
    /// track or nothing, never the neighbour; the id must still be one of
    /// the chip's live choices. Gated on the open overlay being a track
    /// popover — a stale row press while another overlay or none holds the
    /// slot executes nothing.
    pub(in crate::app) fn select_gui_track_popover_track(&mut self, track_id: i64) {
        let Some(GuiOverlay::TrackPopover(popover)) = &self.gui_overlay else {
            return;
        };
        let chip = popover.chip;
        let Some(choices) = self.track_popover_choices(chip) else {
            return;
        };
        if !choices.iter().any(|choice| choice.id == track_id) {
            return;
        }
        self.apply_track_choice(chip, track_id);
        self.gui_overlay = None;
    }

    /// Switch the chip's track to `track_id`: the one switch a track popover's
    /// Enter and a painted row's press both run.
    fn apply_track_choice(&mut self, chip: TrackChipKind, track_id: i64) {
        match chip {
            TrackChipKind::Audio => self.set_audio_track(track_id),
            TrackChipKind::Subtitles => self.set_subtitle_track(track_id),
        }
    }

    /// Whether an overlay holds the GUI Client's keyboard (the row menu, a
    /// track popover, or the playback-target chooser). The GUI Client's core
    /// reads it to dismiss the overlay on an outside press (row 6.5).
    #[must_use]
    pub fn gui_overlay_is_open(&self) -> bool {
        self.gui_overlay.is_some()
    }

    /// Dismiss the open overlay without executing an entry: the outside
    /// press the canvas and the TUI's blocking context menu both take
    /// (add-tauri-gui-client row 6.5).
    pub fn dismiss_open_gui_overlay(&mut self) {
        self.gui_overlay = None;
    }

    /// Toggle the playback-target chooser (add-tauri-gui-client row 6.4):
    /// the canvas header's target press and the GUI-only `r` chord both
    /// open it, the same gesture closes it — the toggle the TUI's F3
    /// Sessions sidebar applies (`request_sidebar_toggle`). The chooser
    /// opens even with no remote targets to list, as the sidebar does. The
    /// cursor starts on the Owner's attached output, as the popover's
    /// starts on the chosen track.
    pub(in crate::app) fn toggle_gui_target_chooser(&mut self) {
        self.gui_overlay = if let Some(GuiOverlay::TargetChooser(_)) = &self.gui_overlay {
            None
        } else {
            let cursor = self
                .playback_target_choices()
                .iter()
                .position(|choice| choice.is_active)
                .unwrap_or(0);
            Some(GuiOverlay::TargetChooser(TargetChooserState { cursor }))
        };
    }

    /// Drop a track popover whose chip source is gone: the popover's
    /// choices are the local player's stream, so once the player goes
    /// inactive the frame projects no overlay for it
    /// (`gui_overlay_projection`'s `None`) and the held state must match —
    /// otherwise the snapshot still reports the overlay focus and the
    /// overlay's key rules keep consuming input the window does not paint.
    /// The GUI input entry reconciles before it builds the snapshot.
    pub(in crate::app) fn close_orphaned_track_popover(&mut self) {
        if matches!(self.gui_overlay, Some(GuiOverlay::TrackPopover(_)))
            && self.track_chips().is_none()
        {
            self.gui_overlay = None;
        }
    }

    /// The open overlay's key rules: the overlay owns the keyboard while it
    /// is open — a key it binds runs its action, any other key is dropped,
    /// and no surface sees a chord. Returns whether an overlay was open.
    pub(in crate::app) fn apply_gui_overlay_key(&mut self, key: KeyEvent) -> bool {
        // The popover's cursor clamps against its live choice list, read
        // before the overlay borrow so the two reads cannot interleave.
        let popover_chip = match &self.gui_overlay {
            Some(GuiOverlay::TrackPopover(popover)) => Some(popover.chip),
            _ => None,
        };
        let popover_choices = popover_chip
            .and_then(|chip| self.track_popover_choices(chip))
            .map_or(0, |choices| choices.len());
        // The chooser's cursor clamps against the same live list the frame
        // projects, read before the overlay borrow as the popover's is.
        let chooser_choices = if matches!(self.gui_overlay, Some(GuiOverlay::TargetChooser(_))) {
            self.playback_target_choices().len()
        } else {
            0
        };
        let outcome = match &mut self.gui_overlay {
            Some(GuiOverlay::RowMenu(menu)) => menu.apply_key(key),
            Some(GuiOverlay::TrackPopover(popover)) => {
                apply_overlay_list_key(&mut popover.cursor, key, popover_choices)
            }
            Some(GuiOverlay::TargetChooser(chooser)) => {
                apply_overlay_list_key(&mut chooser.cursor, key, chooser_choices)
            }
            None => return false,
        };
        match outcome {
            OverlayKeyOutcome::Consumed => {}
            OverlayKeyOutcome::Dismiss => self.gui_overlay = None,
            OverlayKeyOutcome::Execute(index) => self.execute_gui_overlay_entry(index),
        }
        true
    }

    /// Execute the open overlay's entry at `index`, then close the overlay —
    /// the TUI menu's Select flow (dismiss, then dispatch), with the row
    /// menu's action carrying its own resolved queue index (D2).
    fn execute_gui_overlay_entry(&mut self, index: usize) {
        let Some(overlay) = self.gui_overlay.take() else {
            return;
        };
        match overlay {
            GuiOverlay::RowMenu(menu) => {
                let Some(entry) = menu.entries.into_iter().nth(index) else {
                    return;
                };
                let Some(action) = entry.action else {
                    return;
                };
                // The GUI row menu carries no bulk action (the canvas's menu
                // is a per-row menu), so no selection cleanup joins the
                // dispatch.
                self.execute_context_action(Some(action), None);
            }
            GuiOverlay::TrackPopover(popover) => {
                let Some(chips) = self.track_chips() else {
                    return;
                };
                let choices = track_popover_choices(popover.chip, &chips);
                let Some(choice) = choices.get(index) else {
                    return;
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
                    return;
                };
                self.select_target_for_key(&choice.key);
            }
        }
    }

    /// Select the playback target a stable `SessionTargetKey` names,
    /// resolved against the current target snapshot: the dispatch a
    /// chooser's Enter and a chooser row's press share. A target the
    /// snapshot no longer holds selects nothing. Returns whether one did.
    pub(in crate::app) fn select_target_for_key(&mut self, key: &SessionTargetKey) -> bool {
        match resolve_session_target(&self.panel_targets, key) {
            Some(target) => {
                self.select_panel_target(target);
                true
            }
            None => false,
        }
    }

    /// The track popover's choice list for `chip`, read live: the audio chip
    /// lists the player's audio tracks; the subtitle chip lists Off and then
    /// the subtitle tracks (the `gui-client` spec). `None` while the local
    /// player is inactive — the chips' source is the local player's stream.
    pub(in crate::app) fn track_popover_choices(
        &self,
        chip: TrackChipKind,
    ) -> Option<Vec<TrackPopoverChoice>> {
        let chips = self.track_chips()?;
        Some(track_popover_choices(chip, &chips))
    }

    /// The queue filter's key rules while its text entry holds the keyboard:
    /// unmodified and Shift-folded characters append (the DOM's real modifier
    /// keys never type through Ctrl or Alt), Backspace pops — an empty-text
    /// Backspace closes — Esc closes and clears, and Enter closes keeping
    /// the text. Every other chord falls through to the queue surface, so
    /// navigation keeps working while the filter is open. Returns whether
    /// the key was consumed.
    pub(in crate::app) fn apply_gui_filter_key(&mut self, key: KeyEvent) -> bool {
        if !self.queue_filter_active {
            return false;
        }
        match key.code {
            Key::Esc if key.modifiers.is_empty() => {
                self.queue_filter_active = false;
                self.queue_filter.clear();
                true
            }
            Key::Enter if key.modifiers.is_empty() => {
                self.queue_filter_active = false;
                true
            }
            Key::Backspace if key.modifiers.is_empty() => {
                if self.queue_filter.pop().is_none() {
                    self.queue_filter_active = false;
                    self.queue_filter.clear();
                }
                true
            }
            Key::Char(c)
                if !key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT) =>
            {
                self.queue_filter.push(c);
                true
            }
            _ => false,
        }
    }

    /// The `z` toggle (design D10): collapse the panel to the rail.
    pub(in crate::app) fn toggle_panel_collapse(&mut self) {
        self.panel_collapsed = !self.panel_collapsed;
    }

    /// The `q` toggle (design D10): fold the expanded panel's queue.
    pub(in crate::app) fn toggle_queue_fold(&mut self) {
        self.queue_folded = !self.queue_folded;
    }

    /// The queue header's Length / Ends-at switch.
    pub(in crate::app) fn toggle_queue_right_column(&mut self) {
        self.queue_right_column = match self.queue_right_column {
            QueueRightColumn::Length => QueueRightColumn::EndsAt,
            QueueRightColumn::EndsAt => QueueRightColumn::Length,
        };
    }

    /// Focus the queue header's filter text entry (the GUI-only `/`).
    pub(in crate::app) fn activate_queue_filter(&mut self) {
        self.queue_filter_active = true;
    }
}

/// The track popover's choice list for `chip` from the chips' projection
/// data: the audio chip lists the audio tracks; the subtitle chip lists Off
/// and then the subtitle tracks (the `gui-client` spec). One construction
/// site serves both the frame projection and the choice execution.
#[must_use]
fn track_popover_choices(chip: TrackChipKind, chips: &TrackChips) -> Vec<TrackPopoverChoice> {
    match chip {
        TrackChipKind::Audio => popover_choices(&chips.audio, false),
        TrackChipKind::Subtitles => {
            let mut choices = vec![TrackPopoverChoice {
                id: 0,
                label: "Off".to_string(),
                forced: false,
                chosen: chips.subtitles.chosen.is_none(),
            }];
            choices.extend(popover_choices(&chips.subtitles, true));
            choices
        }
    }
}

/// The chip's own choices as popover choices, the `forced` flag passed
/// through and the chosen mark resolved against the chip's chosen id.
fn popover_choices(chip: &TrackChip, forced: bool) -> Vec<TrackPopoverChoice> {
    chip.choices
        .iter()
        .map(|choice| TrackPopoverChoice {
            id: choice.id,
            label: choice.label.clone(),
            forced,
            chosen: chip.chosen == Some(choice.id),
        })
        .collect()
}
