//! The GUI Client's frame projection types (design D4, D10): what the
//! bridge's frame carries for the canvas panel's toggles and the open
//! overlay, plus the `App` method that builds the overlay's projection.

use mbv_queue::{NamedQueueId, QueueSlotId};
use mbv_ui_model::context_menu::ContextAction;
use mbv_ui_msg::TrackChipKind;

use super::daemon_lost::daemon_lost_buttons;
use super::overlay::GuiOverlay;
use super::popover::track_popover_choices;
use crate::app::App;
use crate::app::queue_playback_projection::{PlaybackTargetChoice, QueuePlaybackProjection};
use crate::app::state::playback::ShownEntry;

/// The Queue header's chooser facts for one read (spec `gui-queue-chooser`
/// "The queue header holds the chooser and its controls"): the number of
/// entries the Queue chooser lists, the name the chooser button shows, the
/// shown entry's slot count, and whether the close control is disabled. The
/// close control acts on the only local queue only through a disabled
/// control, so it comes from the same entry list the dropdown paints.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct QueueHeaderProjection {
    /// Every entry the Queue chooser lists: the local Owner's Named queues in
    /// queue order, then the connected peer Player owner when one is.
    pub entries: usize,
    /// The name of the shown entry: the chooser button's label. Empty while
    /// the Owner's first queue list has not arrived, when no entry shows.
    pub shown_name: String,
    /// The shown entry's slot count: the switcher's count chip (design D7 of
    /// `gui-queue-panel-redesign`). It counts every slot of the shown queue,
    /// because the Queue filter never narrows the queue itself.
    pub shown_len: usize,
    /// Whether the close control is disabled: the only local queue has no
    /// close, and no entry has arrived to close yet. A shown peer entry is
    /// always closable, because closing it disconnects.
    pub close_disabled: bool,
}

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

/// The GUI Client's open overlay, as the frame carries it (design D4). The
/// player menu's anchor carries CSS pixels, so the equality is `PartialEq`
/// only.
#[derive(Clone, Debug, PartialEq)]
pub enum GuiOverlayProjection {
    /// The player panel's context menu (gui-player-overlay-menu): the cursor
    /// over its one auto-hide entry and the right-click's window anchor. The
    /// entry's checked fact rides the playback section's `controls`.
    PlayerMenu {
        cursor: usize,
        /// The right-click's window anchor, in CSS pixels.
        x: f32,
        y: f32,
    },
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
    /// The Queue chooser (gui-named-queues design D8): the cursor, the local
    /// queues in queue order followed by the connected peer, and the rename
    /// draft while one is open.
    QueueChooser {
        cursor: usize,
        entries: Vec<QueueChooserEntryProjection>,
        renaming: Option<QueueChooserRenameProjection>,
    },
    /// The shared confirmation modal the App raised (`ask_confirm`): the
    /// message and the button row, as the frame carries them. The action
    /// stays in the core's held state (`GuiOverlay::Confirm`).
    Confirm {
        message: String,
        buttons: Vec<GuiConfirmButtonProjection>,
    },
    /// The daemon-lost recovery dialog the App raised
    /// (`raise_daemon_lost_modal`): the diagnostics the TUI's modal paints
    /// and the recovery choices, as the frame carries them. The dialog
    /// stays in the core's held state (`GuiOverlay::DaemonLost`).
    DaemonLost {
        /// The label of whatever was playing right before the Owner
        /// vanished, when one was known.
        last_playing_title: Option<String>,
        /// Where the local daemon's own log lives, so the window can name it
        /// without the user knowing the path by heart.
        daemon_log_path: String,
        /// The last restart attempt's failure, when one failed: the dialog's
        /// `Restart failed:` line.
        restart_error: Option<String>,
        buttons: Vec<GuiConfirmButtonProjection>,
    },
}

/// One entry of the open Queue chooser (spec `gui-queue-chooser` "The chooser
/// lists local queues and connected peers"): the label the dropdown paints,
/// the local queue it shows (`None` for the connected peer), and the facts
/// the dropdown marks it with.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QueueChooserEntryProjection {
    pub label: String,
    /// The local Named queue the entry shows, or `None` for the connected
    /// peer Player owner: a peer entry is marked as a peer and cannot be
    /// renamed.
    pub queue: Option<NamedQueueId>,
    /// Whether this entry is the one the Queue panel shows.
    pub shown: bool,
    /// Whether this entry is the queue that holds the playhead.
    pub current: bool,
    /// Whether this entry is the current queue while the local Owner plays
    /// it: playback is active and not paused. A stopped or paused current
    /// queue still holds the playhead but is not marked as playing.
    pub playing: bool,
    /// The queue's slot count (design D8 of `gui-queue-panel-redesign`),
    /// read from that queue's freshest view: the shown resting queue's live
    /// view, the current queue's snapshot, another resting queue's published
    /// length, or the peer's own view.
    pub len: usize,
}

