//! Placeholder (task 5.5): the Queue panel's port (`QueuePanel.svelte`) lands
//! there. The marker the placeholder paints lets the `Panel` tests assert the
//! composition while the real port is out; the port replaces it.

use dioxus_native::prelude::*;

/// The Queue panel, painted until it is ported.
#[component]
pub fn QueuePanel() -> Element {
    rsx! {
        div { "data-panel-child": "QueuePanel" }
    }
}
