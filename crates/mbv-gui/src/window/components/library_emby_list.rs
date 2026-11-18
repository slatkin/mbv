//! The plain Emby-library list (`replace-tauri-with-blitz` task 7.7, the
//! port of `LibraryEmbyList`; add-gui-library-panel design D12, row
//! 8.1): the plain mode's name-sorted rows under their letter headings, with
//! the A–Z letter rail beside them, and the feed-group mode's flat rows with
//! their date gutter and no rail.
//!
//! The rows paint at the fixed heights of [`crate::window::library::emby_list`]
//! over native scroll, and only the rows in view paint, so a long library
//! stays fast. The list keeps no cursor: the keys act on the row the pointer
//! marks (`gui-pointer-target-interaction` D2); the scroll offset and the
//! viewport are the Emby list's scroll store (design D9). A press selects
//! the row the Hero shows, a double-click plays it, and the keys' Enter plays
//! the hovered row — every row is playable, because the core's folder-play rule
//! expands a folder or a boxset row.

use dioxus_native::prelude::*;

use crate::bridge::frame::library::GuiEmbyRowKindFrame;
use crate::window::components::library_item_row::{LibraryItemRow, LibraryTrailPart};
use crate::window::components::library_letter_heading::LibraryLetterHeading;
use crate::window::components::library_letter_rail::LibraryLetterRail;
use crate::window::components::library_list_header::LibraryListHeader;
use crate::window::components::library_virtual_list::library_virtual_list;
use crate::window::components::list_frame_notices::{ListFrameNoticeLine, ListFrameNotices};
use crate::window::library::emby_list::{EmbyListRow, emby_letter_rail};
use crate::window::library::parts::{library_emby_detail, library_emby_list};
use crate::window::library::state::LibraryState;
use crate::window::pointer_target::PointerList;
use crate::window::theme::canvas_var;

/// The row kind word the item row's hook paints (`data-library-emby-row`).
fn kind_word(kind: GuiEmbyRowKindFrame) -> &'static str {
    match kind {
        GuiEmbyRowKindFrame::Collection => "collection",
        GuiEmbyRowKindFrame::Media => "media",
    }
}

/// One painted row of the list: the letter heading
/// or the item row.
fn paint_emby_row(row: &EmbyListRow, library: &LibraryState, selected: Option<&str>) -> Element {
    match row {
        EmbyListRow::Heading { letter, height, .. } => rsx! {
            LibraryLetterHeading {
                attr: "emby",
                letter: letter.to_string(),
                height: *height,
            }
        },
        EmbyListRow::Item(item) => {
            let selected_row = selected == Some(item.id.as_str());
            let background = selected_row.then(|| canvas_var("library-raise"));
            let press_row = row.clone();
            let press_library = library.clone();
            let play_id = item.row.id.clone();
            let play_library = library.clone();
            let menu_id = item.row.id.clone();
            let menu_library = library.clone();
            rsx! {
                LibraryItemRow {
                    attr: "emby",
                    kind: kind_word(item.row.kind).to_owned(),
                    id: item.id.clone(),
                    height: item.height,
                    title: item.row.title.clone(),
                    subtitle: item.row.subtitle.clone(),
                    trail: item.row.date.clone(),
                    trail_part: LibraryTrailPart::Date,
                    title_color: canvas_var("text"),
                    background,
                    selected: selected_row,
                    progress: None,
                    on_press: Callback::new(move |()| press_library.press_emby_row(&press_row)),
                    on_play: Callback::new(move |()| play_library.play_emby_row(&play_id)),
                    on_context: Some(Callback::new(
                        move |(x, y): (f32, f32)| menu_library.open_row_menu_at(
                            Some(menu_id.clone()),
                            x,
                            y,
                        ),
                    )),
                }
            }
        }
    }
}

