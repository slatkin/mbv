//! The GUI Client's frame projection types (design D4, D10): what the
//! bridge's frame carries for the canvas panel's toggles and the open
//! overlay, plus the `App` method that builds the overlay's projection.

use mbv_queue::QueueSlotId;
use mbv_ui_model::context_menu::ContextAction;
use mbv_ui_msg::TrackChipKind;

use super::overlay::GuiOverlay;
use super::popover::track_popover_choices;
use crate::app::App;
use crate::app::queue_playback_projection::{PlaybackTargetChoice, QueuePlaybackProjection};

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
    /// The shared confirmation modal the App raised (`ask_confirm`): the
    /// message and the button row, as the frame carries them. The action
    /// stays in the core's held state (`GuiOverlay::Confirm`).
    Confirm {
        message: String,
        buttons: Vec<GuiConfirmButtonProjection>,
    },
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
            None => None,
        }
    }
}
