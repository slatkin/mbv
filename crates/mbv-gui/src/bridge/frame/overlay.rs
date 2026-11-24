//! The open overlay's section (add-tauri-gui-client rows 6.2 and
//! 6.4, design D4): the row menu with its key hints, a track chip's popover
//! with its choices, or the playback-target chooser with its choices. Each
//! field mirrors one fact of the library's `GuiOverlayProjection`, built
//! from the same playback projection read in the same tick, so the
//! popover's choices and the track chips, and the chooser's choices and the
//! header's target fact, cannot disagree inside one tick.

use crate::gui_core::{
    GuiOverlayProjection, QueueChooserEntryProjection, QueueChooserRenameProjection,
    RowMenuEntryProjection, TrackPopoverChoice,
};
use mbv_ui_msg::TrackChipKind;

use super::playback::GuiTargetChoice;

/// The open menu or popover the window paints. `None` in the section while no
/// overlay is open. The player menu's anchor carries CSS pixels, so the
/// equality is `PartialEq` only.
#[derive(Debug, Clone, PartialEq)]
pub enum GuiOverlayFrame {
    /// The player panel's context menu (gui-player-overlay-menu): the cursor
    /// over its one auto-hide entry and the right-click's window anchor. The
    /// entry itself is the window's: its checked fact is the playback
    /// section's `controls.auto_hide`, the one fact the panel already holds.
    PlayerMenu {
        cursor: usize,
        /// The right-click's window anchor, in CSS pixels.
        x: f32,
        y: f32,
    },
    /// The queue row's menu: the anchor row, the menu cursor, and the
    /// entries, each with the key hint that selects it directly (the
    /// canvas's hints).
    RowMenu {
        /// The row the menu opened on (`QueueSlotId::raw`): the window anchors
        /// the menu there instead of guessing from the hover or the cursor.
        anchor_slot: u64,
        cursor: usize,
        entries: Vec<GuiRowMenuEntry>,
    },
    /// A track chip's popover: the chip it belongs to, the cursor, and the
    /// chip's choices — the subtitle popover lists Off first.
    TrackPopover {
        chip: GuiTrackChipKind,
        cursor: usize,
        choices: Vec<GuiTrackPopoverChoice>,
    },
    /// The playback-target chooser (add-tauri-gui-client row 6.4): the
    /// cursor and the cast/session choices — the same list the playback
    /// section's `targetChoices` carries.
    TargetChooser {
        cursor: usize,
        choices: Vec<GuiTargetChoice>,
        /// Whether a target refresh is in flight: the chooser's Refresh row
        /// paints busy until it lands.
        refreshing: bool,
    },
    /// The Queue chooser (gui-named-queues design D8): the cursor, the local
    /// queues in queue order followed by the connected peer, each with the
    /// marks the dropdown paints, and the rename draft while one is open.
    QueueChooser {
        cursor: usize,
        entries: Vec<GuiQueueChooserEntry>,
        renaming: Option<GuiQueueChooserRename>,
    },
    /// The shared confirmation modal the App raised: the message and the
    /// button row, each with the key hint inside its pill and the tone the
    /// canvas paints it in. The action stays in the core's held state, so a
    /// press or key answers what the window painted.
    Confirm {
        message: String,
        buttons: Vec<GuiConfirmButton>,
    },
    /// The daemon-lost recovery dialog the App raised: the diagnostics the
    /// TUI's modal paints, and the recovery choices, each with its key hint
    /// and tone. The dialog stays in the core's held state, so a press or key
    /// answers what the window painted.
    DaemonLost {
        /// The title of whatever was playing right before the Owner vanished;
        /// `None` when nothing was.
        last_playing_title: Option<String>,
        daemon_log_path: String,
        /// The last restart attempt's failure, painted as the dialog's
        /// `Restart failed:` line.
        restart_error: Option<String>,
        buttons: Vec<GuiConfirmButton>,
    },
}

/// One button of the carried confirmation modal: the label, the key hint
/// the pill paints (`"Enter"`, `"Esc"`), and how the canvas paints it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuiConfirmButton {
    pub label: String,
    pub keys: String,
    pub tone: GuiConfirmButtonTone,
}

/// How a confirm button paints, mirroring `ConfirmButtonTone` variant for
/// variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuiConfirmButtonTone {
    Affirmative,
    Cancel,
}

