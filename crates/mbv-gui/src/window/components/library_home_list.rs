//! The Home list (`replace-tauri-with-blitz` task 7.7, the port of
//! `LibraryHomeList`; add-gui-library-panel design D12, the
//! USER-APPROVED GUI Home design; gui-delineated-rows design D1, D2): the flat
//! list in the Music list slot — the sections' heading rows and the item rows,
//! which paint through the shared top-level row part with its art thumb, their
//! trailing text, and their resume run. No letter rail: Home paints none.
//!
//! The rows paint at the fixed heights of [`crate::window::library::home_list`]
//! over native scroll, and only the rows in view paint, so a long list stays
//! fast. The interaction state is the window content's own in
//! [`crate::window::library::home`] (design D1), the scroll offset and the
//! viewport are the Home list's scroll store (design D9); this component paints
//! that state and reports the two facts only the DOM holds. A press selects the
//! row, and a double-click plays it (design D12). The list keeps no cursor: the
//! keys act on the row the pointer marks (`gui-pointer-target-interaction`
//! D2). The rows it paints are also the row art set the core fetches
//! (gui-delineated-rows design D7).

use dioxus_native::prelude::*;

use crate::bridge::frame::library::{GuiHomeRowIdFrame, GuiHomeRowStateFrame};
use crate::bridge::library::GuiRowArtScopeFrame;
use crate::window::components::library_item_row::{
    LibraryItemRow, LibraryRowThumb, LibraryTrailPart, name_color,
};
use crate::window::components::library_list_header::{LibraryListHeader, LibraryListHeading};
use crate::window::components::library_virtual_list::library_virtual_list;
use crate::window::components::list_frame_notices::{ListFrameNoticeLine, ListFrameNotices};
use crate::window::library::home_list::{HomeItemRow, HomeListRow, home_row_key};
use crate::window::library::parts::{library_home_detail, library_home_list};
use crate::window::library::state::LibraryState;
use crate::window::pointer_target::PointerList;
use crate::window::theme::canvas_var;

/// The item id a row's right-click menu opens for (design D8, D15): the
/// `Continue watching` row's Emby item id, which the core resolves for the
/// shared menu. A `Recently added` row's queue-item content id names no
/// single Service item the menu resolves, so it opens no menu. An `Upcoming`
/// row is display-only (gui-collection-dropdown design D9), so it opens no
/// menu even though its key is a string.
fn home_menu_item_id(id: &GuiHomeRowIdFrame) -> Option<String> {
    match id {
        GuiHomeRowIdFrame::Continue(item_id) => Some(item_id.clone()),
        GuiHomeRowIdFrame::Recent(_) => None,
    }
}

/// The row's state as the shared part reads it: the resume percent of an
/// active row.
fn row_progress(state: &GuiHomeRowStateFrame) -> Option<f32> {
    match state {
        GuiHomeRowStateFrame::Active { progress } => progress.map(f32::from),
        _ => None,
    }
}

/// One painted row of the flat list: the section
/// heading, the spacer between two non-empty sections, or the item row.
fn paint_home_row(
    row: &HomeListRow,
    library: &LibraryState,
    selected: Option<&GuiHomeRowIdFrame>,
) -> Element {
    match row {
        // The section heading (gui-delineated-rows design D2): the muted
        // uppercase label over its rows.
        HomeListRow::Heading { text, height, .. } => rsx! {
            div {
                class: "flex items-end",
                style: "height: {height}px; padding: 0 8px 4px; font-size: 12px; font-weight: 600; text-transform: uppercase; letter-spacing: 0.06em; color: var(--canvas-muted);",
                "data-library-home-heading": "{text}",
                "{text}"
            }
        },
        // The spacer between two non-empty sections (design D12).
        HomeListRow::Spacer { height, .. } => rsx! {
            div { style: "height: {height}px;", "aria-hidden": "true" }
        },
        HomeListRow::Item {
            id,
            height,
            row: item,
            ..
        } => paint_home_item_row(id, *height, item, row, library, selected),
    }
}