/// The rename draft an open Queue chooser holds, as the frame carries it: the
/// queue being renamed and the text the inline entry paints.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QueueChooserRenameProjection {
    pub queue: NamedQueueId,
    pub text: String,
    /// Whether the whole text is selected, so the entry paints it highlighted
    /// and the next character replaces it.
    pub selected: bool,
}

/// One button of the projected confirmation modal: the label, the key
/// hint inside the pill (`"Enter"`, `"Esc"`), and the tone the canvas
/// paints it in (the shared modal's own tones).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GuiConfirmButtonProjection {
    pub label: String,
    pub keys: String,
    pub tone: GuiConfirmButtonTone,
}

/// How a projected confirm button paints, mirroring `ConfirmButtonTone`
/// variant for variant.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GuiConfirmButtonTone {
    Affirmative,
    Cancel,
}

impl App {
    /// The Queue header's chooser facts (spec `gui-queue-chooser` "The queue
    /// header holds the chooser and its controls"): the chooser's entry count,
    /// the shown entry's name, and whether the close control is disabled.
    /// Read-only, and read from the same live entry list the chooser's keys
    /// and presses resolve against, so the header and the dropdown cannot
    /// disagree inside one tick.
    #[must_use]
    pub fn gui_queue_header_projection(&self) -> QueueHeaderProjection {
        let entries = self.queue_chooser_entries();
        QueueHeaderProjection {
            shown_name: entries
                .iter()
                .find(|entry| entry.shown)
                .map(|entry| entry.label.clone())
                .unwrap_or_default(),
            shown_len: self.displayed_queue().total_queue_len(),
            close_disabled: self.gui_close_disabled(),
            entries: entries.len(),
        }
    }

    /// Whether the close control is disabled for the entry the panel shows
    /// (spec `gui-queue-chooser` "The close control closes the shown entry"):
    /// a shown local queue closes only while the local Owner holds another one
    /// beside it, a shown peer entry always disconnects, and with no entry
    /// shown the control has nothing to close. The local count is the Owner's
    /// own list, never the chooser's, which carries the peer entry too — so a
    /// connected peer never disables the only local queue's close.
    #[must_use]
    fn gui_close_disabled(&self) -> bool {
        match self.gui_shown_entry {
            Some(ShownEntry::Peer) => false,
            Some(ShownEntry::Local(_)) => self.named_queues.entries().len() < 2,
            None => true,
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
                        // paint must not depend on that. The shared selection
                        // menu's bulk remove is destructive the same way.
                        destructive: matches!(
                            entry.action.as_ref(),
                            Some(
                                ContextAction::RemoveFromQueue(_)
                                    | ContextAction::RemoveSelection(_)
                                    | ContextAction::ClearQueue,
                            )
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
            // The Queue chooser's entries are the same live list its keys
            // resolve against, so the rows it paints and the rows a press or
            // a key addresses cannot disagree inside one frame.
            Some(GuiOverlay::QueueChooser(chooser)) => {
                let entries = self.queue_chooser_entries();
                Some(GuiOverlayProjection::QueueChooser {
                    cursor: chooser.cursor.min(entries.len().saturating_sub(1)),
                    entries,
                    renaming: chooser
                        .renaming
                        .as_ref()
                        .map(|draft| QueueChooserRenameProjection {
                            queue: draft.queue,
                            text: draft.text.clone(),
                            selected: draft.selected,
                        }),
                })
            }
            Some(GuiOverlay::PlayerMenu(menu)) => Some(GuiOverlayProjection::PlayerMenu {
                cursor: menu.cursor,
                x: menu.x,
                y: menu.y,
            }),
            // The confirm's message and button row are the modal's own text:
            // the frame carries them verbatim, and the action stays in the
            // core's held state so a press or key answers what the window
            // painted.
            Some(GuiOverlay::Confirm(modal)) => Some(GuiOverlayProjection::Confirm {
                message: modal.message.clone(),
                buttons: modal
                    .buttons
                    .iter()
                    .map(|button| GuiConfirmButtonProjection {
                        label: button.label.clone(),
                        keys: button.keys.clone(),
                        tone: match button.tone {
                            mbv_ui_model::confirm::ConfirmButtonTone::Affirmative => {
                                GuiConfirmButtonTone::Affirmative
                            }
                            mbv_ui_model::confirm::ConfirmButtonTone::Cancel => {
                                GuiConfirmButtonTone::Cancel
                            }
                        },
                    })
                    .collect(),
            }),
            // The daemon-lost dialog's diagnostics and its recovery choices:
            // the frame carries the same facts the TUI's modal paints, and
            // the choices come from the one table the dialog's chords read,
            // so a press and a key cannot name different answers.
            Some(GuiOverlay::DaemonLost(modal)) => Some(GuiOverlayProjection::DaemonLost {
                last_playing_title: modal.last_playing_title.clone(),
                daemon_log_path: modal.daemon_log_path.clone(),
                restart_error: modal.restart_error.clone(),
                buttons: daemon_lost_buttons(),
            }),
            None => None,
        }
    }
}