impl From<&GuiOverlayProjection> for GuiOverlayFrame {
    fn from(overlay: &GuiOverlayProjection) -> Self {
        match overlay {
            GuiOverlayProjection::PlayerMenu { cursor, x, y } => Self::PlayerMenu {
                cursor: *cursor,
                x: *x,
                y: *y,
            },
            GuiOverlayProjection::RowMenu {
                anchor_slot,
                cursor,
                entries,
            } => Self::RowMenu {
                anchor_slot: anchor_slot.raw(),
                cursor: *cursor,
                entries: entries.iter().map(GuiRowMenuEntry::from).collect(),
            },
            GuiOverlayProjection::TrackPopover {
                chip,
                cursor,
                choices,
            } => Self::TrackPopover {
                chip: GuiTrackChipKind::from(*chip),
                cursor: *cursor,
                choices: choices.iter().map(GuiTrackPopoverChoice::from).collect(),
            },
            GuiOverlayProjection::TargetChooser {
                cursor,
                choices,
                refreshing,
            } => Self::TargetChooser {
                cursor: *cursor,
                choices: choices.iter().map(GuiTargetChoice::from).collect(),
                refreshing: *refreshing,
            },
            GuiOverlayProjection::QueueChooser {
                cursor,
                entries,
                renaming,
            } => Self::QueueChooser {
                cursor: *cursor,
                entries: entries.iter().map(GuiQueueChooserEntry::from).collect(),
                renaming: renaming.as_ref().map(GuiQueueChooserRename::from),
            },
            GuiOverlayProjection::Confirm { message, buttons } => Self::Confirm {
                message: message.clone(),
                buttons: buttons
                    .iter()
                    .map(|button| GuiConfirmButton {
                        label: button.label.clone(),
                        keys: button.keys.clone(),
                        tone: match button.tone {
                            crate::gui_core::GuiConfirmButtonTone::Affirmative => {
                                GuiConfirmButtonTone::Affirmative
                            }
                            crate::gui_core::GuiConfirmButtonTone::Cancel => {
                                GuiConfirmButtonTone::Cancel
                            }
                        },
                    })
                    .collect(),
            },
            GuiOverlayProjection::DaemonLost {
                last_playing_title,
                daemon_log_path,
                restart_error,
                buttons,
            } => Self::DaemonLost {
                last_playing_title: last_playing_title.clone(),
                daemon_log_path: daemon_log_path.clone(),
                restart_error: restart_error.clone(),
                buttons: buttons
                    .iter()
                    .map(|button| GuiConfirmButton {
                        label: button.label.clone(),
                        keys: button.keys.clone(),
                        tone: match button.tone {
                            crate::gui_core::GuiConfirmButtonTone::Affirmative => {
                                GuiConfirmButtonTone::Affirmative
                            }
                            crate::gui_core::GuiConfirmButtonTone::Cancel => {
                                GuiConfirmButtonTone::Cancel
                            }
                        },
                    })
                    .collect(),
            },
        }
    }
}

/// One row-menu entry as the section carries it: the label, the key chord that
/// selects the entry directly when it has one, and whether it is
/// destructive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuiRowMenuEntry {
    pub label: String,
    pub key_hint: Option<String>,
    /// The row menu paints the entry in the canvas's destructive tone under
    /// a divider (design DR6); the flag derives from the entry's action in
    /// the library's projection.
    pub destructive: bool,
    /// The entry's toggle state, when it is a check entry: the player menu's
    /// auto-hide entry paints the small box the former checkbox painted
    /// (gui-player-overlay-menu). `None` paints a plain entry.
    pub checked: Option<bool>,
    /// The entry's own index in the shared builder's order, when the frame
    /// carries one (the Library's item menu): the menu paints and reports it
    /// in place of the paint order, so a window that paints only some
    /// entries still names the entry it painted. The queue's entries carry
    /// none, so the paint order stands.
    pub index: Option<usize>,
}

impl From<&RowMenuEntryProjection> for GuiRowMenuEntry {
    fn from(entry: &RowMenuEntryProjection) -> Self {
        Self {
            label: entry.label.clone(),
            key_hint: entry.key_hint.clone(),
            destructive: entry.destructive,
            index: None,
            checked: None,
        }
    }
}

/// Which track chip a popover belongs to, mirroring `TrackChipKind` variant
/// for variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuiTrackChipKind {
    Audio,
    Subtitles,
}

impl From<TrackChipKind> for GuiTrackChipKind {
    fn from(kind: TrackChipKind) -> Self {
        match kind {
            TrackChipKind::Audio => Self::Audio,
            TrackChipKind::Subtitles => Self::Subtitles,
        }
    }
}

/// One choice of an open track popover, mirroring `TrackPopoverChoice`: the
/// mpv track id a choice sends, its label, whether a subtitle track is
/// forced, and whether it is the track playing now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuiTrackPopoverChoice {
    pub id: i64,
    pub label: String,
    pub forced: bool,
    pub chosen: bool,
}

impl From<&TrackPopoverChoice> for GuiTrackPopoverChoice {
    fn from(choice: &TrackPopoverChoice) -> Self {
        Self {
            id: choice.id,
            label: choice.label.clone(),
            forced: choice.forced,
            chosen: choice.chosen,
        }
    }
}

/// One entry of the open Queue chooser: its label (`NamedQueueId::raw` for a
/// local queue, `None` for the connected peer) and the marks the dropdown
/// paints.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuiQueueChooserEntry {
    pub label: String,
    /// The local Named queue the entry shows (`NamedQueueId::raw`), or `None`
    /// for the connected peer Player owner.
    pub queue: Option<u64>,
    /// Whether this entry is the one the Queue panel shows.
    pub shown: bool,
    /// Whether this entry is the queue that holds the playhead.
    pub current: bool,
    /// Whether this entry is the current queue while the local Owner plays it.
    pub playing: bool,
    /// The queue's slot count, painted at the entry's trailing end (design D8
    /// of `gui-queue-panel-redesign`).
    pub len: usize,
}

impl From<&QueueChooserEntryProjection> for GuiQueueChooserEntry {
    fn from(entry: &QueueChooserEntryProjection) -> Self {
        Self {
            label: entry.label.clone(),
            queue: entry.queue.map(mbv_queue::NamedQueueId::raw),
            shown: entry.shown,
            current: entry.current,
            playing: entry.playing,
            len: entry.len,
        }
    }
}

/// The rename draft an open Queue chooser holds: the queue it renames
/// (`NamedQueueId::raw`) and the text its inline entry paints.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuiQueueChooserRename {
    pub queue: u64,
    pub text: String,
    /// Whether the whole text is selected: the entry paints it highlighted.
    pub selected: bool,
}

impl From<&QueueChooserRenameProjection> for GuiQueueChooserRename {
    fn from(draft: &QueueChooserRenameProjection) -> Self {
        Self {
            queue: draft.queue.raw(),
            text: draft.text.clone(),
            selected: draft.selected,
        }
    }
}
