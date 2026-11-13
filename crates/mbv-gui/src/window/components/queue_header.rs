//! Placeholder (task 5.6): the Queue header's port (`QueueHeader.svelte`)
//! lands there. The marker the placeholder paints lets the composition tests
//! assert the panel's children while the real port is out; the port replaces
//! it, and the header's `rightColumn` and `filter` regions start reporting
//! through the Queue panel's own key then (the `QueuePanel.test.ts` header
//! and region contracts travel with it).

use dioxus_native::prelude::*;

use crate::bridge::frame::queue::GuiQueueSummary;

/// The props of the Queue header (the `QueueHeader.svelte` props).
#[derive(Props, Debug, Clone, PartialEq)]
pub struct QueueHeaderProps {
    /// The queue summary the header paints.
    pub summary: GuiQueueSummary,
}

/// The Queue header, painted until it is ported.
#[component]
pub fn QueueHeader(props: QueueHeaderProps) -> Element {
    let _ = props.summary;
    rsx! {
        div { "data-panel-child": "QueueHeader" }
    }
}
