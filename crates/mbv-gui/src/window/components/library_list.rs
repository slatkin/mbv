//! The Library list (`replace-tauri-with-blitz` task 7.2, the port of
//! `LibraryList`; add-gui-library-panel design D7): the canvas's
//! artist index in the list column — the *Artists* line over the selected
//! collection names (design D6), the `/` filter field, the canvas tree of
//! artist, album, and track rows under its letter headings, and the A–Z
//! letter rail beside it. The rows paint at the canvas's fixed heights over
//! native scroll, and only the rows in view paint, so a long artist list
//! stays fast.
//!
//! The open nodes and the filter text are window-content state in
//! [`crate::window::library::state`] (design D1), the scroll offset and the
//! measured viewport are the list's own store (design D9), and the row model is
//! [`crate::window::library::list`]'s. This component paints that state and
//! reports the two facts only the DOM holds — its measured viewport height and
//! its native scroll offset. A press picks and opens: an artist or album row
//! opens or closes and picks, an album without a cached tracklist asks the core
//! to load it, and a track picks its album. The list keeps no cursor: the keys
//! act on the row the pointer marks (`gui-pointer-target-interaction` D2).

use std::cell::RefCell;
use std::rc::Rc;

use dioxus_native::prelude::*;

use crate::bridge::frame::library::{GuiMetaKindFrame, GuiMetaSegmentFrame};
use crate::window::components::library_letter_rail::LibraryLetterRail;
use crate::window::components::library_list_header::LibraryListHeader;
use crate::window::components::library_virtual_list::library_virtual_list;
use crate::window::components::list_frame_notices::{ListFrameNoticeLine, ListFrameNotices};
use crate::window::components::typed_text::TypedText;
use crate::window::icons::ChevronRight;
use crate::window::input::f32_from_f64;
use crate::window::library::list::{
    LETTER_ROW_HEIGHT, LibraryRow, NODE_ROW_HEIGHT, TRACK_ROW_HEIGHT, letter_rail,
};
use crate::window::library::parts::{library_music_detail, library_music_list};
use crate::window::library::state::LibraryState;
use crate::window::pointer_target::PointerList;
use crate::window::theme::canvas_var;
use crate::window::truncate::{TextStyle, Truncate};

/// Whether the row is the album the Hero shows.
fn is_picked(row: &LibraryRow, picked: Option<&str>) -> bool {
    match row {
        LibraryRow::Album { album_id, .. } => picked == Some(album_id.as_str()),
        _ => false,
    }
}

/// A row's indent: the canvas's tree inset by depth, the letter heading's
/// own padding included.
fn row_padding(row: &LibraryRow) -> &'static str {
    match row {
        LibraryRow::Letter(_) => "0 8px 2px",
        LibraryRow::Artist { .. } => "0 8px 0 4px",
        LibraryRow::Album { .. } => "0 8px 0 22px",
        LibraryRow::Track { .. } => "0 8px 0 40px",
    }
}

/// The letter heading above the first artist of the letter (the former client
/// letter branch).
fn paint_letter_row(letter: char) -> Element {
    rsx! {
        div {
            class: "flex items-end font-mono",
            style: "height: {LETTER_ROW_HEIGHT}px; padding: 0 8px 2px; font-size: 12px; color: var(--canvas-type-label);",
            "data-library-letter": "{letter}",
            "{letter}"
        }
    }
}

/// The track row under an open album: the number,
/// the muted name, and the runtime.
fn paint_track_row(row: &LibraryRow, library: &LibraryState, picked_row: bool) -> Element {
    let LibraryRow::Track {
        base,
        track_id,
        name,
        number,
        duration,
        ..
    } = row
    else {
        return rsx! {};
    };
    let style = format!(
        "height: {TRACK_ROW_HEIGHT}px; padding: {}; gap: 8px; border: 0; border-radius: {}; text-align: left; font-size: 14px; cursor: pointer;",
        row_padding(row),
        if picked_row { "4px" } else { "0" }
    );
    let press_row = row.clone();
    let press_library = library.clone();
    let menu_library = library.clone();
    let play_library = library.clone();
    let item_id = LibraryState::action_item_id(row);
    let track_id = track_id.clone();
    rsx! {
        button {
            "type": "button",
            class: "flex w-full items-center overflow-hidden",
            style: "{style}",
            "data-library-row": "track",
            "data-library-row-id": "{base.id}",
            onclick: move |_| press_library.press_library_row(&press_row),
            oncontextmenu: move |event: MouseEvent| {
                event.prevent_default();
                let coordinates = event.client_coordinates();
                menu_library.open_row_menu_at(
                    item_id.clone(),
                    f32_from_f64(coordinates.x),
                    f32_from_f64(coordinates.y),
                );
            },
            ondoubleclick: move |_| play_library.play_library_track(track_id.clone()),
            span {
                class: "flex flex-none items-center justify-end font-mono tabular-nums",
                style: "width: 18px; font-size: 12px; color: var(--canvas-ordinal);",
                "{number}"
            }
            Truncate {
                text: name.clone(),
                class: "min-w-0 flex-1".to_owned(),
                css: "color: var(--canvas-muted);".to_owned(),
                style: TextStyle::sans(14.0, 400.0),
            }
            span {
                class: "flex-none font-mono tabular-nums",
                style: "font-size: 12px; color: var(--canvas-muted);",
                "{duration}"
            }
        }
    }
}

