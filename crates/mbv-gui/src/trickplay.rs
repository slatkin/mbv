//! The scrubber-preview trickplay tile (task 6.7, design D4a): the hovered
//! thumbnail's place in its sheet, and the sheet's bytes as the `data:` URL
//! the webview paints.
//!
//! The sheet's image is the largest payload of the scrubber section, so the
//! memo holds the encoded URL per sheet key: a key change re-encodes, and the
//! section ledger compares each tile it builds, so an unchanged hover sends
//! nothing.

use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use mbv::TrickplayPreviewTile;

use crate::bridge::frame::playback::GuiScrubberTile;

/// The trickplay memo (task 6.7): the encoded sheet for the current key.
#[derive(Debug, Default)]
pub(crate) struct TrickplayFrames {
    /// The encoded sheet for the current key: a key change re-encodes, and a
    /// return to the previous sheet re-carries without one.
    encoded: Option<(String, String)>,
}

impl TrickplayFrames {
    /// The tile for `preview` with its sheet image always carried. The section
    /// ledger carries the tile and compares it, so the sheet travels in the
    /// value.
    pub(crate) fn current_tile(&mut self, preview: &TrickplayPreviewTile) -> GuiScrubberTile {
        let sheet_key = preview.key.wire_key();
        let sheet = self.encoded_sheet(&sheet_key, preview);
        tile(preview, sheet_key, sheet)
    }

    /// The sheet's `data:` URL, encoded once per key.
    fn encoded_sheet(&mut self, sheet_key: &str, preview: &TrickplayPreviewTile) -> String {
        if let Some((key, image)) = self.encoded.as_ref()
            && key == sheet_key
        {
            return image.clone();
        }
        let image = format!("data:image/jpeg;base64,{}", BASE64.encode(&preview.jpeg));
        self.encoded = Some((sheet_key.to_string(), image.clone()));
        image
    }
}

/// The window's tile for `preview`: the thumbnail's place in the sheet named by
/// `sheet_key`, with the sheet image.
fn tile(preview: &TrickplayPreviewTile, sheet_key: String, sheet: String) -> GuiScrubberTile {
    GuiScrubberTile {
        sheet_key,
        width: preview.width,
        height: preview.height,
        columns: preview.columns,
        column: preview.column,
        row: preview.row,
        sheet: Some(sheet),
    }
}
