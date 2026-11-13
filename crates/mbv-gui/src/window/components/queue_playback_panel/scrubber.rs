//! The scrubber seam (task 5.3): the port of `QueuePlaybackPanel.svelte`'s
//! scrubber block — the 6 px track, the intro and credits blocks, the played
//! fill, the 2×14 chapter ticks, the 16 px thumb, the hover position line,
//! the hover preview with the trickplay tile, and the elapsed and
//! `−remaining · ends HH:MM` labels. Task 5.2 landed the component's seat in
//! the transport column and the `seekbar` region's element handle; the
//! scrubber's own markup and its `QueuePlaybackPanel.test.ts` contracts land
//! there.

use dioxus_native::prelude::*;

/// The seekbar region's element handle (design D4): the scrubber's slider
/// box is the `seekbar` region of the panel's one report, so the panel holds
/// the handle and the scrubber sets it on mount.
#[derive(Props, Clone, PartialEq)]
pub struct ScrubberProps {
    /// The `seekbar` element's mounted event, `None` while the element is
    /// absent.
    #[props(default)]
    pub seekbar_mounted: Signal<Option<Event<MountedData>>>,
}

/// The scrubber, painted until task 5.3 ports it.
#[component]
pub fn Scrubber(props: ScrubberProps) -> Element {
    let _ = props.seekbar_mounted;
    rsx! {}
}
