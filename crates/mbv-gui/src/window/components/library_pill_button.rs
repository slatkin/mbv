//! The one pill button every Library selector paints
//! (`replace-tauri-with-blitz` task 7.4, the port of
//! `LibraryPillButton.svelte`; add-gui-library-panel design D6, D15): the
//! top pill rows and the Shows Hero's season pills. A destination supplies
//! the pill's name, its selected and focused state, its own attributes, and
//! the press; it never restates the pill's look, so the two selectors cannot
//! drift.

use dioxus_native::prelude::*;

use crate::bridge::frame::theme::GuiRole;
use crate::window::theme::role_var;

/// The props of the pill button (the `LibraryPillButton.svelte` props, in
/// order).
#[derive(Props, Clone, PartialEq)]
pub struct LibraryPillButtonProps {
    /// The pill's label.
    pub name: String,
    /// Whether the core carries the pill as the selected one.
    pub selected: bool,
    /// Whether the keys' focus holds the pill, which paints the accent
    /// outline (design D6).
    #[props(default)]
    pub focused: bool,
    /// The destination's own hook attributes for this pill.
    #[props(default)]
    pub attrs: Vec<Attribute>,
    /// A press on the pill.
    pub onpress: Callback<()>,
}

impl std::fmt::Debug for LibraryPillButtonProps {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LibraryPillButtonProps")
            .field("name", &self.name)
            .field("selected", &self.selected)
            .field("focused", &self.focused)
            .field("attrs", &self.attrs.len())
            .finish_non_exhaustive()
    }
}

/// The one pill button (the Svelte markup, character for character): the
/// selected pill fills in the source tone, the focused one paints the accent
/// outline.
#[component]
pub fn LibraryPillButton(props: LibraryPillButtonProps) -> Element {
    let (border, background, color, weight) = if props.selected {
        (
            "1px solid var(--canvas-source)",
            "var(--canvas-source)",
            "var(--canvas-ink)",
            600,
        )
    } else {
        (
            "1px solid var(--canvas-border)",
            "none",
            "var(--canvas-text)",
            400,
        )
    };
    let outline = if props.focused {
        format!("2px solid {}", role_var(GuiRole::AccentActive))
    } else {
        "none".to_owned()
    };
    let onpress = props.onpress;
    let selected = props.selected;

    rsx! {
        button {
            "type": "button",
            class: "flex flex-none items-center whitespace-nowrap",
            style: "height: 32px; gap: 5px; padding: 0 12px; border-radius: 16px; font-size: 14px; font-weight: {weight}; border: {border}; background: {background}; color: {color}; outline: {outline}; outline-offset: 1px;",
            "aria-pressed": "{selected}",
            onclick: move |_| onpress.call(()),
            ..props.attrs,
            "{props.name}"
        }
    }
}