/// The plain Emby-library list:
/// the header, the load, failure, and empty lines, the rows over native
/// scroll, and the plain mode's letter rail.
#[component]
pub fn LibraryEmbyList() -> Element {
    let library = consume_context::<LibraryState>();

    // The carried Emby content and the core's selection (row 8.1): the
    // clones subscribe the list to the frames that carry them, as the Music
    // list's read does; the row model borrows the frame from the section
    // store's own storage through the memo's revision key.
    let carried = library.sections.library_list.cloned();
    let detail_section = library.sections.library_detail.cloned();
    let frame = library_emby_list(carried.as_deref());
    let detail = library_emby_detail(detail_section.as_deref());

    // The interaction state and the scroll store this list paints (design
    // D1, D9).
    let emby_ui = library.emby_ui.cloned();
    let scroll = library.scrolls.emby.cloned();

    // The painted rows (row 8.1): the plain mode's rows with their headings,
    // or the feed-group mode's flat rows, narrowed by the filter's text.
    let rows = library.emby_rows();

    // The header's selection label (design D15): the feed-group mode's
    // selected group, and `All` for a plain library, whose list is the whole
    // library — the Music header's own rule.
    let selection_names = frame
        .map(|frame| frame.collection_names.clone())
        .unwrap_or_default();
    let selection_label = if selection_names.is_empty() {
        "All".to_owned()
    } else {
        selection_names.join(" + ")
    };

    // The filter field's placeholder, naming the selection it narrows.
    let filter_placeholder = if selection_names.is_empty() {
        "Filter".to_owned()
    } else {
        format!("Filter {}", selection_names.join(" + "))
    };

    // The collection dropdown the header paints on the title's line (design
    // D6a): the feed-group mode's carried selector, which names the selection
    // itself, so the header paints no selection label beside it. A plain
    // library carries none, so its header keeps the label.
    let collections = frame.and_then(|frame| frame.collections.clone());

    // The rail's A–Z letters over the painted rows (row 8.1): the plain mode
    // paints it, and the date-ordered feed-group mode paints none.
    let rail = emby_letter_rail(&rows);

    let selected = detail.and_then(|detail| detail.selected.clone());

    // The list's two DOM facts report to the scroll store (design D9), and
    // the rows paint over the interaction state this render read.
    let measure_library = library.clone();
    let scroll_library = library.clone();
    let paint_library = library.clone();
    let paint_selected = selected.clone();
    let virtual_list = library_virtual_list(
        rows.as_ref(),
        scroll,
        "emby",
        PointerList::Emby,
        "8px 0 8px 8px",
        Callback::new(move |height: f32| measure_library.measure_emby_list(height)),
        Callback::new(move |top: f32| scroll_library.scroll_emby_list(top)),
        move |row: &EmbyListRow| paint_emby_row(row, &paint_library, paint_selected.as_deref()),
    );

    // The plain mode paints the rail (row 8.1); the feed-group mode's
    // date-ordered rows paint none.
    let paints_rail = frame.is_some_and(|frame| !frame.feed_group);

    // The rail's press scrolls the list to the letter's first row (design
    // D7): the rail's keyboard path is the `/` filter.
    let jump_library = library.clone();
    let filter_library = library.clone();
    let accept_library = library.clone();

    rsx! {
        div {
            class: "flex min-h-0 min-w-0 flex-1 flex-col",
            "data-library-emby-list-column": "",
            // The Music list's header (design D15): the library's name, the
            // carried selection's label, and the `/` filter field.
            div {
                class: "flex flex-none flex-col",
                LibraryListHeader {
                    title: frame.map_or(String::new(), |frame| frame.library_name.clone()),
                    label: selection_label,
                    label_attr: Some("data-library-emby-selection-label"),
                    collections,
                    filter: emby_ui.filter.clone(),
                    placeholder: filter_placeholder,
                    aria_label: "Filter items".to_owned(),
                    on_input: move |value: String| filter_library.set_emby_filter(value),
                    on_accept: move |()| accept_library.accept_destination_filter(),
                }
            }
            if let Some(frame) = frame {
                // The active list has not landed yet (row 8.1), the plain mode's
                // root-list fetch failed, or the library holds no item: one line
                // per state, and the empty line only over a landed row set.
                ListFrameNotices {
                    loading: frame.loading.then(|| {
                        ListFrameNoticeLine::loading("data-library-emby-loading")
                    }),
                    failed: frame.failed.then(|| {
                        ListFrameNoticeLine::new(
                            "data-library-emby-failed",
                            "Couldn’t load this library.",
                        )
                    }),
                    empty: (!frame.loading && !frame.failed && frame.rows.is_empty()).then(|| {
                        ListFrameNoticeLine::new("data-library-emby-empty", "No items")
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
                            jump_library.jump_emby_to_letter(letter);
                        })),
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests;
