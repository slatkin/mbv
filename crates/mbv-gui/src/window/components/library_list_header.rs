//! The Library list's header (`replace-tauri-with-blitz` task 7.2, the port
//! of `LibraryListHeader.svelte`; add-gui-library-panel design D7, D14): the
//! canvas's list title beside the carried selection's label, and the `/`
//! filter field under both. Every destination paints this one header, so the
//! column cannot drift between tabs: a destination passes its title, the
//! label its core projection derived, and its own filter state.
//!
//! `title_attr` and `label_attr` name a destination's own attributes, so a
//! destination's tests keep a per-tab hook beside the shared ones.

use dioxus_native::prelude::*;

use crate::window::components::library_filter_field::LibraryFilterField;

/// The props of the list header (the `LibraryListHeader.svelte` props, in
/// order).
#[derive(Props, Debug, Clone, PartialEq)]
pub struct LibraryListHeaderProps {
    /// The list's title, as the canvas names the column.
    pub title: String,
    /// The destination's own title attribute, when it has one.
    #[props(default)]
    pub title_attr: Option<&'static str>,
    /// The carried selection's label, painted on the title's line.
    pub label: String,
    /// The destination's own label attribute, when it has one.
    #[props(default)]
    pub label_attr: Option<&'static str>,
    /// The filter text the field paints.
    pub filter: String,
    /// The field's placeholder, which names the selection it narrows.
    pub placeholder: String,
    /// The field's accessible name.
    pub aria_label: String,
    /// A keystroke in the field.
    pub on_input: Callback<String>,
    /// The field's accept, as Enter does.
    pub on_accept: Callback<()>,
}

/// The list header (the Svelte markup, character for character).
#[component]
pub fn LibraryListHeader(props: LibraryListHeaderProps) -> Element {
    // The destination's own title and label attributes, when it has one.
    let title_attrs = props.title_attr.map_or(Vec::new(), |name| {
        vec![Attribute::new(name, "", None, false)]
    });
    let label_attrs = props.label_attr.map_or(Vec::new(), |name| {
        vec![Attribute::new(name, "", None, false)]
    });

    rsx! {
        div {
            class: "flex flex-none flex-col",
            style: "padding: 14px 16px 10px; gap: 10px;",
            div {
                class: "flex items-baseline justify-between",
                style: "gap: 12px;",
                span {
                    style: "font-size: 14px; font-weight: 600; color: var(--canvas-heading);",
                    "data-library-list-title": "",
                    ..title_attrs,
                    "{props.title}"
                }
                span {
                    class: "font-mono",
                    style: "font-size: 12px; color: var(--canvas-muted);",
                    "data-library-selection-label": "",
                    ..label_attrs,
                    "{props.label}"
                }
            }
            LibraryFilterField {
                value: props.filter,
                placeholder: props.placeholder,
                aria_label: props.aria_label,
                on_input: props.on_input,
                on_accept: props.on_accept,
            }
        }
    }
}
