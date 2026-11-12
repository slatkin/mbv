//! The Queue playback artwork (design D10): the App projection's state encoded
//! into the `data:` URL the webview paints. The memo encodes once per state
//! class; the section ledger compares each value it builds, so an unchanged
//! artwork sends nothing.

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

/// The artwork memo (design D10): the encoded value for the current state
/// class.
#[derive(Debug, Default)]
pub(crate) struct ArtworkFrames {
    /// The encoded value for the current class: an encode runs once per
    /// class change.
    encoded: Option<(ArtworkClass, GuiCardArtwork)>,
}

impl ArtworkFrames {
    /// The artwork value for `artwork`, encoded once per state class. The
    /// section ledger carries the value and compares it, so it always reports
    /// the current state.
    pub(crate) fn current_value(&mut self, artwork: &QueueCardArtwork) -> GuiCardArtwork {
        let class = ArtworkClass::of(artwork);
        match self.encoded.as_ref() {
            Some((encoded_class, value)) if *encoded_class == class => value.clone(),
            _ => {
                let value = encode(artwork);
                self.encoded = Some((class, value.clone()));
                value
            }
        }
    }
}

/// The frame's artwork value for one projection state: the image bytes as a
/// `data:` URL. The webview never fetches artwork — the bytes are the
/// core's cache reads — so the URL needs only the CSP's `data:` image
/// source. The bytes are the server's own, as the image cache holds them,
/// so the URL carries the MIME type those bytes declare.
fn encode(artwork: &QueueCardArtwork) -> GuiCardArtwork {
    match artwork {
        QueueCardArtwork::Image { source, .. } => {
            GuiCardArtwork::Image(data_url(source.mime, &source.bytes))
        }
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
