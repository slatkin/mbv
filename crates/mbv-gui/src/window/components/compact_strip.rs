//! Placeholder (task 5.7): the compact strip's port (`CompactStrip.svelte`)
//! lands there. The marker the placeholder paints lets the `Panel` tests
//! assert the composition while the real port is out; the port replaces it.

use dioxus_native::prelude::*;

use crate::art::GuiArtRef;

/// The props the `Panel` call site passes.
#[derive(Props, Debug, Clone, PartialEq)]
pub struct CompactStripProps {
    /// The next item's title, from the summary section: `None` while no slot
    /// follows the playhead.
    #[props(default)]
    pub next: Option<String>,
    /// The artwork reference the playback section carries.
    #[props(default)]
    pub art: Option<GuiArtRef>,
}

/// The compact strip, painted until it is ported.
#[component]
pub fn CompactStrip(props: CompactStripProps) -> Element {
    let _ = (props.next, props.art);
    rsx! {
        div { "data-panel-child": "CompactStrip" }
    }
}
