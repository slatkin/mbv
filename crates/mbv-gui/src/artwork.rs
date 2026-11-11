//! The frame's Queue playback artwork (design D10): the App projection's
//! state encoded into the `data:` URL the webview paints, armed per window.
//!
//! The memo holds the value the last frame carried, so a frame carries the
//! artwork again only when the slot's art changed — the theme's arming rule
//! (design D7). A window reload forgets the front end's copy; the core
//! re-arms on the reload's `report_layout`, and the next frame re-carries
//! the value with no re-encode behind it.

use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use mbv::QueueCardArtwork;
use mbv_images::QUEUE_CARD_PLACEHOLDER_BYTES;

use crate::bridge::frame::playback::GuiCardArtwork;

/// The artwork state class one encode is keyed by: the state without the
/// payload bytes, which an unchanged state's re-encode never changes.
#[derive(Debug, Clone, PartialEq, Eq)]
enum ArtworkClass {
    Image(String),
    Placeholder,
    Loading,
    Hidden,
}

impl ArtworkClass {
    fn of(artwork: &QueueCardArtwork) -> Self {
        match artwork {
            QueueCardArtwork::Image { key, .. } => Self::Image(key.clone()),
            QueueCardArtwork::Placeholder => Self::Placeholder,
            QueueCardArtwork::Loading => Self::Loading,
            QueueCardArtwork::Hidden => Self::Hidden,
        }
    }
}

/// The frames' artwork memo (design D10): the value encoded per state class
/// and the value the last frame carried.
#[derive(Debug, Default)]
pub(crate) struct ArtworkFrames {
    /// The encoded value for the current class: an encode runs once per
    /// class change, so a re-armed frame re-carries without one.
    encoded: Option<(ArtworkClass, GuiCardArtwork)>,
    /// The value the last frame carried; `None` arms the next frame.
    sent: Option<GuiCardArtwork>,
}

impl ArtworkFrames {
    /// The frame's artwork value for `artwork`: `None` when the last frame
    /// already carried it.
    pub(crate) fn frame_value(&mut self, artwork: &QueueCardArtwork) -> Option<GuiCardArtwork> {
        let class = ArtworkClass::of(artwork);
        let value = match self.encoded.as_ref() {
            Some((encoded_class, value)) if *encoded_class == class => value.clone(),
            _ => {
                let value = encode(artwork);
                self.encoded = Some((class, value.clone()));
                value
            }
        };
        if self.sent.as_ref() == Some(&value) {
            return None;
        }
        self.sent = Some(value.clone());
        Some(value)
    }

    /// Arm the next frame: a window reload's `report_layout` re-carries the
    /// current artwork, exactly as it re-arms the theme (design D7).
    pub(crate) fn rearm(&mut self) {
        self.sent = None;
    }
}

/// The frame's artwork value for one projection state: the image bytes as a
/// `data:` URL. The webview never fetches artwork — the bytes are the
/// core's cache reads — so the URL needs only the CSP's `data:` image
/// source.
fn encode(artwork: &QueueCardArtwork) -> GuiCardArtwork {
    match artwork {
        QueueCardArtwork::Image { jpeg, .. } => GuiCardArtwork::Image(data_url("image/jpeg", jpeg)),
        QueueCardArtwork::Placeholder => {
            GuiCardArtwork::Placeholder(data_url("image/webp", QUEUE_CARD_PLACEHOLDER_BYTES))
        }
        QueueCardArtwork::Loading => GuiCardArtwork::Loading,
        QueueCardArtwork::Hidden => GuiCardArtwork::Hidden,
    }
}

pub(crate) fn data_url(mime: &str, bytes: &[u8]) -> String {
    format!("data:{mime};base64,{}", BASE64.encode(bytes))
}
