//! A Library list's one-line notice (`replace-tauri-with-blitz` task 7.2,
//! the port of `LibraryListNotice.svelte`; add-gui-library-panel design D5,
//! D7, D14): the placeholder a list paints above its rows while it loads,
//! when its fetch failed, and when it holds nothing. Every destination
//! paints it in the same look, so the three states cannot drift between
//! tabs.
//!
//! `attr` names the destination's own attribute
//! (`data-library-shows-empty`), so a destination's tests keep their hook.

use dioxus_native::prelude::*;

/// The props of the notice (the `LibraryListNotice.svelte` props, in order).
#[derive(Props, Debug, Clone, PartialEq)]
pub struct LibraryListNoticeProps {
    /// The line's text.
    pub text: String,
    /// The destination's own attribute name, which its tests read.
    pub attr: &'static str,
}

/// The notice (the Svelte markup, character for character).
#[component]
pub fn LibraryListNotice(props: LibraryListNoticeProps) -> Element {
    let attrs = vec![Attribute::new(props.attr, "", None, false)];

    rsx! {
        div {
            class: "flex flex-none items-center",
            style: "height: 36px; padding: 0 16px; font-size: 14px; color: var(--canvas-muted);",
            ..attrs,
            "{props.text}"
        }
    }
}