/// One painted item row: the shared top-level row part over the carried row —
/// its art thumb, the two-line name, the trailing text, and the resume run
/// inside the thumb (gui-delineated-rows design D1).
fn paint_home_item_row(
    id: &str,
    height: f32,
    item: &HomeItemRow,
    row: &HomeListRow,
    library: &LibraryState,
    selected: Option<&GuiHomeRowIdFrame>,
) -> Element {
    // The core's selected row paints the raised fill: the selection is what the
    // Hero shows, so its row stays marked, as every Library list's selected row
    // is.
    let selected_row = selected.is_some_and(|picked| home_row_key(picked) == id);
    let press_row = row.clone();
    let press_library = library.clone();
    let play_id = item.row_id.clone();
    let play_library = library.clone();
    let menu_item_id = home_menu_item_id(&item.row_id);
    let menu_library = library.clone();
    rsx! {
        LibraryItemRow {
            attr: "home",
            kind: "item".to_owned(),
            id: id.to_owned(),
            height,
            title: item.title.clone(),
            subtitle: item.subtitle.clone(),
            // The trailing text rides the row's right-hand gutter: the Continue
            // watching time left, or the Recently added row's date (design D9).
            trail: item.trailing.clone(),
            trail_part: LibraryTrailPart::Date,
            title_color: name_color(&item.state),
            background: selected_row.then(|| canvas_var("library-raise")),
            selected: selected_row,
            progress: row_progress(&item.state),
            thumb: Some(LibraryRowThumb {
                art: item.art.clone(),
                media: item.media,
            }),
            on_press: Callback::new(move |()| press_library.press_home_row(&press_row)),
            on_play: Callback::new(move |()| play_library.play_home_row(play_id.clone())),
            on_context: Some(Callback::new(move |(x, y): (f32, f32)| {
                menu_library.open_row_menu_at(menu_item_id.clone(), x, y);
            })),
        }
    }
}

/// The Home list: the header,
/// the load and empty lines, and the flat list's rows over native scroll.
#[component]
pub fn LibraryHomeList() -> Element {
    let library = consume_context::<LibraryState>();

    // The carried Home content and the core's selection (design D12): the
    // clones subscribe the list to the frames that carry them, as the Music
    // list's read does; the row model borrows the frame from the section
    // store's own storage through the memo's revision key.
    let carried = library.sections.library_list.cloned();
    let detail_section = library.sections.library_detail.cloned();
    let frame = library_home_list(carried.as_deref());
    let detail = library_home_detail(detail_section.as_deref());

    // The interaction state and the scroll store this list paints (design
    // D1, D9). The state clone subscribes this render to its changes.
    let home_ui = library.home_ui.cloned();
    let scroll = library.scrolls.home.cloned();

    // The painted rows of the flat list (design D12): the same model
    // [`crate::window::library::home`] builds from the carried content,
    // narrowed by the filter's text.
    let filter = home_ui.filter.clone();
    let rows = library.home_rows(&filter);

    // The list's two DOM facts report to the scroll store (design D9), the
    // rows in view report to the core for its row art fetches (design D7),
    // and the rows paint over the interaction state this render read.
    let measure_library = library.clone();
    let scroll_library = library.clone();
    let art_library = library.clone();
    let paint_library = library.clone();
    let paint_selected = detail.and_then(|detail| detail.selected.clone());
    let virtual_list = library_virtual_list(
        rows.as_ref(),
        scroll,
        "home",
        PointerList::Home,
        "8px 8px 8px 8px",
        Callback::new(move |height: f32| measure_library.measure_home_list(height)),
        Callback::new(move |top: f32| scroll_library.scroll_home_list(top)),
        Some(Callback::new(move |ids: Vec<String>| {
            art_library.report_rows_in_view(GuiRowArtScopeFrame::Home, ids);
        })),
        move |row: &HomeListRow| paint_home_row(row, &paint_library, paint_selected.as_ref()),
    );

    let filter_library = library.clone();
    let accept_library = library.clone();

    rsx! {
        div {
            class: "flex min-h-0 min-w-0 flex-1 flex-col",
            "data-library-home-list-column": "",
            // The Music list's header (design D15): Home keeps its text title
            // (gui-library-header-service-marks design D5), the whole destination
            // is the default scope so it paints no label, and the `/` filter
            // field narrows it.
            div {
                class: "flex flex-none flex-col",
                LibraryListHeader {
                    heading: LibraryListHeading::Title("Home".to_owned()),
                    label: String::new(),
                    filter: filter.clone(),
                    placeholder: "Filter home".to_owned(),
                    aria_label: "Filter home".to_owned(),
                    on_input: move |value: String| filter_library.set_home_filter(value),
                    on_accept: move |()| accept_library.accept_destination_filter(),
                }
            }
            if let Some(frame) = frame {
                // The Home content has not landed yet (design D12), or it holds
                // nothing to play: one line per state.
                ListFrameNotices {
                    loading: frame.loading.then(|| {
                        ListFrameNoticeLine::loading("data-library-home-loading")
                    }),
                    empty: (!frame.loading && frame.rows.is_empty()).then(|| {
                        ListFrameNoticeLine::new("data-library-home-empty", "Nothing to play")
                    }),
                }
            }
            {virtual_list}
        }
    }
}

#[cfg(test)]
mod tests;