/// One painted row of the canvas tree: the letter
/// heading, the track row, or the artist and album node row.
fn paint_library_row(row: &LibraryRow, library: &LibraryState, picked: Option<&str>) -> Element {
    let picked_row = is_picked(row, picked);
    match row {
        LibraryRow::Letter(base) => paint_letter_row(base.letter),
        LibraryRow::Track { .. } => paint_track_row(row, library, picked_row),
        LibraryRow::Artist {
            base, name, open, ..
        }
        | LibraryRow::Album {
            base, name, open, ..
        } => {
            let kind = match row {
                LibraryRow::Artist { .. } => "artist",
                _ => "album",
            };
            let color = match row {
                LibraryRow::Artist { .. } => canvas_var("heading"),
                _ => canvas_var("text"),
            };
            let weight = if matches!(row, LibraryRow::Artist { .. }) {
                600.0
            } else {
                400.0
            };
            let year = match row {
                LibraryRow::Album { year, .. } if !year.is_empty() => Some(year.clone()),
                _ => None,
            };
            let style = format!(
                "height: {NODE_ROW_HEIGHT}px; padding: {}; gap: 8px; border: 0; border-radius: {}; text-align: left; font-size: 14.5px; cursor: pointer;",
                row_padding(row),
                if picked_row { "4px" } else { "0" }
            );
            let press_row = row.clone();
            let press_library = library.clone();
            let menu_library = library.clone();
            let item_id = LibraryState::action_item_id(row);
            let rotate = if *open { 90 } else { 0 };
            rsx! {
                button {
                    "type": "button",
                    class: "flex w-full items-center overflow-hidden",
                    style: "{style}",
                    "aria-expanded": "{open}",
                    "data-library-row": "{kind}",
                    "data-library-row-id": "{base.id}",
                    onclick: move |_| press_library.press_library_row(&press_row),
                    oncontextmenu: move |event: MouseEvent| {
                        event.prevent_default();
                        let coordinates = event.client_coordinates();
                        menu_library.open_row_menu_at(
                            item_id.clone(),
                            f32_from_f64(coordinates.x),
                            f32_from_f64(coordinates.y),
                        );
                    },
                    span {
                        class: "flex flex-none items-center justify-center",
                        style: "width: 18px; color: {color};",
                        span {
                            class: "flex",
                            style: "transform: rotate({rotate}deg); transition: transform 120ms ease;",
                            ChevronRight { size: 10, stroke_width: 3.0 }
                        }
                    }
                    Truncate {
                        text: name.clone(),
                        class: "min-w-0 flex-1".to_owned(),
                        css: format!("color: {color}; font-weight: {weight};"),
                        style: TextStyle::sans(14.5, weight),
                    }
                    if let Some(year) = year {
                        // A year repeats down the tree, so it paints the date
                        // token at the repeat opacity; the picked album keeps
                        // its own ink (design D14).
                        span {
                            class: "flex-none font-mono tabular-nums",
                            style: if picked_row {
                                "font-size: 12px; color: var(--canvas-muted);"
                            } else {
                                "font-size: 12px;"
                            },
                            "data-library-row-year": "",
                            TypedText {
                                segments: vec![GuiMetaSegmentFrame {
                                    text: year,
                                    kind: GuiMetaKindFrame::Date,
                                }],
                                repeat: !picked_row,
                            }
                        }
                    }
                }
            }
        }
    }
}

