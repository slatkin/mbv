//! Placeholder (task 5.6): the Queue footer's port (`QueueFooter.svelte`)
//! lands there. The marker the placeholder paints lets the `Panel` tests
//! assert the composition while the real port is out; the port replaces it.

use dioxus_native::prelude::*;

/// The props the `Panel` call site passes.
#[derive(Props, Debug, Clone, PartialEq)]
pub struct QueueFooterProps {
    /// Whether the Library panel shows beside this one: the Library Status
    /// bar owns the armed prefix and live toast then, so the footer line
    /// paints neither.
    #[props(default)]
    pub library_visible: bool,
}

/// The Queue footer, painted until it is ported.
#[component]
pub fn QueueFooter(props: QueueFooterProps) -> Element {
    let _ = props.library_visible;
    rsx! {
        div { "data-panel-child": "QueueFooter" }
    }
}
