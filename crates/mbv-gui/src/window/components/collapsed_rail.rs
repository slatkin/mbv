//! Placeholder (task 5.7): the collapsed rail's port (`CollapsedRail.svelte`)
//! lands there. The marker the placeholder paints lets the `Panel` tests
//! assert the composition while the real port is out; the port replaces it.

use dioxus_native::prelude::*;

use crate::art::GuiArtRef;

/// The props the `Panel` call site passes.
#[derive(Props, Debug, Clone, PartialEq)]
pub struct CollapsedRailProps {
    /// The artwork reference the playback section carries.
    #[props(default)]
    pub art: Option<GuiArtRef>,
}

/// The collapsed rail, painted until it is ported.
#[component]
pub fn CollapsedRail(props: CollapsedRailProps) -> Element {
    let _ = props.art;
    rsx! {
        div { "data-panel-child": "CollapsedRail" }
    }
}
