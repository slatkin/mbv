//! A Library list frame's notice lines (`simplify` of `replace-tauri-with-blitz`
//! task 7.2, issue #1011): the load line, the failure line, and the empty line
//! a destination paints above its rows. Every destination holds the same three
//! kinds of line, each present only while its state holds, so one component
//! paints them and their order and look cannot drift between tabs; each
//! destination names its own line attributes and its own failure and empty
//! text, so its tests keep their hooks.
//!
//! The look stays `LibraryListNotice`'s alone: this component only decides
//! which of the three lines the frame holds.

use dioxus_native::prelude::*;

use crate::window::components::library_list_notice::LibraryListNotice;

/// One notice line: the destination's own attribute and the line's text.
#[derive(Debug, Clone, PartialEq)]
pub struct ListFrameNoticeLine {
    /// The destination's own attribute name, which its tests read.
    pub attr: &'static str,
    /// The line's text.
    pub text: String,
}

impl ListFrameNoticeLine {
    /// The load line every destination paints while its frame loads: the same
    /// text everywhere, so only the attribute is the destination's.
    #[must_use]
    pub fn loading(attr: &'static str) -> Self {
        Self {
            attr,
            text: "Loading…".to_owned(),
        }
    }

    /// A line with its own text: a destination's failure or empty line.
    #[must_use]
    pub fn new(attr: &'static str, text: impl Into<String>) -> Self {
        Self {
            attr,
            text: text.into(),
        }
    }
}

/// The props of the frame's notices: each line, present only while its state
/// holds.
#[derive(Props, Debug, Clone, PartialEq)]
pub struct ListFrameNoticesProps {
    /// The load line, while the frame loads.
    #[props(default)]
    pub loading: Option<ListFrameNoticeLine>,
    /// The failure line, while the frame's fetch failed.
    #[props(default)]
    pub failed: Option<ListFrameNoticeLine>,
    /// The empty line, while the frame landed holding no row.
    #[props(default)]
    pub empty: Option<ListFrameNoticeLine>,
}

/// The frame's notices: the load, failure, and empty lines in the order the
/// lists paint them.
#[component]
pub fn ListFrameNotices(props: ListFrameNoticesProps) -> Element {
    rsx! {
        if let Some(line) = props.loading.clone() {
            LibraryListNotice { text: line.text, attr: line.attr }
        }
        if let Some(line) = props.failed.clone() {
            LibraryListNotice { text: line.text, attr: line.attr }
        }
        if let Some(line) = props.empty.clone() {
            LibraryListNotice { text: line.text, attr: line.attr }
        }
    }
}
