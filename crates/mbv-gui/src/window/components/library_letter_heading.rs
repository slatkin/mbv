//! The letter heading of an alphabetical Library list
//! (`replace-tauri-with-blitz` task 7.2, the port of
//! `LibraryLetterHeading.svelte`; add-gui-library-panel design D15): the one
//! heading the Emby and Audiobooks lists paint between letter groups. `attr`
//! names the destination's own hook.

use dioxus_native::prelude::*;

use crate::window::components::library_attr::library_attr;

/// The props of the letter heading (the `LibraryLetterHeading.svelte` props,
/// in order).
#[derive(Props, Debug, Clone, PartialEq)]
pub struct LibraryLetterHeadingProps {
    /// The destination's hook prefix, as in `emby` or `abs`.
    pub attr: &'static str,
    /// The heading's letter.
    pub letter: String,
    /// The heading's painted height, in pixels.
    pub height: f32,
}

/// The letter heading (the Svelte markup, character for character).
#[component]
pub fn LibraryLetterHeading(props: LibraryLetterHeadingProps) -> Element {
    let letter_attrs = vec![Attribute::new(
        library_attr(props.attr, "letter"),
        props.letter.clone(),
        None,
        false,
    )];

    rsx! {
        div {
            class: "flex items-end font-mono",
            style: "height: {props.height}px; padding: 0 8px 2px; font-size: 12px; color: var(--canvas-type-label);",
            ..letter_attrs,
            "{props.letter}"
        }
    }
}
