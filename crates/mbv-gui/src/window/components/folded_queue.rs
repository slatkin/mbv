//! Placeholder (task 5.5): the folded queue's port (`FoldedQueue.svelte`)
//! lands there. The marker the placeholder paints lets the `Panel` tests
//! assert the composition while the real port is out; the port replaces it.

use dioxus_native::prelude::*;

/// The folded queue's one-line summary, painted until it is ported.
#[component]
pub fn FoldedQueue() -> Element {
    rsx! {
        div { "data-panel-child": "FoldedQueue" }
    }
}
