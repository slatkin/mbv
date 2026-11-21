//! The Shows destination's single-choice view dropdown
//! (gui-collection-dropdown design D8): All, New, or Upcoming. It supplies the
//! three labelled views and the radio marker to the shared
//! [`LibraryDropdown`] presentation; no collection code names Shows and no
//! filter or date-order control rides here. A press replaces the active view
//! and closes the popover.

use dioxus_native::prelude::*;

use mbv::GuiShowsView;

use crate::window::components::library_dropdown::{
    LibraryDropdown, LibraryDropdownMarker, LibraryDropdownRow,
};
use crate::window::library::parts::library_shows_list;
use crate::window::library::state::LibraryState;

/// The props of the Shows view dropdown: the active view the core frame
/// carries, and the library index the command names.
#[derive(Props, Debug, Clone, PartialEq)]
pub struct LibraryShowsViewDropdownProps {
    /// The active view, one of All, New, or Upcoming.
    pub view: GuiShowsView,
}

/// The rows the popover paints, in the user-visible order.
fn view_rows(active: GuiShowsView) -> Vec<LibraryDropdownRow> {
    GuiShowsView::ALL_VIEWS
        .into_iter()
        .map(|view| LibraryDropdownRow {
            key: view.key().to_owned(),
            label: view.label().to_owned(),
            selected: view == active,
        })
        .collect()
}

/// The trigger, the three radio rows, and the press that installs one view.
#[component]
pub fn LibraryShowsViewDropdown(props: LibraryShowsViewDropdownProps) -> Element {
    let library = consume_context::<LibraryState>();
    let active = props.view;
    let open = library.ui.cloned().shows_view_open.is_open();
    let lib_idx = library
        .sections
        .library_list
        .peek()
        .as_deref()
        .and_then(|list| library_shows_list(Some(list)))
        .map(|shows| shows.lib_idx);

    let toggle_library = library.clone();
    let on_toggle = Callback::new(move |()| toggle_library.toggle_shows_view_dropdown());
    let inside_library = library.clone();
    let on_inside_press = Callback::new(move |()| inside_library.close_library_row_menu());
    let on_filter = Callback::new(move |_: String| {});
    let select_library = library.clone();
    let on_select = Callback::new(move |key: String| {
        let Some(view) = GuiShowsView::from_key(&key) else {
            return;
        };
        if let Some(lib_idx) = lib_idx {
            select_library.select_shows_view(lib_idx, view);
        }
        select_library.close_shows_view_dropdown();
    });

    rsx! {
        LibraryDropdown {
            label: active.label().to_owned(),
            open,
            rows: view_rows(active),
            marker: LibraryDropdownMarker::Radio,
            aria_label: "Shows view",
            trigger_attr: "data-library-shows-view-trigger",
            menu_attr: "data-library-shows-view-menu",
            row_attr: "data-library-shows-view-row",
            mark_attr: "data-library-shows-view-mark",
            filter: None,
            on_toggle,
            on_select,
            on_filter,
            on_inside_press,
        }
    }
}

#[cfg(test)]
mod tests;
