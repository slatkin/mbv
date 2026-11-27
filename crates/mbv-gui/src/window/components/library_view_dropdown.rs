//! The Emby destination's single-choice view dropdown (emby-favourites
//! design D5): the views `GuiLibraryView::choices` names for the dropdown's
//! destination — All, New, Upcoming, and Favourites on Shows, All and
//! Favourites elsewhere. It supplies the labelled views and the radio marker
//! to the shared [`LibraryDropdown`] presentation; no collection code rides
//! here and no filter or date-order control does. A press replaces the
//! active view and closes the popover.

use dioxus_native::prelude::*;

use crate::gui_core::{GuiDestinationKind, GuiLibraryView};

use crate::window::components::library_dropdown::{
    LibraryDropdown, LibraryDropdownMarker, LibraryDropdownRow,
};
use crate::window::library::state::LibraryState;

/// The props of the view dropdown: the active view the core frame carries,
/// and the destination whose choices the dropdown names.
#[derive(Props, Debug, Clone, PartialEq)]
pub struct LibraryViewDropdownProps {
    /// The active view, one of the destination's choices.
    pub view: GuiLibraryView,
    /// The destination the view is a view of: its choices are the rows the
    /// popover paints.
    pub destination: GuiDestinationKind,
}

/// The rows the popover paints, in the user-visible order.
fn view_rows(active: GuiLibraryView, destination: GuiDestinationKind) -> Vec<LibraryDropdownRow> {
    GuiLibraryView::choices(destination)
        .iter()
        .map(|view| LibraryDropdownRow {
            key: view.key().to_owned(),
            label: view.label().to_owned(),
            selected: *view == active,
        })
        .collect()
}

/// The trigger, the destination's radio rows, and the press that installs one
/// view.
#[component]
pub fn LibraryViewDropdown(props: LibraryViewDropdownProps) -> Element {
    let library = consume_context::<LibraryState>();
    let active = props.view;
    let destination = props.destination;
    let open = library.ui.cloned().library_view.is_open();
    let lib_idx = destination.library_index();

    let toggle_library = library.clone();
    let on_toggle = Callback::new(move |()| toggle_library.toggle_library_view_dropdown());
    let inside_library = library.clone();
    let on_inside_press = Callback::new(move |()| inside_library.close_library_row_menu());
    let on_filter = Callback::new(move |_: String| {});
    let select_library = library.clone();
    let on_select = Callback::new(move |key: String| {
        let Some(view) = GuiLibraryView::from_key(&key) else {
            return;
        };
        if let Some(lib_idx) = lib_idx {
            select_library.select_library_view(lib_idx, view);
        }
        select_library.close_library_view_dropdown();
    });

    rsx! {
        LibraryDropdown {
            label: active.label().to_owned(),
            open,
            rows: view_rows(active, destination),
            marker: LibraryDropdownMarker::Radio,
            aria_label: "View",
            trigger_attr: "data-library-view-trigger",
            menu_attr: "data-library-view-menu",
            row_attr: "data-library-view-row",
            mark_attr: "data-library-view-mark",
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