/// The Library list. It reads
/// the carried Music frames from the section store itself, so a panel
/// repaint never deep-clones the union's artists for it (the port review's
/// whole-list finding).
#[component]
pub fn LibraryList() -> Element {
    let library = consume_context::<LibraryState>();

    // The carried content the row model borrows (design D6, D15): the
    // clones subscribe the list to the frames that carry them, and the memo
    // borrows the union from the section store's own storage and keys on its
    // revision.
    let carried = library.sections.library_list.cloned();
    let detail_section = library.sections.library_detail.cloned();
    let frame = library_music_list(carried.as_deref());
    let detail = library_music_detail(detail_section.as_deref());

    // The interaction state and the scroll store this list paints (design
    // D1, D9).
    let ui = library.ui.cloned();
    let scroll = library.music_scroll.cloned();

    // The painted rows of the canvas tree (design D7): the same model
    // [`crate::window::library::state`] builds from the carried content,
    // over the open nodes and the filter this component paints.
    let rows = library.rows_of(&ui);

    // The album the Hero shows: the list paints its row in the canvas's
    // picked fill (design D7).
    let picked = detail
        .and_then(|detail| detail.hero.as_ref())
        .map(|hero| hero.album.album_id.clone());

    // The album the list last revealed. A pick reveals once, so a later
    // frame carrying the same pick does not move the cursor again. The
    // effect reads the carried frames and writes no signal it reads — the
    // reveal moves the interaction state and the scroll store; the `Rc`
    // shares the one reveal record across the effect's re-runs.
    let revealed_album = use_hook(|| Rc::new(RefCell::new(None::<String>)));
    let reveal_library = library.clone();
    use_effect(move || {
        let list_section = reveal_library.sections.library_list.cloned();
        let detail_section = reveal_library.sections.library_detail.cloned();
        let (Some(list), Some(detail)) = (
            library_music_list(list_section.as_deref()),
            library_music_detail(detail_section.as_deref()),
        ) else {
            return;
        };
        let album_id = detail.hero.as_ref().map(|hero| hero.album.album_id.clone());
        let mut revealed = revealed_album.borrow_mut();
        if album_id.is_none() || album_id == *revealed {
            return;
        }
        *revealed = album_id;
        drop(revealed);
        reveal_library.reveal_picked_album(list, detail);
    });

    // The selected collection names in pill order (design D6): the core's
    // one derivation, carried on the Music frame so the list header and the
    // Status bar location cannot disagree. An empty selection is the whole
    // library: the header names it `All` instead of an empty label.
    let collection_names = frame.map_or(&[][..], |frame| frame.collection_names.as_slice());
    let selection_label = match collection_names {
        [] => "All".to_owned(),
        [one] => format!("{one} · sort name"),
        names => names.join(" + "),
    };

    // The collection dropdown the header paints on the title's line (design
    // D6a): the destination's carried selector, which names the selection
    // itself, so the header paints no selection label beside it.
    let collections = frame.and_then(|frame| frame.collections.clone());

    // The filter field's placeholder, naming the selection as the canvas
    // does.
    let filter_placeholder = match collection_names {
        [] => "Filter All".to_owned(),
        names => format!("Filter {}", names.join(" + ")),
    };

    // The rail's A–Z letters over the painted rows.
    let rail = letter_rail(&rows);

    // The list's two DOM facts report to the scroll store (design D9), and
    // the rows paint over the interaction state this render read.
    let measure_library = library.clone();
    let scroll_library = library.clone();
    let paint_library = library.clone();
    let virtual_list = library_virtual_list(
        rows.as_ref(),
        scroll,
        "",
        PointerList::Music,
        "0 0 0 8px",
        Callback::new(move |height: f32| measure_library.measure_library_list(height)),
        Callback::new(move |top: f32| scroll_library.scroll_library_list(top)),
        |row: &LibraryRow| paint_library_row(row, &paint_library, picked.as_deref()),
    );

    let filter_library = library.clone();
    let accept_library = library.clone();

    rsx! {
        div {
            class: "flex min-h-0 min-w-0 flex-1 flex-col",
            "data-library-list-column": "",
            div {
                class: "flex flex-none flex-col",
                LibraryListHeader {
                    title: "Artists".to_owned(),
                    label: selection_label,
                    collections,
                    filter: ui.filter.clone(),
                    placeholder: filter_placeholder,
                    aria_label: "Filter artists".to_owned(),
                    on_input: move |value: String| filter_library.set_library_filter(value),
                    on_accept: move |()| accept_library.accept_library_filter(),
                }
            }
            if let Some(frame) = frame {
                // A selected group's album list has not landed yet (design D5),
                // the selected groups' album lists failed, or the collections
                // hold no artist: one line per state, the failure line naming
                // the groups the frame names.
                ListFrameNotices {
                    loading: frame.loading.then(|| {
                        ListFrameNoticeLine::loading("data-library-loading")
                    }),
                    failed: frame.failure.clone().map(|failure| {
                        ListFrameNoticeLine::new("data-library-failure", failure)
                    }),
                    empty: (!frame.loading && frame.artists.is_empty()).then(|| {
                        ListFrameNoticeLine::new("data-library-empty", "No artists")
                    }),
                }
            }
            div {
                class: "flex min-h-0 flex-1",
                style: "overflow: hidden;",
                {virtual_list}
                LibraryLetterRail { letters: rail }
            }
        }
    }
}

#[cfg(test)]
mod tests;
