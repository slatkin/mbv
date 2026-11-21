//! The Library list (`replace-tauri-with-blitz` task 7.2, the port of
//! `LibraryList`; add-gui-library-panel design D7): the canvas's
//! artist index in the list column — the *Artists* line over the selected
//! collection names (design D6), the `/` filter field, the canvas tree of
//! artist, album, and track rows under its letter headings, and the A–Z
//! letter rail beside it. The rows paint at the canvas's fixed heights over
//! native scroll, and only the rows in view paint, so a long artist list
//! stays fast. The artist rows are top-level rows and paint through the shared
//! row part with their art thumb; the album and track rows under them keep
//! their own painters (gui-delineated-rows design D1). The rows it paints are
//! also the row art set the core fetches (design D7).
//!
//! The open nodes and the filter text are window-content state in
//! [`crate::window::library::state`] (design D1), the scroll offset and the
//! measured viewport are the list's own store (design D9), and the row model is
//! [`crate::window::library::list`]'s. This component paints that state and
//! reports the two facts only the DOM holds — its measured viewport height and
//! its native scroll offset. A press picks: an album or track row picks its
//! album. A double-click on an artist or album row opens or closes the node
//! (gui-tree-double-click design D1), and an album without a cached tracklist
//! asks the core to load it. The list keeps no cursor: the keys
//! act on the row the pointer marks (`gui-pointer-target-interaction` D2).

use std::cell::RefCell;
use std::rc::Rc;

use dioxus_native::prelude::*;

use crate::bridge::frame::library::{GuiMetaKindFrame, GuiMetaSegmentFrame};
use crate::bridge::library::GuiRowArtScopeFrame;
use crate::window::components::library_item_row::{
    LibraryItemRow, LibraryRowThumb, LibraryTrailPart, ROW_PADDING_PX,
};
use crate::window::components::library_letter_rail::LibraryLetterRail;
use crate::window::components::library_list_header::{LibraryListHeader, LibraryListHeading};
use crate::window::components::library_virtual_list::library_virtual_list;
use crate::window::components::list_frame_notices::{ListFrameNoticeLine, ListFrameNotices};
use crate::window::components::tree_guide::{
    LEAF_INSET_X, NESTED_INSET_X, leaf_guides, nested_parent_guides,
};
use crate::window::components::tree_hint::TreeHint;
use crate::window::components::typed_text::TypedText;
use crate::window::input::f32_from_f64;
use crate::window::library::list::{
    LETTER_ROW_HEIGHT, LibraryRow, NODE_ROW_HEIGHT, TRACK_ROW_HEIGHT, letter_rail,
};
use crate::window::library::parts::{library_music_detail, library_music_list};
use crate::window::library::state::LibraryState;
use crate::window::pointer_target::PointerList;
use crate::window::service_mark::ServiceMark;
use crate::window::theme::canvas_var;
use crate::window::truncate::{TextStyle, Truncate};

/// Whether the row is the album the Hero shows.
fn is_picked(row: &LibraryRow, picked: Option<&str>) -> bool {
    match row {
        LibraryRow::Album { album_id, .. } => picked == Some(album_id.as_str()),
        _ => false,
    }
}

/// The letter heading above the first artist of the letter (the former client
/// letter branch).
fn paint_letter_row(letter: char) -> Element {
    rsx! {
        div {
            class: "flex items-end font-mono",
            style: "height: {LETTER_ROW_HEIGHT}px; padding: 0 {ROW_PADDING_PX}px 2px; font-size: 12px; color: var(--canvas-type-label);",
            "data-library-letter": "{letter}",
            "{letter}"
        }
    }
}

