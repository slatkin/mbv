//! The Library panel's collection dropdown (gui-collection-dropdown design
//! D6): one component for every destination that has collections — Music groups,
//! Podcast shows, Feeds subscriptions, and an Emby library's feed-group folders
//! — so the four cannot drift. It supplies the collection frame's All row,
//! entries, and filter to the shared [`LibraryDropdown`] presentation; the
//! core owns the selection and the window owns the open fact and filter text
//! (design D7).
//!
//! The component sits in the list header on the title's line (design D6a), so
//! the panel paints no selector row between the window's global header and
//! its list. The
//! trigger is an outlined box in the accent with a trailing chevron; the open
//! popover paints the artboard's filter strip, All row, and checkbox rows under
//! it, over the list.

use dioxus_native::prelude::*;

use mbv::GuiCollectionNoun;

use crate::bridge::frame::library::{
    ALL_KEY, GuiCollectionEntry, GuiCollections, GuiCollectionsFrame as _,
};
use crate::window::components::library_dropdown::{
    LibraryDropdown, LibraryDropdownMarker, LibraryDropdownRow,
};
use crate::window::library::list::filter_match;
use crate::window::library::state::LibraryState;

/// The props of the collection dropdown: the destination's carried collections
/// frame alone (design D6), so a panel repaint never clones the list's rows for
/// the dropdown.
#[derive(Props, Debug, Clone, PartialEq)]
pub struct LibraryCollectionDropdownProps {
    /// The carried collections frame: the destination the commands name, All,
    /// every collection in catalog order with its checked flag, and the noun
    /// the trigger reads.
    pub collections: GuiCollections,
}

/// The entries the open dropdown paints under `filter` (design D7): the
/// collections whose name matches by the Library filter's rule.
fn matching_entries<'a>(
    collections: &'a GuiCollections,
    filter: &str,
) -> Vec<&'a GuiCollectionEntry> {
    collections
        .entries
        .iter()
        .filter(|entry| filter_match(&entry.name, filter))
        .collect()
}

/// The filter strip's placeholder (design D6): the destination's noun in the
/// singular, as the artboard's "Find a category" names the frame's
/// "categories".
fn placeholder_for(noun: GuiCollectionNoun) -> &'static str {
    match noun {
        GuiCollectionNoun::Groups => "Find a group",
        GuiCollectionNoun::Shows => "Find a show",
        GuiCollectionNoun::Subscriptions => "Find a subscription",
        GuiCollectionNoun::Folders => "Find a folder",
    }
}

/// One collection row is the shared dropdown's checkbox row, so this
/// destination adds no paint of its own.
fn rows(collections: &GuiCollections, filter: &str) -> Vec<LibraryDropdownRow> {
    let mut rows = vec![LibraryDropdownRow {
        key: ALL_KEY.to_owned(),
        label: "All".to_owned(),
        selected: collections.all,
    }];
    rows.extend(
        matching_entries(collections, filter)
            .into_iter()
            .map(|entry| LibraryDropdownRow {
                key: entry.key.clone(),
                label: entry.name.clone(),
                selected: entry.checked,
            }),
    );
    rows
}

/// The collection dropdown: the shared trigger and popover over one carried
/// collections frame.
#[component]
pub fn LibraryCollectionDropdown(props: LibraryCollectionDropdownProps) -> Element {
    let library = consume_context::<LibraryState>();
    let collections = props.collections;
    let ui = library.ui.cloned();
    let open = ui.collections.is_some();
    let filter = ui.collections.clone().unwrap_or_default();
    let label = collections.label();
    let placeholder = placeholder_for(collections.noun);
    let entries = if open {
        rows(&collections, &filter)
    } else {
        Vec::new()
    };

    let toggle_library = library.clone();
    let on_toggle = Callback::new(move |()| toggle_library.toggle_collections_dropdown());
    let inside_library = library.clone();
    let on_inside_press = Callback::new(move |()| inside_library.close_library_row_menu());
    let filter_library = library.clone();
    let on_filter = Callback::new(move |text: String| filter_library.set_collections_filter(text));
    let target = collections.target.clone();
    let on_select = Callback::new(move |key: String| {
        library.press_collection_target(&target, &key);
    });

    rsx! {
        LibraryDropdown {
            label,
            open,
            rows: entries,
            marker: LibraryDropdownMarker::Check,
            aria_label: "Collections",
            trigger_attr: "data-library-collections-trigger",
            menu_attr: "data-library-collections-menu",
            row_attr: "data-library-collections-row",
            mark_attr: "data-library-collections-checkbox",
            first_row_attr: Some("data-library-collections-all"),
            first_row_gap: true,
            filter: open.then_some(filter),
            filter_placeholder: Some(placeholder),
            filter_attr: Some("data-library-collections-filter"),
            on_toggle,
            on_select,
            on_filter,
            on_inside_press,
        }
    }
}

#[cfg(test)]
mod tests;
