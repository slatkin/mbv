//! The Library list's header (`replace-tauri-with-blitz` task 7.2, the port
//! of `LibraryListHeader`; add-gui-library-panel design D7, D14): the
//! canvas's list title, the destination's collection dropdown on the title's
//! line, and the `/` filter field under both. Every destination paints this one
//! header, so the column cannot drift between tabs: a destination passes its
//! title, its collections frame when it has one, a selection label when it
//! wants one, and its own filter state.
//!
//! The dropdown rides the title line's right end (gui-collection-dropdown
//! design D6a, user override): it paints at the line's right end, so the panel
//! paints no selector row above the list, and the selection label is not
//! painted while the trigger names the selection itself. The label paints only
//! where it is non-empty: a plain Emby library and the Shows destination pass
//! an empty one, because the UI is not verbose.
//!
//! `title_attr` and `label_attr` name a destination's own attributes, so a
//! destination's tests keep a per-tab hook beside the shared ones.

use dioxus_native::prelude::*;

use mbv::GuiCollections;

use crate::window::components::library_collection_dropdown::LibraryCollectionDropdown;
use crate::window::components::library_filter_field::LibraryFilterField;

/// The props of the list header (the `LibraryListHeader` props, in
/// order).
#[derive(Props, Debug, Clone, PartialEq)]
pub struct LibraryListHeaderProps {
    /// The list's title, as the canvas names the column.
    pub title: String,
    /// The destination's own title attribute, when it has one.
    #[props(default)]
    pub title_attr: Option<&'static str>,
    /// The carried selection's label, painted on the title's line only when
    /// it is non-empty and the destination paints no collection dropdown.
    pub label: String,
    /// The destination's own label attribute, when it has one.
    #[props(default)]
    pub label_attr: Option<&'static str>,
    /// The destination's collection selector, when it has one (design D6a):
    /// the dropdown paints on the title's line after the title, and the label
    /// is not painted, because the trigger names the selection.
    #[props(default)]
    pub collections: Option<GuiCollections>,
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

/// The list header.
#[component]
pub fn LibraryListHeader(props: LibraryListHeaderProps) -> Element {
    let LibraryListHeaderProps {
        title,
        title_attr,
        label,
        label_attr,
        collections,
        filter,
        placeholder,
        aria_label,
        on_input,
        on_accept,
    } = props;
    // The destination's own title and label attributes, when it has one.
    let title_attrs = title_attr.map_or(Vec::new(), |name| {
        vec![Attribute::new(name, "", None, false)]
    });
    let label_attrs = label_attr.map_or(Vec::new(), |name| {
        vec![Attribute::new(name, "", None, false)]
    });

    rsx! {
        div {
            class: "flex flex-none flex-col",
            style: "padding: 14px 16px 10px; gap: 10px;",
            "data-library-list-header": "",
            div {
                class: "flex items-center justify-between",
                style: "gap: 12px;",
                span {
                    style: "font-size: 14px; font-weight: 600; color: var(--canvas-heading);",
                    "data-library-list-title": "",
                    ..title_attrs,
                    "{title}"
                }
                if let Some(collections) = collections {
                    // The dropdown paints the title line's right end (design
                    // D6a): the trigger after the title, at the line's end.
                    LibraryCollectionDropdown { collections }
                } else if !label.is_empty() {
                    span {
                        class: "font-mono",
                        style: "font-size: 12px; color: var(--canvas-muted);",
                        "data-library-selection-label": "",
                        ..label_attrs,
                        "{label}"
                    }
                }
            }
            LibraryFilterField {
                value: filter,
                placeholder,
                aria_label,
                on_input,
                on_accept,
            }
        }
    }
}

#[cfg(test)]
mod tests;
