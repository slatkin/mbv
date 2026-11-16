//! The Audiobooks book list (`replace-tauri-with-blitz` task 7.7, the port
//! of `LibraryAudiobooksList`; add-gui-library-panel design D12, row
//! 9.1): the catalog's book rows in the TUI's sort — author sort key, then
//! title — under their letter headings, with the A–Z letter rail beside
//! them. No second mode rides on the destination and no pill row: the rail
//! replaces the TUI's surname buckets.
//!
//! The rows paint at the fixed heights of
//! [`crate::window::library::audiobooks_list`] over native scroll, and only
//! the rows in view paint, so a long catalog stays fast. The list keeps no
//! cursor: the keys act on the row the pointer marks
//! (`gui-pointer-target-interaction` D2); the scroll offset and the viewport are
//! the Audiobooks list's scroll store (design D9). A press selects the book the
//! Hero shows, and a double-click plays it from its saved position — the same
//! play command the keys' Enter sends. A book has no play-next: `n` is consumed
//! and does nothing.
//!
//! The Hero's chapter Workspace keeps no cursor either: the keys act on the
//! chapter the pointer marks, so a selection change needs no reset.

use dioxus_native::prelude::*;

use crate::window::components::library_item_row::{LibraryItemRow, LibraryTrailPart};
use crate::window::components::library_letter_heading::LibraryLetterHeading;
use crate::window::components::library_letter_rail::LibraryLetterRail;
use crate::window::components::library_list_header::LibraryListHeader;
use crate::window::components::library_virtual_list::library_virtual_list;
use crate::window::components::list_frame_notices::{ListFrameNoticeLine, ListFrameNotices};
use crate::window::library::audiobooks_list::{AudiobooksListRow, audiobooks_letter_rail};
use crate::window::library::parts::{library_audiobooks_detail, library_audiobooks_list};
use crate::window::library::state::LibraryState;
use crate::window::pointer_target::PointerList;
use crate::window::theme::canvas_var;

/// One painted row of the list: the letter heading
/// or the book row.
fn paint_audiobooks_row(
    row: &AudiobooksListRow,
    library: &LibraryState,
    selected: Option<&str>,
) -> Element {
    match row {
        AudiobooksListRow::Heading { letter, height, .. } => rsx! {
            LibraryLetterHeading {
                attr: "abs",
                letter: letter.to_string(),
                height: *height,
            }
        },
        AudiobooksListRow::Item(item) => {
            let selected_row = selected == Some(item.id.as_str());
            let press_row = row.clone();
            let press_library = library.clone();
            let play_id = item.row.id.clone();
            let play_library = library.clone();
            let menu_id = item.row.id.clone();
            let menu_library = library.clone();
            rsx! {
                LibraryItemRow {
                    attr: "abs",
                    kind: "book".to_owned(),
                    id: item.id.clone(),
                    height: item.height,
                    title: item.row.title.clone(),
                    subtitle: item.row.subtitle.clone(),
                    trail: Some(item.row.duration.clone()),
                    trail_part: LibraryTrailPart::Duration,
                    title_color: canvas_var("text"),
                    background: selected_row.then(|| canvas_var("library-raise")),
                    selected: selected_row,
                    progress: None,
                    on_press: Callback::new(move |()| {
                        press_library.press_audiobooks_row(&press_row);
                    }),
                    on_play: Callback::new(move |()| play_library.play_audiobooks_row(&play_id)),
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

/// The Audiobooks book list:
/// the header, the load, failure, and empty lines, the rows over native
/// scroll, and the letter rail.
#[component]
pub fn LibraryAudiobooksList() -> Element {
    let library = consume_context::<LibraryState>();

    // The carried book content and the core's selection (row 9.1): the
    // clones subscribe the list to the frames that carry them, as the Music
    // list's read does; the row model borrows the frame from the section
    // store's own storage through the memo's revision key.
    let carried = library.sections.library_list.cloned();
    let detail_section = library.sections.library_detail.cloned();
    let frame = library_audiobooks_list(carried.as_deref());
    let detail = library_audiobooks_detail(detail_section.as_deref());

    // The interaction state and the scroll store this list paints (design
    // D1, D9).
    let audiobooks_ui = library.audiobooks_ui.cloned();
    let scroll = library.scrolls.audiobooks.cloned();

    // The painted rows (row 9.1): the book rows with their letter headings,
    // narrowed by the filter's text.
    let rows = library.audiobooks_rows(&audiobooks_ui.filter.clone());

    // The rail's A–Z letters over the painted rows (row 9.1).
    let rail = audiobooks_letter_rail(&rows);

    let selected = detail.and_then(|detail| detail.selected.clone());

    // The list's two DOM facts report to the scroll store (design D9), and
    // the rows paint over the section this render read.
    let measure_library = library.clone();
    let scroll_library = library.clone();
    let paint_library = library.clone();
    let paint_selected = selected.clone();
    let virtual_list = library_virtual_list(
        rows.as_ref(),
        scroll,
        "abs",
        PointerList::Audiobooks,
        "8px 0 8px 8px",
        Callback::new(move |height: f32| measure_library.measure_audiobooks_list(height)),
        Callback::new(move |top: f32| scroll_library.scroll_audiobooks_list(top)),
        move |row: &AudiobooksListRow| {
            paint_audiobooks_row(row, &paint_library, paint_selected.as_deref())
        },
    );

    // The rail's press scrolls the list to the letter's first book (design
    // D7).
    let jump_library = library.clone();
    let filter_library = library.clone();
    let accept_library = library.clone();

    rsx! {
        div {
            class: "flex min-h-0 min-w-0 flex-1 flex-col",
            "data-library-abs-list-column": "",
            // The Music list's header (design D15): the list names itself,
            // `All` is the whole catalog, and the `/` filter field narrows it.
            div {
                class: "flex flex-none flex-col",
                LibraryListHeader {
                    title: "Audiobooks".to_owned(),
                    label: "All".to_owned(),
                    filter: audiobooks_ui.filter.clone(),
                    placeholder: "Filter books".to_owned(),
                    aria_label: "Filter books".to_owned(),
                    on_input: move |value: String| filter_library.set_audiobooks_filter(value),
                    on_accept: move |()| accept_library.accept_destination_filter(),
                }
            }
            if let Some(frame) = frame {
                // The catalog has not landed yet (row 9.1), its fetch failed, or
                // it holds no book: one line per state, and the empty line only
                // over a landed row set.
                ListFrameNotices {
                    loading: frame.loading.then(|| {
                        ListFrameNoticeLine::loading("data-library-abs-loading")
                    }),
                    failed: frame.failed.then(|| {
                        ListFrameNoticeLine::new(
                            "data-library-abs-failed",
                            "Couldn’t load this catalog.",
                        )
                    }),
                    empty: (!frame.loading && !frame.failed && frame.rows.is_empty()).then(|| {
                        ListFrameNoticeLine::new("data-library-abs-empty", "No audiobooks")
                    }),
                }
            }
            div {
                class: "flex min-h-0 flex-1",
                style: "overflow: hidden;",
                {virtual_list}
                LibraryLetterRail {
                    letters: rail,
                    jump: Some(Callback::new(move |letter: char| {
                        jump_library.jump_audiobooks_to_letter(letter);
                    })),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests;
