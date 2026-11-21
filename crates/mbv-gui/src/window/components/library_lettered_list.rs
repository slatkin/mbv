//! The lettered destinations' one list (`gui-tech-debt-simplification` design
//! D4): the plain Emby library, Audiobooks, Podcasts, and Feeds paint their
//! carried rows under their letter headings with the A–Z rail beside them,
//! and a date sort's rows flat with their date gutter and no rail
//! (gui-flat-list-sort design D8). The list, its header, its notice lines, and
//! its rows are one body over [`LetteredDestination`]: the destination names
//! its attribute word, its art scope, its pointer list, its header facts, its
//! notice texts, and its row painter, so this module holds the wiring and each
//! destination keeps only its data.
//!
//! The rows paint at the destination's fixed [`LetteredDestination::ROW_HEIGHT`]
//! over native scroll, and only the rows in view paint, so a long list stays
//! fast. The list keeps no cursor: the keys act on the row the pointer marks
//! (`gui-pointer-target-interaction` D2); the scroll offset and the viewport
//! are the destination's own scroll store (design D9). A press selects the row
//! the Hero shows, and a double-click plays it, through the destination's
//! command methods.

use dioxus_native::prelude::*;

use crate::window::components::library_attr::library_attr;
use crate::window::components::library_letter_rail::LibraryLetterRail;
use crate::window::components::library_list_header::{LibraryListHeader, LibraryListHeading};
use crate::window::components::library_virtual_list::library_virtual_list;
use crate::window::components::list_frame_notices::{ListFrameNoticeLine, ListFrameNotices};
use crate::window::library::lettered::{LetteredDestination, LetteredItemRow};
use crate::window::library::list::{LetteredRow, carries_letters, lettered_rail};
use crate::window::library::state::LibraryState;

/// The lettered list for destination `D` (design D4): the destination's header,
/// its load, failure, and empty lines, its rows over native scroll, and its
/// letter rail.
///
/// The destination is the component's type parameter, so the list paints the
/// same body for all four: `LibraryLetteredList::<EmbyList> {}`.
#[component]
pub fn LibraryLetteredList<D: LetteredDestination>() -> Element {
    let library = consume_context::<LibraryState>();

    // The carried content and the core's selection: the clones subscribe the
    // list to the frames that carry them, as the Music list's read does; the
    // row model borrows the frame from the section store's own storage
    // through the memo's revision key.
    let carried = library.sections.library_list.cloned();
    let detail_section = library.sections.library_detail.cloned();
    let frame = D::list(carried.as_deref());
    let detail = D::detail(detail_section.as_deref());

    // The interaction state and the scroll store this list paints (design D1,
    // D9).
    let ui = D::ui(&library).cloned();
    let scroll = D::scroll(&library).cloned();

    // The header's facts (design D4): the mark that names the destination, the
    // selection label, the collection selector, the sort control, and the
    // filter field's texts, each as the destination carries it.
    let header = D::header(frame);

    // The load, failure, and empty lines this list paints: the destination's
    // own line attributes and texts, present only while their fact holds.
    let load_facts = frame.map(|frame| D::load_facts(frame));

    // The painted rows (design D4): the carried rows with their letter
    // headings, narrowed by the filter's text.
    let rows = D::painted_rows(&library);

    // The rail's A–Z letters over the painted rows, and whether the rows carry
    // any letter at all (gui-flat-list-sort design D8): a name sort paints the
    // rail, and a date sort paints none.
    let rail = lettered_rail(&rows);
    let paints_rail = carries_letters(&rows);

    let selected = detail.and_then(|detail| D::selected(detail));

    // The list's two DOM facts report to the scroll store (design D9), the rows
    // in view report to the core for their row art fetches (gui-delineated-rows
    // design D7), and the rows paint through the destination's own painter.
    let measure_library = library.clone();
    let scroll_library = library.clone();
    let art_library = library.clone();
    let paint_library = library.clone();
    let paint_selected = selected.map(str::to_owned);
    let virtual_list = library_virtual_list(
        rows.as_ref(),
        scroll,
        D::ATTR,
        D::POINTER_LIST,
        D::PADDING,
        Callback::new(move |height: f32| measure_library.measure_lettered_list::<D>(height)),
        Callback::new(move |top: f32| scroll_library.scroll_lettered_list::<D>(top)),
        Some(Callback::new(move |ids: Vec<String>| {
            art_library.report_rows_in_view(D::ART_SCOPE, ids);
        })),
        move |row: &LetteredRow<LetteredItemRow<D::Row>>| {
            D::paint_row(row, &paint_library, paint_selected.as_deref())
        },
    );

    // The rail's press scrolls the list to the letter's first row (design D7).
    let jump_library = library.clone();
    let filter_library = library.clone();
    let accept_library = library.clone();

    // The list column's own hook, which the destination's tests read.
    let column_attrs = vec![Attribute::new(
        library_attr(D::ATTR, "list-column"),
        "",
        None,
        false,
    )];

    rsx! {
        div {
            class: "flex min-h-0 min-w-0 flex-1 flex-col",
            ..column_attrs,
            // The Music list's header (design D15): the destination's mark
            // names it (gui-library-header-service-marks design D5), its
            // selection label and collection selector are its own, and the `/`
            // filter field narrows it.
            div {
                class: "flex flex-none flex-col",
                LibraryListHeader {
                    heading: LibraryListHeading::Mark(header.mark),
                    label: header.label.clone(),
                    label_attr: header.label_attr,
                    collections: header.collections.clone(),
                    sort: header.sort.clone(),
                    filter: ui.filter.clone(),
                    placeholder: header.placeholder.clone(),
                    aria_label: header.aria_label.to_owned(),
                    on_input: move |value: String| {
                        filter_library.set_lettered_filter::<D>(value);
                    },
                    on_accept: move |()| accept_library.accept_destination_filter(),
                }
            }
            if let Some(facts) = load_facts {
                // The active list has not landed yet, its fetch failed, or it
                // holds no item: one line per state, and the empty line only
                // over a landed row set.
                ListFrameNotices {
                    loading: facts
                        .loading
                        .then(|| ListFrameNoticeLine::loading(D::NOTICES.loading_attr)),
                    failed: facts.failed.then(|| {
                        ListFrameNoticeLine::new(D::NOTICES.failed_attr, D::NOTICES.failed_text)
                    }),
                    empty: facts.empty.then(|| {
                        ListFrameNoticeLine::new(D::NOTICES.empty_attr, D::NOTICES.empty_text)
                    }),
                }
            }
            div {
                class: "flex min-h-0 flex-1",
                style: "overflow: hidden;",
                {virtual_list}
                if paints_rail {
                    LibraryLetterRail {
                        letters: rail,
                        jump: Some(Callback::new(move |letter: char| {
                            jump_library.jump_lettered_to_letter::<D>(letter);
                        })),
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests;
