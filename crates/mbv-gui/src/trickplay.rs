//! The frame's scrubber-preview trickplay tile (task 6.7, design D4a): the
//! hovered thumbnail's place in its sheet, and the sheet's bytes as the
//! `data:` URL the webview paints.
//!
//! The sheet's image is the frame's largest payload, so the memo holds the
//! encoded URL per sheet key and a frame re-carries it only when the hovered
//! sheet changes — the artwork's arming rule (design D10). The window latches
//! the image it last received, keyed by the sheet key the tile always names.

use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use mbv::TrickplayPreviewTile;

use crate::bridge::frame::playback::GuiScrubberTile;

/// The frames' trickplay memo (task 6.7): the encoded sheet for the current
/// key and the key whose image the last frame carried.
#[derive(Debug, Default)]
pub(crate) struct TrickplayFrames {
    /// The encoded sheet for the current key: a key change re-encodes, and a
    /// return to the previous sheet re-carries without one.
    encoded: Option<(String, String)>,
    /// The sheet key whose image the last frame carried; `None` arms the next
    /// frame.
    sent_sheet: Option<String>,
}

impl TrickplayFrames {
    /// The frame's tile for `preview`, `None` while the preview is absent (the
    /// pointer is away from the bar, or the item's sheet is not cached). The
    /// tile travels in every covered frame; the sheet image only when the
    /// hovered sheet changed for this window.
    pub(crate) fn frame_value(
        &mut self,
        preview: Option<&TrickplayPreviewTile>,
    ) -> Option<GuiScrubberTile> {
        let preview = preview?;
        let sheet_key = preview.key.wire_key();
        let carry_sheet = self.sent_sheet.as_ref() != Some(&sheet_key);
        let sheet = carry_sheet.then(|| self.encoded_sheet(&sheet_key, preview));
        if carry_sheet {
            self.sent_sheet = Some(sheet_key.clone());
        }
        Some(GuiScrubberTile {
            sheet_key,
            width: preview.width,
            height: preview.height,
            columns: preview.columns,
            column: preview.column,
            row: preview.row,
            sheet,
        })
    }

    /// Arm the next frame: a window reload's `report_layout` re-carries the
    /// hovered sheet, exactly as it re-arms the artwork (design D7).
    pub(crate) fn rearm(&mut self) {
        self.sent_sheet = None;
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

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use mbv::{TrickplayPreviewTile, TrickplaySheetKey};

    use super::TrickplayFrames;

    /// A preview of one thumbnail in a sheet of `key`.
    fn preview(key: TrickplaySheetKey, column: u32) -> TrickplayPreviewTile {
        TrickplayPreviewTile {
            key,
            width: 320,
            height: 180,
            columns: 10,
            column,
            row: 0,
            jpeg: Arc::from(&b"jpeg"[..]),
        }
    }

    fn sheet_key(sheet: u32) -> TrickplaySheetKey {
        TrickplaySheetKey {
            item_id: "item1".to_string(),
            width: 320,
            sheet,
        }
    }

    /// One sheet's image reaches the frame once: the next frame of the same
    /// sheet names it without re-carrying the bytes, and the window repaints
    /// the image it latched for that key.
    #[test]
    fn a_sheets_image_is_carried_once_per_window() {
        let mut frames = TrickplayFrames::default();

        let first = frames
            .frame_value(Some(&preview(sheet_key(0), 0)))
            .expect("the first frame carries the tile");
        let second = frames
            .frame_value(Some(&preview(sheet_key(0), 1)))
            .expect("a tile move carries the tile");

        assert_eq!(
            first.sheet.as_deref(),
            Some("data:image/jpeg;base64,anBlZw==")
        );
        assert_eq!(second.sheet, None);
        assert_eq!(second.sheet_key, first.sheet_key);
        assert_eq!(second.column, 1);
    }

    /// A hover onto another sheet carries that sheet's image: the window
    /// cannot repaint a sheet it never received.
    #[test]
    fn a_new_sheet_carries_its_own_image() {
        let mut frames = TrickplayFrames::default();
        let _first = frames
            .frame_value(Some(&preview(sheet_key(0), 0)))
            .expect("the first sheet is carried");

        let next = frames
            .frame_value(Some(&preview(sheet_key(1), 0)))
            .expect("the second sheet is carried");

        assert!(next.sheet.is_some());
        assert_eq!(next.sheet_key, "item1:320:1");
    }

    /// A pointer away from the bar carries no tile, and a reload's re-arm
    /// makes the next frame re-carry the sheet it had already sent.
    #[test]
    fn arming_again_re_carries_the_sheet() {
        let mut frames = TrickplayFrames::default();
        let _first = frames
            .frame_value(Some(&preview(sheet_key(0), 0)))
            .expect("the first frame carries the tile");
        assert_eq!(frames.frame_value(None), None);

        frames.rearm();

        let rearmed = frames
            .frame_value(Some(&preview(sheet_key(0), 0)))
            .expect("the re-armed frame carries the tile");
        assert!(rearmed.sheet.is_some());
    }
}
