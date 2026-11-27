//! The Favourites list (emby-favourites design D6): the rows the Music, Shows,
//! and plain Emby destinations paint in their list column while their view is
//! Favourites, over the shared flat-row parts. The rows are the plain library's
//! own row frame, carried on the destination's list part as `GuiFavouritesList`
//! (design D6), so this component paints one list for all three destinations and
//! no destination keeps a Favourites painter of its own.
//!
//! The rows paint at the shared top-level row height over native scroll, and
//! only the rows in view paint. The list paints no letter heading, no letter
//! rail, no tree button, no collection dropdown, and no sort control (design
//! D5, D6): its rows carry no rail letter, so the column is the shell alone. Its
//! own scroll and filter text per library live beside the carried frames
//! ([`crate::window::library::favourites`]), never in the destination's All
//! stores, so pressing All finds the All list's selection and scroll as they
//! were (design D7).
//!
//! A press picks the row for the Favourites Hero, a double-click plays it, and a
//! right-press opens the row menu, all by the row's stable Service id through
//! the shared commands (design D6): the core resolves the id against the
//! favourites store while the view is Favourites. The header's Refresh button
//! refetches the favourites list through the destination's refresh command,
//! which the core routes to the favourites fetch while the view is Favourites
//! (design D5).

use std::sync::Arc;

use dioxus_native::prelude::*;

use crate::bridge::frame::library::GuiEmbyRowKindFrame;
use crate::bridge::library::GuiRowArtScopeFrame;
use crate::gui_core::GuiLibraryView;
use crate::window::components::library_list_column::{
    ListColumnBody, ListColumnWiring, library_list_column,
};
use crate::window::components::library_list_header::LibraryListHeaderProps;
use crate::window::components::list_frame_notices::{ListFrameNoticeLine, ListFrameNoticesProps};
use crate::window::components::list_row::{FlatRow, LibraryRowThumb, LibraryTrailPart};
use crate::window::library::favourites::{FavouritesRow, favourites_part, favourites_rows};
use crate::window::library::state::LibraryState;
use crate::window::pointer_target::PointerList;
use crate::window::service_mark::ServiceMark;

/// One painted Favourites row: the shared top-level row part over the carried
/// row's two title lines, its art thumb, and its play state. A press picks the
/// row for the Favourites Hero, a double-click plays it, and a right-press opens
/// the row menu. Its hover offers Play and Add on every row, and Shuffle on a
/// `Collection` row alone (gui-row-hover-actions design D3): a favourite
/// collection row's shuffle runs its own menu entry, and a favourite music
/// artist row's Play, Add, and Shuffle run the artist's tracks (design D8).
fn paint_favourites_row(
    row: &FavouritesRow,
    library: &LibraryState,
    selected: Option<&str>,
) -> Element {
    let selected_row = selected == Some(row.id.as_str());
    let item = &row.row;
    let press_id = row.id.clone();
    let press_library = library.clone();
    let play_id = row.id.clone();
    let play_library = library.clone();
    let hover = library.item_hover_presses(
        row.id.clone(),
        LibraryState::play_favourites_row,
        LibraryState::queue_favourites_row,
        item.kind == GuiEmbyRowKindFrame::Collection,
    );
    let menu_id = row.id.clone();
    let menu_library = library.clone();
    rsx! {
        FlatRow {
            attr: "favourites",
            kind: item.kind.kind_word().to_owned(),
            id: row.id.clone(),
            title: item.title.clone(),
            subtitle: item.subtitle.clone(),
            trail: item.date.clone(),
            trail_part: LibraryTrailPart::Date,
            play: Some(item.state.clone()),
            selected: selected_row,
            thumb: Some(LibraryRowThumb {
                art: item.art.clone(),
                media: item.media,
            }),
            on_press: Callback::new(move |()| press_library.press_favourites_row(&press_id)),
            on_play: Callback::new(move |()| play_library.play_favourites_row(&play_id)),
            on_hover_play: hover.play,
            on_hover_queue: hover.queue,
            on_hover_shuffle: hover.shuffle,
            // The shared row part presses on a right-click before it calls
            // `on_context`, so the menu's View Detail shows this row.
            on_context: Some(Callback::new(move |(x, y): (f32, f32)| {
                menu_library.open_row_menu_at(
                    Some(LibraryState::row_menu_item(menu_id.clone())),
                    x,
                    y,
                );
            })),
        }
    }
}

