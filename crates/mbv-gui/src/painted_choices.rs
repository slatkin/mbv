//! The choice lists the window last painted (add-tauri-gui-client row
//! 6.5's review of row 6.4): a pointer press's region index addresses the list
//! the window painted, so a session, cast, or track-list change between the
//! paint and the press selects the painted choice or nothing — never the
//! neighbour an index would name against a fresh read.
//!
//! The lists are adopted from the batch the core sends, which is the same
//! value the window paints. A press resolves against them and names the
//! choice's stable identity (the target key, the mpv track id); the App then
//! re-resolves that identity against its live state, so a vanished choice
//! applies nothing.

use mbv_ui_model::panel_targets::ChooserTargetKey;

use crate::bridge::frame::overlay::GuiOverlayFrame;
use crate::bridge::frame::playback::chooser_target_key;
/// The open overlay's choice lists as the window last painted them.
/// A section value with no overlay (or the row menu, whose entries carry their own
/// resolved targets) clears both.
#[derive(Debug, Default)]
pub(crate) struct PaintedChoices {
    /// The playback-target chooser's choices, in paint order.
    target_keys: Vec<ChooserTargetKey>,
    /// The open track popover's choice track ids, in paint order. `None`
    /// while no popover is open.
    popover_track_ids: Option<Vec<i64>>,
}

impl PaintedChoices {
    /// Adopt the choices `overlay` paints. The core calls this with the
    /// batch it is about to send, so a press after this batch addresses what
    /// the window shows.
    pub(crate) fn adopt(&mut self, overlay: Option<&GuiOverlayFrame>) {
        self.target_keys.clear();
        self.popover_track_ids = None;
        match overlay {
            Some(GuiOverlayFrame::TargetChooser { choices, .. }) => {
                self.target_keys = choices
                    .iter()
                    .map(|choice| chooser_target_key(&choice.key))
                    .collect();
            }
            Some(GuiOverlayFrame::TrackPopover { choices, .. }) => {
                let ids = choices.iter().map(|choice| choice.id).collect();
                self.popover_track_ids = Some(ids);
            }
            // The confirm and the daemon-lost dialog name no choice list:
            // their buttons resolve through their own regions, not through
            // painted choice indexes.
            Some(
                GuiOverlayFrame::RowMenu { .. }
                | GuiOverlayFrame::Confirm { .. }
                | GuiOverlayFrame::DaemonLost { .. },
            )
            | None => {}
        }
    }

    /// The chooser target the painted row at `index` names: `None` when the
    /// window painted no chooser or fewer rows than that.
    pub(crate) fn target_key(&self, index: usize) -> Option<&ChooserTargetKey> {
        self.target_keys.get(index)
    }

    /// The track id the painted popover row at `index` names: `None` when
    /// the window painted no popover or fewer rows than that.
    pub(crate) fn popover_track_id(&self, index: usize) -> Option<i64> {
        self.popover_track_ids
            .as_ref()
            .and_then(|ids| ids.get(index))
            .copied()
    }
}
