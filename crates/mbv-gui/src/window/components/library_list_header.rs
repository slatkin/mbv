//! The Library list's header (`replace-tauri-with-blitz` task 7.2, the port
//! of `LibraryListHeader`; add-gui-library-panel design D7, D14): the
//! canvas's list heading, the destination's collection dropdown on the
//! heading's line, and the `/` filter field under both. Every destination
//! paints this one header, so the column cannot drift between tabs: a
//! destination passes its heading, its collections frame when it has one, a
//! selection label when it wants one, and its own filter state.
//!
//! The heading is a Service mark or Home's text title (gui-library-header-
//! service-marks design D5): a Service destination names itself by its mark,
//! because the tab bar already names the destination in words, and Home keeps
//! its title. Either paints at the left end of the heading line.
//!
//! The dropdown rides the heading line's right end (gui-collection-dropdown
//! design D6a, user override): it paints at the line's right end, so the panel
//! paints no selector row above the list, and the selection label is not
//! painted while the trigger names the selection itself. The label paints only
//! where it is non-empty: a plain Emby library and the Shows destination pass
//! an empty one, because the UI is not verbose.
//!
//! `label_attr` names a destination's own attribute, so a destination's tests
//! keep a per-tab hook beside the shared ones.

use dioxus_native::prelude::*;

use mbv::{GuiCollections, GuiShowsView};

use crate::window::components::library_collection_dropdown::LibraryCollectionDropdown;
use crate::window::components::library_filter_field::LibraryFilterField;
use crate::window::components::library_shows_view_dropdown::LibraryShowsViewDropdown;
use crate::window::service_mark::{ServiceMark, ServiceMarkIcon};

/// What a destination paints at the left end of its heading line (design
/// D5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LibraryListHeading {
    /// A Service destination: its Service mark, and no text.
    Mark(ServiceMark),
    /// Home: its text title.
    Title(String),
}

/// The props of the list header (the `LibraryListHeader` props, in
/// order).
#[derive(Props, Debug, Clone, PartialEq)]
pub struct LibraryListHeaderProps {
    /// The list's heading, as the destination names itself.
    pub heading: LibraryListHeading,
    /// The carried selection's label, painted on the heading's line only when
    /// it is non-empty and the destination paints no collection dropdown.
    pub label: String,
    /// The destination's own label attribute, when it has one.
    #[props(default)]
    pub label_attr: Option<&'static str>,
    /// The destination's collection selector, when it has one (design D6a):
    /// the dropdown paints on the heading's line after the heading, and the
    /// label is not painted, because the trigger names the selection.
    #[props(default)]
    pub collections: Option<GuiCollections>,
    /// The Shows destination's single-choice view, when it paints one
    /// (gui-collection-dropdown design D8): the dropdown goes on the same
    /// heading line and is not a collection selector.
    #[props(default)]
    pub view: Option<GuiShowsView>,
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
        heading,
        label,
        label_attr,
        collections,
        view,
        filter,
        placeholder,
        aria_label,
        on_input,
        on_accept,
    } = props;
    // The destination's own label attribute, when it has one.
    let label_attrs = label_attr.map_or(Vec::new(), |name| {
        vec![Attribute::new(name, "", None, false)]
    });

    rsx! {
        div {
            class: "flex flex-none flex-col",
            style: "padding: 14px 16px 10px; gap: 10px;",
            "data-library-list-header": "",
            div {
                class: "flex flex-none items-center justify-between",
                // The line is as tall as the dropdown trigger (28px) whether or
                // not the destination paints one, so the filter field under it
                // sits at one height on every tab.
                style: "gap: 12px; min-height: 28px;",
                match heading {
                    LibraryListHeading::Mark(mark) => rsx! { ServiceMarkIcon { mark } },
                    LibraryListHeading::Title(title) => rsx! {
                        span {
                            style: "font-size: 14px; font-weight: 600; color: var(--canvas-heading);",
                            "data-library-list-title": "",
                            "{title}"
                        }
                    },
                }
                if let Some(collections) = collections {
                    // The dropdown paints the heading line's right end (design
                    // D6a): the trigger after the heading, at the line's end.
                    LibraryCollectionDropdown { collections }
                } else if let Some(view) = view {
                    LibraryShowsViewDropdown { view }
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
