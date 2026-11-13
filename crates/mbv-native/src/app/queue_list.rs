//! The queue list (D9 of `blitz-queue-panel-spike`): the viewport rows of the
//! `queue` section, the active marker, and the active row's played fraction.
//! The core already cut the rows to the viewport, so the list paints them as
//! they arrive.

use dioxus_native::prelude::*;
use mbv_gui::bridge::frame::queue::GuiQueueRow;

use super::Sections;
use super::playhead::{Anchor, played_fraction};

/// The list. It reads the queue section, so a playhead tick repaints only the
/// active row's progress.
#[component]
pub fn QueueList(sections: Sections) -> Element {
    let queue = sections.queue.read();
    let Some(queue) = queue.as_ref() else {
        return rsx! { div { class: "queue-list" } };
    };
    let active = queue.active;
    rsx! {
        div { class: "queue-list",
            for (index, row) in queue.rows.iter().enumerate() {
                QueueRow {
                    key: "{index}",
                    row: row.clone(),
                    is_active: row_is_active(row, active),
                    sections,
                }
            }
        }
    }
}

/// Whether `row` carries the active marker: an item whose slot is the queue's
/// active slot.
fn row_is_active(row: &GuiQueueRow, active: Option<u64>) -> bool {
    match row {
        GuiQueueRow::Item { slot, .. } => Some(*slot) == active,
        GuiQueueRow::Heading { .. } | GuiQueueRow::Spacer => false,
    }
}

#[derive(Props, Clone, PartialEq)]
struct QueueRowProps {
    row: GuiQueueRow,
    is_active: bool,
    sections: Sections,
}

/// One row of the list. Headings and spacers paint their text or a gap.
#[component]
fn QueueRow(props: QueueRowProps) -> Element {
    let sections = props.sections;
    match props.row {
        GuiQueueRow::Item {
            ordinal,
            primary,
            secondary,
            duration,
            striped,
            ..
        } => {
            let class = row_class(striped, props.is_active);
            rsx! {
                div { class: "{class}",
                    span { class: "row-ordinal mono", "{ordinal:02}" }
                    div { class: "row-text",
                        div { class: "row-title", "{primary}" }
                        if let Some(secondary) = secondary {
                            div { class: "row-secondary", "{secondary}" }
                        }
                    }
                    if let Some(duration) = duration {
                        span { class: "row-duration mono", "{duration}" }
                    }
                    if props.is_active {
                        RowProgress { sections }
                    }
                }
            }
        }
        GuiQueueRow::Heading { text } => rsx! {
            div { class: "row-heading", "{text}" }
        },
        GuiQueueRow::Spacer => rsx! {
            div { class: "row-spacer" }
        },
    }
}

/// The active row's progress bar. It reads the position, so a playhead tick
/// repaints only this bar.
#[component]
fn RowProgress(sections: Sections) -> Element {
    let fraction = {
        let anchor = sections.anchor.read();
        let runtime = anchor.as_ref().map_or(0.0, Anchor::runtime_secs);
        played_fraction(*sections.position.read(), runtime)
    };
    rsx! {
        div { class: "row-progress",
            div { class: "row-progress-fill", style: "width: {fraction * 100.0}%" }
        }
    }
}

/// The row's class names: its zebra stripe and its active marker.
fn row_class(striped: bool, is_active: bool) -> String {
    let mut class = String::from("queue-row");
    if striped {
        class.push_str(" striped");
    }
    if is_active {
        class.push_str(" active");
    }
    class
}