/// The track row under an open album: the number,
/// the muted name, and the runtime. It is a leaf of the tree, so it paints both
/// guides that pass through it — the artist's and its album's
/// (gui-tree-indent-guides design D3).
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
    // The leaf inset clears both guides that pass through the row
    // (gui-tree-indent-guides design D2, D3).
    let style = format!(
        "height: {TRACK_ROW_HEIGHT}px; padding: 0 {ROW_PADDING_PX}px; padding-left: {LEAF_INSET_X}px; gap: 8px; border: 0; border-radius: {}; text-align: left; font-size: 14px; cursor: pointer;",
        if picked_row { "4px" } else { "0" }
    );
    let press_row = row.clone();
    let press_library = library.clone();
    let menu_row = row.clone();
    let menu_library = library.clone();
    let play_library = library.clone();
    let item_id = LibraryState::action_item_id(row);
    let track_id = track_id.clone();
    rsx! {
        button {
            "type": "button",
            class: "relative flex w-full items-center overflow-hidden",
            style: "{style}",
            "data-library-row": "track",
            "data-library-row-id": "{base.id}",
            onclick: move |_| press_library.press_library_row(&press_row),
            oncontextmenu: move |event: MouseEvent| {
                event.prevent_default();
                // A right-click selects the row first, as a press does, so the
                // menu's View Detail opens the Hero for this row.
                menu_library.press_library_row(&menu_row);
                let coordinates = event.client_coordinates();
                menu_library.open_row_menu_at(
                    item_id.clone().map(LibraryState::row_menu_item),
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
            {leaf_guides()}
        }
    }
}

/// One painted row of the canvas tree: the letter heading, the track row, the
/// shared part's artist row (gui-delineated-rows design D1), or the nested
/// album node.
fn paint_library_row(row: &LibraryRow, library: &LibraryState, picked: Option<&str>) -> Element {
    let picked_row = is_picked(row, picked);
    match row {
        LibraryRow::Letter(base) => paint_letter_row(base.letter),
        LibraryRow::Track { .. } => paint_track_row(row, library, picked_row),
        LibraryRow::Artist { .. } => paint_artist_row(row, library, picked_row),
        LibraryRow::Album { .. } => paint_album_row(row, library, picked_row),
    }
}

/// A top-level artist row (gui-delineated-rows design D1): the shared row part
/// over the artist's name and its art thumb. It is a tree parent
/// (gui-tree-double-click design D2, D4), so it paints no caret: its
/// double-click toggles the node and its hover shows the hint. A press picks
/// nothing (the Hero is album-scoped), and a double-click only opens it, so the
/// row names no play path. While it shows album rows it starts its guide under
/// its thumb (gui-tree-indent-guides design D3).
fn paint_artist_row(row: &LibraryRow, library: &LibraryState, picked_row: bool) -> Element {
    let LibraryRow::Artist {
        base,
        name,
        open,
        shows_children,
        art,
        media,
        ..
    } = row
    else {
        return rsx! {};
    };
    let press_row = row.clone();
    let press_library = library.clone();
    let toggle_row = row.clone();
    let toggle_library = library.clone();
    let menu_library = library.clone();
    let item_id = LibraryState::action_item_id(row);
    rsx! {
        LibraryItemRow {
            attr: "",
            kind: "artist".to_owned(),
            id: base.id.clone(),
            height: base.height,
            title: name.clone(),
            subtitle: None,
            trail: None,
            trail_part: LibraryTrailPart::Date,
            title_color: canvas_var("text"),
            background: picked_row.then(|| canvas_var("library-raise")),
            selected: picked_row,
            progress: None,
            thumb: Some(LibraryRowThumb {
                art: art.clone(),
                media: *media,
            }),
            tree_open: Some(*open),
            tree_guide: *shows_children,
            on_press: Callback::new(move |()| press_library.press_library_row(&press_row)),
            on_play: Callback::new(|()| {}),
            on_toggle: Some(Callback::new(move |()| {
                toggle_library.toggle_library_node(&toggle_row);
            })),
            on_context: Some(Callback::new(move |(x, y): (f32, f32)| {
                // An artist press picks no item, so its menu offers no View
                // Detail.
                menu_library.open_row_menu_without_detail_at(
                    item_id.clone().map(LibraryState::row_menu_item),
                    x,
                    y,
                );
            })),
        }
    }
}

/// A nested album row under an open artist: the canvas's tree node, which keeps
/// its own painter and height (gui-delineated-rows design D1). It is a tree
/// parent (gui-tree-double-click design D2, D4): its press picks the album, its
/// double-click opens or closes the node, its year gives way to the hover hint,
/// and its indent is the row inset alone — no caret column. It passes the
/// artist's guide through and, while it shows track rows, starts its own under
/// its label (gui-tree-indent-guides design D2, D3).
fn paint_album_row(row: &LibraryRow, library: &LibraryState, picked_row: bool) -> Element {
    let LibraryRow::Album {
        base,
        name,
        year,
        open,
        shows_children,
        ..
    } = row
    else {
        return rsx! {};
    };
    let color = canvas_var("text");
    let year = (!year.is_empty()).then(|| year.clone());
    // The nested inset clears the artist's guide that passes through the row
    // (gui-tree-indent-guides design D2).
    let style = format!(
        "height: {NODE_ROW_HEIGHT}px; padding: 0 {ROW_PADDING_PX}px; padding-left: {NESTED_INSET_X}px; gap: 8px; border: 0; border-radius: {}; text-align: left; font-size: 14.5px; cursor: pointer;",
        if picked_row { "4px" } else { "0" }
    );
    let press_row = row.clone();
    let press_library = library.clone();
    let toggle_row = row.clone();
    let toggle_library = library.clone();
    let menu_row = row.clone();
    let menu_library = library.clone();
    let item_id = LibraryState::action_item_id(row);
    rsx! {
        button {
            "type": "button",
            class: "tree-parent relative flex w-full items-center",
            style: "{style}",
            "aria-expanded": "{open}",
            "data-library-row": "album",
            "data-library-row-id": "{base.id}",
            onclick: move |_| press_library.press_library_row(&press_row),
            oncontextmenu: move |event: MouseEvent| {
                event.prevent_default();
                // A right-click selects the row first, as a press does, so the
                // menu's View Detail opens the Hero for this row.
                menu_library.press_library_row(&menu_row);
                let coordinates = event.client_coordinates();
                menu_library.open_row_menu_at(
                    item_id.clone().map(LibraryState::row_menu_item),
                    f32_from_f64(coordinates.x),
                    f32_from_f64(coordinates.y),
                );
            },
            ondoubleclick: move |_| toggle_library.toggle_library_node(&toggle_row),
            Truncate {
                text: name.clone(),
                class: "min-w-0 flex-1".to_owned(),
                css: format!("color: {color}; font-weight: 400.0;"),
                style: TextStyle::sans(14.5, 400.0),
            }
            if let Some(year) = year {
                // A year repeats down the tree, so it paints the date
                // token at the repeat opacity; the picked album keeps
                // its own ink (design D14). The hover hint replaces it while
                // the pointer rests on the row (gui-tree-double-click D3).
                span {
                    class: "tree-trailing flex-none font-mono tabular-nums",
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
            {nested_parent_guides(NODE_ROW_HEIGHT, *shows_children)}
            TreeHint { open: *open }
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

    // The selected collection names in selector order (design D6): the
    // core's one derivation, `GuiCollections::selected_names` over the carried
    // selector, so the filter's placeholder names the selection the Status
    // bar's location names too.
    let collection_names = frame
        .map(|frame| frame.collections.selected_names())
        .unwrap_or_default();

    // The collection dropdown the header paints on the title's line (design
    // D6a): the destination's carried selector, which names the selection
    // itself, so the header paints no selection label beside it.
    let collections = frame.map(|frame| frame.collections.clone());

    // The filter field's placeholder, naming the selection as the canvas
    // does.
    let filter_placeholder = match collection_names.as_slice() {
        [] => "Filter All".to_owned(),
        names => format!("Filter {}", names.join(" + ")),
    };

    // The rail's A–Z letters over the painted rows.
    let rail = letter_rail(&rows);

    // The list's two DOM facts report to the scroll store (design D9), the
    // artist rows in view report to the core for their row art fetches
    // (gui-delineated-rows design D7), and the rows paint over the interaction
    // state this render read.
    let measure_library = library.clone();
    let scroll_library = library.clone();
    let art_library = library.clone();
    let paint_library = library.clone();
    let virtual_list = library_virtual_list(
        rows.as_ref(),
        scroll,
        "",
        PointerList::Music,
        "0 0 0 8px",
        Callback::new(move |height: f32| measure_library.measure_library_list(height)),
        Callback::new(move |top: f32| scroll_library.scroll_library_list(top)),
        Some(Callback::new(move |ids: Vec<String>| {
            art_library.report_rows_in_view(GuiRowArtScopeFrame::Music, ids);
        })),
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
                    // Music is an Emby destination: its Emby mark names it
                    // (gui-library-header-service-marks design D5).
                    heading: LibraryListHeading::Mark(ServiceMark::Emby),
                    // The carried selector names the selection itself, so the
                    // header paints no selection label (design D6a).
                    label: String::new(),
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
