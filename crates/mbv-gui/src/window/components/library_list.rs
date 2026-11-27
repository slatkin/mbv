//! The Library list (`replace-tauri-with-blitz` task 7.2, the port of
//! `LibraryList`; add-gui-library-panel design D7): the canvas's
//! artist index in the list column — the *Artists* line over the selected
//! collection names (design D6), the `/` filter field, the canvas tree of
//! artist, album, and track rows under its letter headings, and the A–Z
//! letter rail beside it. The rows paint at the canvas's fixed heights over
//! native scroll, and only the rows in view paint, so a long artist list
//! stays fast. The artist rows are top-level rows and paint through the shared
//! row part with their art thumb; the album and track rows under them paint
//! through the nested row part at the first and second depth (gui-unified-list-
//! rows design D3). The rows it paints are also the row art set the core
//! fetches (design D7).
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
//!
//! The list is a tree, so its header paints the Expand all and Collapse all
//! buttons (gui-list-header-buttons design D1, D4): they change the window's
//! open nodes and send no command. The header's Refresh button reloads the
//! destination in place, naming the open album rows in view (design D7).

use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;

use dioxus_native::prelude::*;

use crate::bridge::frame::library::{GuiLibraryList, GuiMusicHeroFrame};
use crate::bridge::library::GuiRowArtScopeFrame;
use crate::window::components::library_letter_rail::LibraryLetterRail;
use crate::window::components::library_list_column::{
    ListColumnBody, ListColumnWiring, library_list_column,
};
use crate::window::components::library_list_header::{LibraryListHeaderProps, LibraryTreeActions};
use crate::window::components::list_frame_notices::{ListFrameNoticeLine, ListFrameNoticesProps};
use crate::window::components::list_row::{
    FlatRow, LibraryRowThumb, LibraryTrailPart, ROW_PADDING_PX, TreeDepth, TreeLevel, TreeRow,
    TreeTrail,
};
use crate::window::components::tree_guide::subtree_tails;
use crate::window::library::list::{LETTER_ROW_HEIGHT, LibraryRow, letter_rail};
use crate::window::library::parts::{library_music_detail, library_music_list};
use crate::window::library::state::LibraryState;
use crate::window::pointer_target::PointerList;
use crate::window::sections::SectionRef;
use crate::window::service_mark::ServiceMark;

