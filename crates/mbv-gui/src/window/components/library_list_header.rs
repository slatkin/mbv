//! The Library list's header (`replace-tauri-with-blitz` task 7.2, the port
//! of `LibraryListHeader`; add-gui-library-panel design D7, D14): the
//! canvas's list heading, the destination's trailing control group on the
//! heading's line, and the `/` filter field under both. Every destination
//! paints this one header, so the column cannot drift between tabs: a
//! destination passes its heading, its collections frame when it has one, a
//! selection label when it wants one, its sort frame when its list has one, its
//! refresh and tree presses, and its own filter state.
//!
//! The heading is a Service mark or Home's text title (gui-library-header-
//! service-marks design D5): a Service destination names itself by its mark,
//! because the tab bar already names the destination in words, and Home keeps
//! its title. Either paints at the left end of the heading line.
//!
//! The heading line's right end holds one trailing control group
//! (gui-list-header-buttons design D1): the refresh button, the tree list's
//! Expand all and Collapse all buttons when the destination passes tree
//! actions, then the dropdown, the Shows view dropdown, or the selection
//! label, then the sort control. The group takes a 6 px inner gap in place of
//! the line's 12 px gap, so its controls read as one cluster.
//!
//! The dropdown rides the heading line's right end (gui-collection-dropdown
//! design D6a, user override): it paints there, so the panel paints no selector
//! row above the list, and the selection label is not painted while the trigger
//! names the selection itself. The label paints only where it is non-empty: a
//! plain Emby library and the Shows destination pass an empty one, because the
//! UI is not verbose.
//!
//! The sort control paints last in the group (gui-flat-list-sort design D7), so
//! it is the rightmost item and sits directly right of the collection dropdown
//! or the Shows view dropdown. The five flat lists pass their frame's sort and
//! every other destination paints none, so the placement has one owner here.
//!
//! `label_attr` names a destination's own attribute, so a destination's tests
//! keep a per-tab hook beside the shared ones.

use std::borrow::Cow;

use dioxus_native::prelude::*;

use crate::gui_core::{GuiCollections, GuiListSortFrame, GuiShowsView};

use crate::window::components::header_icon_button::{
    HEADER_ICON_SIZE, HEADER_ICON_STROKE_WIDTH, HeaderIconButton,
};
use crate::window::components::library_collection_dropdown::LibraryCollectionDropdown;
use crate::window::components::library_filter_field::LibraryFilterField;
use crate::window::components::library_shows_view_dropdown::LibraryShowsViewDropdown;
use crate::window::components::library_sort_control::LibrarySortControl;
use crate::window::icons::{ChevronsDownUp, ChevronsUpDown, RefreshCcw};
use crate::window::service_mark::{ServiceMark, ServiceMarkIcon};

/// What a destination paints at the left end of its heading line (design
/// D5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LibraryListHeading {
    /// A Service destination: its Service mark, and no text.
    Mark(ServiceMark),
    /// Home: its text title. The `Cow` keeps the common static literal
    /// allocation-free on every render.
    Title(Cow<'static, str>),
}

impl From<ServiceMark> for LibraryListHeading {
    fn from(mark: ServiceMark) -> Self {
        Self::Mark(mark)
    }
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
    /// The flat list's sort control, when its list has one (gui-flat-list-sort
    /// design D7): the control paints last in the heading line's trailing
    /// group, so it is the line's rightmost item.
    #[props(default)]
    pub sort: Option<GuiListSortFrame>,
    /// The list's refresh press (gui-list-header-buttons design D1): every
    /// destination paints one, left of its dropdown, label, or sort control.
    pub on_refresh: Callback<()>,
    /// Whether the destination's refresh is in flight (the frame's busy
    /// fact, design D7): the refresh glyph spins while it holds.
    #[props(default)]
    pub refreshing: bool,
    /// The tree list's Expand all and Collapse all presses
    /// (gui-list-header-buttons design D1); `None` on a flat list.
    #[props(default)]
    pub tree: Option<LibraryTreeActions>,
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

/// The tree list's two presses (gui-list-header-buttons design D1): the
/// header paints them only for a destination that passes them, so a flat list
/// paints neither button. Both change the window's open-node state and send no
/// command.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LibraryTreeActions {
    /// Open every parent row at the tree's top level.
    pub on_expand_all: Callback<()>,
    /// Close every open row at every level.
    pub on_collapse_all: Callback<()>,
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
        sort,
        on_refresh,
        refreshing,
        tree,
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
                // The trailing control group (gui-list-header-buttons design
                // D1): the refresh button, the tree buttons, the dropdown or
                // the selection label, then the sort control. Its 6 px gap
                // replaces the line's 12 px gap inside the group.
                div {
                    class: "flex flex-none items-center",
                    style: "gap: 6px;",
                    HeaderIconButton {
                        label: "Refresh",
                        hook: "data-library-refresh",
                        onpress: on_refresh,
                        // The refresh glyph spins while the destination's
                        // refresh is in flight (design D7).
                        RefreshCcw {
                            size: HEADER_ICON_SIZE,
                            stroke_width: HEADER_ICON_STROKE_WIDTH,
                            class: if refreshing { "animate-spin" } else { "" },
                        }
                    }
                    if let Some(tree) = tree {
                        HeaderIconButton {
                            label: "Expand all",
                            hook: "data-library-expand-all",
                            onpress: tree.on_expand_all,
                            ChevronsUpDown {
                                size: HEADER_ICON_SIZE,
                                stroke_width: HEADER_ICON_STROKE_WIDTH,
                            }
                        }
                        HeaderIconButton {
                            label: "Collapse all",
                            hook: "data-library-collapse-all",
                            onpress: tree.on_collapse_all,
                            ChevronsDownUp {
                                size: HEADER_ICON_SIZE,
                                stroke_width: HEADER_ICON_STROKE_WIDTH,
                            }
                        }
                    }
                    if let Some(collections) = collections {
                        // The dropdown paints the end of the trailing group
                        // (design D6a), left of the sort control when the list
                        // has one.
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
                    // The sort control closes the group (design D7): the last
                    // item, right of the dropdown or the view dropdown when
                    // there is one.
                    if let Some(sort) = sort {
                        LibrarySortControl { sort }
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
