//! The Podcasts list (`replace-tauri-with-blitz` task 7.7, the port of
//! `LibraryPodcastsList`; add-gui-library-panel design D12, row
//! 10.1): the active pill's flat episode rows — the episode's title over its
//! show, and the publish date in the gutter — with no letter headings,
//! because the list is sorted by date, and so no letter rail.
//!
//! The rows paint at the fixed heights of
//! [`crate::window::library::podcasts_list`] over native scroll, and only
//! the rows in view paint, so a long Latest merge stays fast. The list keeps
//! no cursor: the keys act on the row the pointer marks
//! (`gui-pointer-target-interaction` D2); the scroll offset and the viewport are
//! the Podcasts list's scroll store (design D9). A press selects the episode the
//! Hero shows, and a double-click plays it (design D12): the keys' Enter plays
//! the hovered row, and the press is the selection's half.

use dioxus_native::prelude::*;

use crate::bridge::frame::library::GuiCollectionsFrame;
use crate::window::components::library_item_row::{ItemRowPaint, paint_item_row};
use crate::window::components::library_list_header::LibraryListHeader;
use crate::window::components::library_virtual_list::library_virtual_list;
use crate::window::components::list_frame_notices::{ListFrameNoticeLine, ListFrameNotices};
use crate::window::library::parts::{library_podcasts_detail, library_podcasts_list};
use crate::window::library::podcasts_list::{PODCASTS_ROW_HEIGHT, PodcastsItemRow};
use crate::window::library::state::LibraryState;
use crate::window::pointer_target::PointerList;

/// One painted row of the flat list: the episode row. The row's press selects
/// the episode the Hero shows, a double-click plays it, and a right-press
/// opens the row menu.
fn paint_podcasts_row(
    row: &PodcastsItemRow,
    library: &LibraryState,
    selected: Option<&str>,
) -> Element {
    let press_row = row.clone();
    let press_library = library.clone();
    let play_id = row.row.id.clone();
    let play_library = library.clone();
    let menu_id = row.row.id.clone();
    let menu_library = library.clone();
    paint_item_row(
        ItemRowPaint {
            attr: "podcasts",
            kind: "episode",
            height: PODCASTS_ROW_HEIGHT,
            id: &row.id,
            entry: &row.row,
            selected: Some(row.id.as_str()) == selected,
        },
        Callback::new(move |()| press_library.press_podcasts_row(&press_row)),
        Callback::new(move |()| play_library.play_podcasts_row(&play_id)),
        Some(Callback::new(move |(x, y): (f32, f32)| {
            menu_library.open_row_menu_at(Some(menu_id.clone()), x, y);
        })),
    )
}

/// The Podcasts list: the
/// header, the load, failure, and empty lines, and the flat episode rows
/// over native scroll.
#[component]
pub fn LibraryPodcastsList() -> Element {
    let library = consume_context::<LibraryState>();

    // The carried Podcasts content and the core's selection (design D12,
    // row 10.1): the clones subscribe the list to the frames that carry
    // them, as the Music list's read does; the row model borrows the frame
    // from the section store's own storage through the memo's revision key.
    let carried = library.sections.library_list.cloned();
    let detail_section = library.sections.library_detail.cloned();
    let frame = library_podcasts_list(carried.as_deref());
    let detail = library_podcasts_detail(detail_section.as_deref());

    // The interaction state and the scroll store this list paints (design
    // D1, D9).
    let podcasts_ui = library.podcasts_ui.cloned();
    let scroll = library.scrolls.podcasts.cloned();

    // The header's selection label (design D15, D6): the collections frame's
    // label — All, the one checked name, or the count with the noun — so the
    // column names the view it holds.
    let selection_label = frame
        .and_then(|frame| frame.collections.as_ref())
        .map_or_else(|| "All".to_owned(), GuiCollectionsFrame::label);

    // The painted rows of the flat list (row 10.1): the same model
    // [`crate::window::library::podcasts`] builds from the carried content,
    // narrowed by the filter's text.
    let rows = library.podcasts_rows();

    // The list's two DOM facts report to the scroll store (design D9), and
    // the rows paint over the section this render read.
    let measure_library = library.clone();
    let scroll_library = library.clone();
    let paint_library = library.clone();
    let paint_selected = detail.and_then(|detail| detail.selected.clone());
    let virtual_list = library_virtual_list(
        rows.as_ref(),
        scroll,
        "podcasts",
        PointerList::Podcasts,
        "8px 8px 8px 8px",
        Callback::new(move |height: f32| measure_library.measure_podcasts_list(height)),
        Callback::new(move |top: f32| scroll_library.scroll_podcasts_list(top)),
        move |row: &PodcastsItemRow| {
            paint_podcasts_row(row, &paint_library, paint_selected.as_deref())
        },
    );

    let filter_library = library.clone();
    let accept_library = library.clone();

    rsx! {
        div {
            class: "flex min-h-0 min-w-0 flex-1 flex-col",
            "data-library-podcasts-list-column": "",
            // The Music list's header (design D15): the list names itself, the
            // active pill is the carried scope, and the `/` filter field
            // narrows it.
            div {
                class: "flex flex-none flex-col",
                LibraryListHeader {
                    title: "Podcasts".to_owned(),
                    label: selection_label.clone(),
                    label_attr: Some("data-library-podcasts-selection-label"),
                    filter: podcasts_ui.filter.clone(),
                    placeholder: format!("Filter {selection_label}"),
                    aria_label: "Filter episodes".to_owned(),
                    on_input: move |value: String| filter_library.set_podcasts_filter(value),
                    on_accept: move |()| accept_library.accept_destination_filter(),
                }
            }
            if let Some(frame) = frame {
                // The active view's episodes have not landed yet (row 10.1), the
                // show catalog's fetch failed, or the view holds no episode:
                // one line per state, and the empty line only over a landed row
                // set.
                ListFrameNotices {
                    loading: frame.loading.then(|| {
                        ListFrameNoticeLine::loading("data-library-podcasts-loading")
                    }),
                    failed: frame.failed.then(|| {
                        ListFrameNoticeLine::new(
                            "data-library-podcasts-failed",
                            "Couldn’t load this library.",
                        )
                    }),
                    empty: (!frame.loading && !frame.failed && frame.rows.is_empty()).then(|| {
                        ListFrameNoticeLine::new("data-library-podcasts-empty", "No episodes")
                    }),
                }
            }
            {virtual_list}
        }
    }
}

#[cfg(test)]
mod tests;