/// Whether the row is the picked row the Hero shows (gui-music-artist-page
/// design D6): the album row while the subject is an album, the artist row
/// while the subject is the artist page. `picked` is the picked row's own id.
fn is_picked(row: &LibraryRow, picked: Option<&str>) -> bool {
    match row {
        LibraryRow::Album { .. } | LibraryRow::Artist { .. } => picked == Some(row.id()),
        LibraryRow::Letter(_) | LibraryRow::Track { .. } => false,
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

/// The track row under an open album: the number, the muted name, and the
/// runtime. It is a leaf of the tree at the second depth, so it paints both
/// guides that pass through it — the artist's and its album's — and the mono
/// leaf face (gui-unified-list-rows design D3). Its hover offers Play and Add
/// and no Shuffle, because a track is no collection (gui-row-hover-actions
/// design D3).
fn paint_track_row(
    row: &LibraryRow,
    library: &LibraryState,
    picked_row: bool,
    tail: bool,
) -> Element {
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
    let press_row = row.clone();
    let press_library = library.clone();
    let menu_library = library.clone();
    let play_library = library.clone();
    let item_id = LibraryState::action_item_id(row);
    let hover = library.row_hover_presses(item_id.clone(), false);
    let track_id = track_id.clone();
    rsx! {
        TreeRow {
            attr: "",
            kind: "track".to_owned(),
            id: base.id.clone(),
            level: TreeLevel::Leaf,
            depth: TreeDepth::Two,
            title: name.clone(),
            number: Some(number.to_string()),
            trail: Some(TreeTrail::Duration(duration.clone())),
            selected: picked_row,
            tail,
            on_press: Callback::new(move |()| press_library.press_library_row(&press_row)),
            on_play: Callback::new(move |()| play_library.play_library_track(&track_id)),
            on_hover_play: hover.play,
            on_hover_queue: hover.queue,
            // The part presses the row first, so the menu's View Detail opens
            // the Hero for this row.
            on_context: Some(Callback::new(move |(x, y): (f32, f32)| {
                menu_library.open_row_menu_at(
                    item_id.clone().map(LibraryState::row_menu_item),
                    x,
                    y,
                );
            })),
        }
    }
}

/// One painted row of the canvas tree: the letter heading, the nested track
/// row, the shared part's artist row (gui-delineated-rows design D1), or the
/// nested album row (gui-unified-list-rows design D3).
fn paint_library_row(
    row: &LibraryRow,
    library: &LibraryState,
    picked: Option<&str>,
    tails: &HashSet<String>,
) -> Element {
    let picked_row = is_picked(row, picked);
    let tail = tails.contains(row.id());
    match row {
        LibraryRow::Letter(base) => paint_letter_row(base.letter),
        LibraryRow::Track { .. } => paint_track_row(row, library, picked_row, tail),
        LibraryRow::Artist { .. } => paint_artist_row(row, library, picked_row),
        LibraryRow::Album { .. } => paint_album_row(row, library, picked_row, tail),
    }
}

/// A top-level artist row (gui-delineated-rows design D1): the shared top-level
/// row part over the artist's name and its art thumb. It is a tree parent
/// (gui-tree-double-click design D2, D4), so it paints no caret: its
/// double-click toggles the node and its hover offers Play, Add, and Shuffle
/// over the artist's item id, plus Expand
/// (gui-row-hover-actions design D3). An artist frame that carries no item id
/// paints Expand alone. A press picks the artist — the Hero shows its page
/// (gui-music-artist-page design D6) — and a double-click only opens it, so
/// the row names no double-click play path.
/// While it shows album rows it starts its guide under its thumb
/// (gui-tree-indent-guides design D3).
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
    let hover = library.row_hover_presses(item_id.clone(), true);
    rsx! {
        FlatRow {
            attr: "",
            kind: "artist".to_owned(),
            id: base.id.clone(),
            title: name.clone(),
            subtitle: None,
            trail: None,
            trail_part: LibraryTrailPart::Date,
            selected: picked_row,
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
            on_hover_play: hover.play,
            on_hover_queue: hover.queue,
            on_hover_shuffle: hover.shuffle,
            // The part presses the row first, so the menu's View Detail opens
            // the Hero for this row — the artist page it picks
            // (gui-music-artist-page design D6, spec "The overlay shows the
            // artist page").
            on_context: Some(Callback::new(move |(x, y): (f32, f32)| {
                menu_library.open_row_menu_at(
                    item_id.clone().map(LibraryState::row_menu_item),
                    x,
                    y,
                );
            })),
        }
    }
}

/// A nested album row under an open artist: a tree parent at the first depth,
/// so it paints the Music album look — its press picks the album, its
/// double-click opens or closes the node, its year paints the parent trail's
/// date, and the hover controls replace it (gui-unified-list-rows design D3;
/// gui-row-hover-actions design D3). The album is a collection that names its
/// own item, so it offers Play, Add, Shuffle, and Expand.
fn paint_album_row(
    row: &LibraryRow,
    library: &LibraryState,
    picked_row: bool,
    tail: bool,
) -> Element {
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
    let press_row = row.clone();
    let press_library = library.clone();
    let toggle_row = row.clone();
    let toggle_library = library.clone();
    let menu_library = library.clone();
    let item_id = LibraryState::action_item_id(row);
    let hover = library.row_hover_presses(item_id.clone(), true);
    rsx! {
        TreeRow {
            attr: "",
            kind: "album".to_owned(),
            id: base.id.clone(),
            level: TreeLevel::Parent,
            depth: TreeDepth::One,
            title: name.clone(),
            trail: Some(TreeTrail::Date(year.clone())),
            selected: picked_row,
            open: Some(*open),
            shows_children: *shows_children,
            tail,
            on_press: Callback::new(move |()| press_library.press_library_row(&press_row)),
            on_play: Callback::new(|()| {}),
            on_toggle: Some(Callback::new(move |()| {
                toggle_library.toggle_library_node(&toggle_row);
            })),
            on_hover_play: hover.play,
            on_hover_queue: hover.queue,
            on_hover_shuffle: hover.shuffle,
            // The part presses the row first, so the menu's View Detail opens
            // the Hero for this row.
            on_context: Some(Callback::new(move |(x, y): (f32, f32)| {
                menu_library.open_row_menu_at(
                    item_id.clone().map(LibraryState::row_menu_item),
                    x,
                    y,
                );
            })),
        }
    }
}

/// Whether the carried list section marks the active destination's refresh
/// in flight (the header's refresh busy fact, design D7).
fn carried_refreshing(list: Option<&SectionRef<GuiLibraryList>>) -> bool {
    list.is_some_and(|list| list.refreshing)
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

    // The row the Hero shows: the list paints it in the canvas's picked fill
    // (design D7) — the album row for an album subject, the artist row for an
    // artist page (gui-music-artist-page design D6).
    let picked = detail
        .and_then(|detail| detail.hero.as_ref())
        .map(GuiMusicHeroFrame::picked_row_id);

    // The picked row the list last revealed. A pick reveals once, so a later
    // frame carrying the same pick does not move the cursor again. The
    // effect reads the carried frames and writes no signal it reads — the
    // reveal moves the interaction state and the scroll store; the `Rc`
    // shares the one reveal record across the effect's re-runs.
    let revealed_row = use_hook(|| Rc::new(RefCell::new(None::<String>)));
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
        // The picked row's id (gui-music-artist-page design D6): the reveal
        // keys on it, so a pick of either subject reveals once.
        let picked_row_id = detail.hero.as_ref().map(GuiMusicHeroFrame::picked_row_id);
        let mut revealed = revealed_row.borrow_mut();
        if picked_row_id.is_none() || picked_row_id == *revealed {
            return;
        }
        *revealed = picked_row_id;
        drop(revealed);
        reveal_library.reveal_picked_row(list, detail);
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
    // state this render read. The column's wiring is the one scaffold every
    // list paints through (design D14).
    let measure_library = library.clone();
    let scroll_library = library.clone();
    let art_library = library.clone();
    let paint_library = library.clone();
    // The rows that close an open artist's subtree paint its divider.
    let tails = subtree_tails(&rows, |row| {
        matches!(row, LibraryRow::Album { .. } | LibraryRow::Track { .. })
    });
    let wiring = ListColumnWiring {
        rows: rows.as_ref(),
        view: scroll,
        attr: "",
        list: PointerList::Music,
        padding: "0 0 0 8px",
        on_measure: Callback::new(move |height: f32| measure_library.measure_library_list(height)),
        on_scroll: Callback::new(move |top: f32| scroll_library.scroll_library_list(top)),
        rows_in_view: Some(Callback::new(move |ids: Vec<String>| {
            art_library.report_rows_in_view(GuiRowArtScopeFrame::Music, ids);
        })),
    };

    let filter_library = library.clone();
    let accept_library = library.clone();
    // The tree's two header presses (gui-list-header-buttons design D1, D4):
    // Music is a tree list, so its All view paints both. The Favourites view
    // paints no tree buttons (emby-favourites design D5, the title line's
    // order).
    let expand_library = library.clone();
    let collapse_library = library.clone();
    let tree = frame.is_some().then(|| LibraryTreeActions {
        on_expand_all: Callback::new(move |()| expand_library.expand_all_music()),
        on_collapse_all: Callback::new(move |()| collapse_library.collapse_all_music()),
    });
    // The header's Refresh button (gui-list-header-buttons design D7): Music
    // names the open album rows its window holds, so the core reloads exactly
    // those tracklists.
    let refresh_library = library.clone();
    // The header's facts (design D14): Music is an Emby destination, so its Emby
    // mark names it (gui-library-header-service-marks design D5), and the
    // carried selector names the selection itself, so it paints no selection
    // label (design D6a).
    let header = LibraryListHeaderProps {
        heading: ServiceMark::Emby.into(),
        label: String::new(),
        label_attr: None,
        collections,
        // A Grouped Music destination is excluded from favourites
        // everywhere (emby-favourites design D11), so its header paints no
        // view dropdown: the collection dropdown is its only one.
        view: None,
        sort: None,
        on_refresh: Callback::new(move |()| refresh_library.refresh_music_list()),
        refreshing: carried_refreshing(carried.as_ref()),
        tree,
        filter: ui.filter.clone(),
        placeholder: filter_placeholder,
        aria_label: "Filter artists".to_owned(),
        on_input: Callback::new(move |value: String| filter_library.set_library_filter(value)),
        on_accept: Callback::new(move |()| accept_library.accept_library_filter()),
    };

    // A selected group's album list has not landed yet (design D5), the
    // selected groups' album lists failed, or the collections hold no artist:
    // one line per state, the failure line naming the groups the frame names.
    let notices = frame.map(|frame| ListFrameNoticesProps {
        loading: frame
            .loading
            .then(|| ListFrameNoticeLine::loading("data-library-loading")),
        failed: frame
            .failure
            .clone()
            .map(|failure| ListFrameNoticeLine::new("data-library-failure", failure)),
        empty: (!frame.loading && frame.artists.is_empty())
            .then(|| ListFrameNoticeLine::new("data-library-empty", "No artists")),
    });

    library_list_column(
        &wiring,
        |row: &LibraryRow| paint_library_row(row, &paint_library, picked.as_deref(), &tails),
        header,
        notices,
        ListColumnBody::Rail(Some(rsx! {
            LibraryLetterRail { letters: rail }
        })),
    )
}

#[cfg(test)]
mod tests;