/// The list has not landed yet, or it landed with no favourite: one line per
/// state. A failed first load lands an empty list, so the list paints no failure
/// line (design D6).
fn favourites_notices(loading: bool, empty: bool) -> ListFrameNoticesProps {
    ListFrameNoticesProps {
        loading: loading.then(|| ListFrameNoticeLine::loading("data-library-favourites-loading")),
        failed: None,
        empty: empty
            .then(|| ListFrameNoticeLine::new("data-library-favourites-empty", "No favourites")),
    }
}

/// The Favourites list: the header with the destination's view dropdown, the
/// load and empty lines, and the rows over native scroll.
#[component]
pub fn LibraryFavouritesList() -> Element {
    let library = consume_context::<LibraryState>();

    // The carried list section and the Favourites part it holds (design D6):
    // the site names the destination whose view dropdown and stores the list
    // uses, so one component paints all three destinations' Favourites view.
    let carried = library.sections.library_list.cloned();
    let Some((site, part)) = favourites_part(carried.as_deref()) else {
        return rsx! {};
    };
    let lib_idx = site.lib_idx();
    let loading = part.loading;
    let selected = part.selected.clone();

    // The list's own scroll and filter text (design D6, D7), and the rows they
    // leave: the filter narrows the list alone, so the destination's All list
    // keeps its own text, selection, and scroll.
    let view = library.favourites_view(lib_idx);
    let rows: Arc<[FavouritesRow]> = favourites_rows(part, &view.filter).into();
    let empty = !loading && rows.is_empty();

    // The list's two DOM facts report to the list's own store, and the rows in
    // view report the Favourites scope to the core for their row art fetches
    // (gui-delineated-rows design D7).
    let measure_library = library.clone();
    let measure_rows = Arc::clone(&rows);
    let measure_selected = selected.clone();
    let scroll_library = library.clone();
    let scroll_rows = Arc::clone(&rows);
    let art_library = library.clone();
    let wiring = ListColumnWiring {
        rows: &rows,
        view: view.scroll,
        attr: "favourites",
        list: PointerList::Favourites,
        padding: "8px 0 8px 8px",
        on_measure: Callback::new(move |height: f32| {
            measure_library.measure_favourites_list(
                lib_idx,
                height,
                measure_selected.as_deref(),
                &measure_rows,
            );
        }),
        on_scroll: Callback::new(move |top: f32| {
            scroll_library.scroll_favourites_list(lib_idx, top, &scroll_rows);
        }),
        rows_in_view: Some(Callback::new(move |ids: Vec<String>| {
            art_library.report_rows_in_view(GuiRowArtScopeFrame::Favourites, ids);
        })),
    };

    let paint_library = library.clone();
    let paint_selected = selected;

    let filter_library = library.clone();
    let accept_library = library.clone();
    // The header's Refresh button (design D5, D6): the destination's refresh
    // command, which the core routes to the favourites refetch while the view
    // is Favourites, so the All list's refresh path does not run.
    let refresh_library = library.clone();
    // The header's facts (design D5): the Emby mark names the destination, the
    // view dropdown marks Favourites, and the list paints no collection
    // dropdown, no tree button, and no sort control.
    let header = LibraryListHeaderProps {
        heading: ServiceMark::Emby.into(),
        label: String::new(),
        label_attr: None,
        collections: None,
        view: Some((GuiLibraryView::Favourites, site.destination())),
        sort: None,
        on_refresh: Callback::new(move |()| refresh_library.refresh_library_list()),
        refreshing: carried.as_ref().is_some_and(|list| list.refreshing),
        tree: None,
        filter: view.filter.clone(),
        placeholder: "Filter favourites".to_owned(),
        aria_label: "Filter favourites".to_owned(),
        on_input: Callback::new(move |value: String| {
            filter_library.set_favourites_filter(lib_idx, value);
        }),
        on_accept: Callback::new(move |()| accept_library.accept_destination_filter()),
    };

    let notices = Some(favourites_notices(loading, empty));

    library_list_column(
        &wiring,
        move |row: &FavouritesRow| {
            paint_favourites_row(row, &paint_library, paint_selected.as_deref())
        },
        header,
        notices,
        ListColumnBody::Shell,
    )
}
