//! The choice lists the last emitted frame painted (add-tauri-gui-client row
//! 6.5's review of row 6.4): a pointer press's region index addresses the list
//! the window painted, so a session, cast, or track-list change between the
//! paint and the press selects the painted choice or nothing — never the
//! neighbour an index would name against a fresh read.
//!
//! The lists are adopted from the frame the core emits, which is the same
//! value the window paints. A press resolves against them and names the
//! choice's stable identity (the target key, the mpv track id); the App then
//! re-resolves that identity against its live state, so a vanished choice
//! applies nothing.

use mbv_ui_model::panel_targets::{ChooserTargetKey, SessionTargetKey};

use crate::bridge::frame::overlay::GuiOverlayFrame;
use crate::bridge::frame::playback::GuiTargetKey;
/// The open overlay's choice lists as the last emitted frame painted them.
/// A frame with no overlay (or the row menu, whose entries carry their own
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
    /// frame it is about to emit, so a press after this frame addresses what
    /// the window shows.
    pub(crate) fn adopt(&mut self, overlay: Option<&GuiOverlayFrame>) {
        self.target_keys.clear();
        self.popover_track_ids = None;
        match overlay {
            Some(GuiOverlayFrame::TargetChooser { choices, .. }) => {
                self.target_keys = choices
                    .iter()
                    .map(|choice| painted_target_key(&choice.key))
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
    /// last frame painted no chooser or fewer rows than that.
    pub(crate) fn target_key(&self, index: usize) -> Option<&ChooserTargetKey> {
        self.target_keys.get(index)
    }

    /// The track id the painted popover row at `index` names: `None` when
    /// the last frame painted no popover or fewer rows than that.
    pub(crate) fn popover_track_id(&self, index: usize) -> Option<i64> {
        self.popover_track_ids
            .as_ref()
            .and_then(|ids| ids.get(index))
            .copied()
    }
}

/// The reverse of the frame's `GuiTargetKey` projection: the stable identity
/// a painted chooser row names, back in the shared vocabulary the selection
/// request carries. A function, not a `From` impl: the conversion is this
/// bridge's, and an impl on the shared key type would spread it crate-wide.
fn painted_target_key(key: &GuiTargetKey) -> ChooserTargetKey {
    match key {
        GuiTargetKey::Local => ChooserTargetKey::Local,
        GuiTargetKey::Emby(id) => ChooserTargetKey::Target(SessionTargetKey::Emby(id.clone())),
        GuiTargetKey::Cast(id) => ChooserTargetKey::Target(SessionTargetKey::Cast(id.clone())),
    }
}
