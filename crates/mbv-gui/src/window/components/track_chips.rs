//! Placeholder (task 5.4): the track chips row's port (`TrackChips.svelte`)
//! lands there — the video label chip and the audio and subtitle chips with
//! the open track popover. Until then the row paints a marker, which the
//! `Panel` tests read while the composition is out.

use dioxus_native::prelude::*;

use crate::bridge::frame::overlay::GuiOverlayFrame;
use crate::bridge::frame::playback::GuiTrackChips;

/// The props the `QueuePlaybackPanel` call site passes.
#[derive(Props, Debug, Clone, PartialEq)]
pub struct TrackChipsProps {
    /// The track chips the playback section carries, `None` while the local
    /// player is inactive.
    #[props(default)]
    pub chips: Option<GuiTrackChips>,
    /// The open menu or popover, `None` while none is open.
    #[props(default)]
    pub overlay: Option<GuiOverlayFrame>,
}

/// The track chips row, painted until it is ported.
#[component]
pub fn TrackChips(props: TrackChipsProps) -> Element {
    let _ = (props.chips, props.overlay);
    rsx! {
        div { "data-panel-child": "TrackChips" }
    }
}
