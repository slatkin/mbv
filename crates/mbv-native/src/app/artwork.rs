//! The playback artwork (B5 of `blitz-queue-panel-spike`). The window paints
//! the reference's `src` for an image and for the placeholder, and nothing
//! while the reference loads or is absent.

use dioxus_native::prelude::*;
use mbv_gui::art::GuiArtRef;
use mbv_gui::bridge::frame::playback::GuiQueuePlayback;

/// The URL the window paints for `art`, or `None` when it paints nothing.
#[must_use]
pub fn art_src(art: &GuiArtRef) -> Option<&str> {
    match art {
        GuiArtRef::Image { src } | GuiArtRef::Placeholder { src } => Some(src),
        GuiArtRef::Loading => None,
    }
}

/// The playback artwork slot. It reads the playback section only.
#[component]
pub fn Artwork(playback: Signal<Option<GuiQueuePlayback>>) -> Element {
    let src = playback
        .read()
        .as_ref()
        .and_then(|playback| playback.artwork.as_ref())
        .and_then(art_src)
        .map(str::to_owned);
    rsx! {
        div { class: "artwork-slot",
            if let Some(src) = src {
                img { class: "artwork", src: "{src}" }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use mbv_gui::art::GuiArtRef;

    use super::art_src;

    #[test]
    fn loading_artwork_paints_no_source() {
        assert_eq!(art_src(&GuiArtRef::Loading), None);
    }
}
