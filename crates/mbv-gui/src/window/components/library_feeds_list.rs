//! The Feeds list (`replace-tauri-with-blitz` task 7.7, the port of
//! `LibraryFeedsList`; add-gui-library-panel design D12, row 11.1):
//! the checked collections' flat entry rows — the entry's title over its
//! subscription, and the entry date in the gutter — with no letter headings,
//! because the list is sorted by date, and so no letter rail.
//!
//! The rows paint at the fixed heights of [`crate::window::library::feeds_list`]
//! over native scroll, and only the rows in view paint, so a long Latest
//! merge stays fast. The list keeps no cursor: the keys act on the row the
//! pointer marks (`gui-pointer-target-interaction` D2); the scroll offset and
//! the viewport are the Feeds list's scroll store (design D9). A press selects
//! the entry the Hero shows, and a double-click plays it (design D12): the
//! keys' Enter plays the hovered row, and the press is the selection's half.

use dioxus_native::prelude::*;

use crate::bridge::frame::library::GuiCollectionsFrame;
use crate::window::components::library_item_row::{ItemRowPaint, paint_item_row};
use crate::window::components::library_list_header::LibraryListHeader;
use crate::window::components::library_virtual_list::library_virtual_list;
use crate::window::components::list_frame_notices::{ListFrameNoticeLine, ListFrameNotices};
use crate::window::library::feeds_list::{FEEDS_ROW_HEIGHT, FeedsItemRow};
use crate::window::library::parts::{library_feeds_detail, library_feeds_list};
use crate::window::library::state::LibraryState;
use crate::window::pointer_target::PointerList;

/// One painted row of the flat list: the entry row.
/// A feed entry names no right-click menu — the former row paints none.
fn paint_feeds_row(row: &FeedsItemRow, library: &LibraryState, selected: Option<&str>) -> Element {
    let press_row = row.clone();
    let press_library = library.clone();
    let play_id = row.row.id.clone();
    let play_library = library.clone();
    paint_item_row(
        ItemRowPaint {
            attr: "feeds",
            kind: "entry",
            height: FEEDS_ROW_HEIGHT,
            id: &row.id,
            entry: &row.row,
            selected: Some(row.id.as_str()) == selected,
        },
        Callback::new(move |()| press_library.press_feeds_row(&press_row)),
        Callback::new(move |()| play_library.play_feeds_row(&play_id)),
        None,
    )
}

/// The Feeds list: the header,
/// the load, failure, and empty lines, and the flat entry rows over native
/// scroll.
#[component]
pub fn LibraryFeedsList() -> Element {
    let library = consume_context::<LibraryState>();

    // The carried Feeds content and the core's selection (design D12, row
    // 11.1): the clones subscribe the list to the frames that carry them, as
    // the Music list's read does; the row model borrows the frame from the
    // section store's own storage through the memo's revision key.
    let carried = library.sections.library_list.cloned();
    let detail_section = library.sections.library_detail.cloned();
    let frame = library_feeds_list(carried.as_deref());
    let detail = library_feeds_detail(detail_section.as_deref());

    // The interaction state and the scroll store this list paints (design
    // D1, D9).
    let feeds_ui = library.feeds_ui.cloned();
    let scroll = library.scrolls.feeds.cloned();

    // The header's selection label (design D15, D6): the collections frame's
    // label — All, the one checked name, or the count with the noun.
    let selection_label = frame
        .and_then(|frame| frame.collections.as_ref())
        .map_or_else(|| "All".to_owned(), GuiCollectionsFrame::label);

    // The collection dropdown the header paints on the title's line (design
    // D6a): the destination's carried selector, which names the selection
    // itself, so the header paints no selection label beside it.
    let collections = frame.and_then(|frame| frame.collections.clone());

    // The painted rows of the flat list (row 11.1): the same model
    // [`crate::window::library::feeds`] builds from the carried content,
    // narrowed by the filter's text.
    let rows = library.feeds_rows();

    // The list's two DOM facts report to the scroll store (design D9), and
    // the rows paint over the section this render read.
    let measure_library = library.clone();
    let scroll_library = library.clone();
    let paint_library = library.clone();
    let paint_selected = detail.and_then(|detail| detail.selected.clone());
    let virtual_list = library_virtual_list(
        rows.as_ref(),
        scroll,
        "feeds",
        PointerList::Feeds,
        "8px 8px 8px 8px",
        Callback::new(move |height: f32| measure_library.measure_feeds_list(height)),
        Callback::new(move |top: f32| scroll_library.scroll_feeds_list(top)),
        move |row: &FeedsItemRow| paint_feeds_row(row, &paint_library, paint_selected.as_deref()),
    );

    let filter_library = library.clone();
    let accept_library = library.clone();

    rsx! {
        div {
            class: "flex min-h-0 min-w-0 flex-1 flex-col",
            "data-library-feeds-list-column": "",
            // The Music list's header (design D15): the list names itself, the
            // checked collections are the carried scope, and the `/` filter field
            // narrows it.
            div {
                class: "flex flex-none flex-col",
                LibraryListHeader {
                    title: "Feeds".to_owned(),
                    label: selection_label.clone(),
                    label_attr: Some("data-library-feeds-selection-label"),
                    collections,
                    filter: feeds_ui.filter.clone(),
                    placeholder: format!("Filter {selection_label}"),
                    aria_label: "Filter entries".to_owned(),
                    on_input: move |value: String| filter_library.set_feeds_filter(value),
                    on_accept: move |()| accept_library.accept_destination_filter(),
                }
            }
            if let Some(frame) = frame {
                // The tab's fetch results have not landed yet (row 11.1), a fetch
                // failed, or the active view holds no entry: one line per state,
                // and the empty line only over a landed row set.
                ListFrameNotices {
                    loading: frame.loading.then(|| {
                        ListFrameNoticeLine::loading("data-library-feeds-loading")
                    }),
                    failed: frame.failed.then(|| {
                        ListFrameNoticeLine::new(
                            "data-library-feeds-failed",
                            "Couldn’t load this destination.",
                        )
                    }),
                    empty: (!frame.loading && !frame.failed && frame.rows.is_empty()).then(|| {
                        ListFrameNoticeLine::new("data-library-feeds-empty", "No entries")
                    }),
                }
            }
            {virtual_list}
        }
    }
}

#[cfg(test)]
mod tests;
