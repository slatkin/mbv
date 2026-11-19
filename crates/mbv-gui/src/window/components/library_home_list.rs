//! The Home list (`replace-tauri-with-blitz` task 7.7, the port of
//! `LibraryHomeList`; add-gui-library-panel design D12, the
//! USER-APPROVED GUI Home design): the flat list in the Music list slot —
//! the two sections' heading rows, the item rows with their date gutter and
//! their art thumb when the row carries art, and the playing state's
//! styling. No letter rail: Home paints none.
//!
//! The rows paint at the fixed heights of [`crate::window::library::home_list`]
//! over native scroll, and only the rows in view paint, so a long list stays
//! fast. The interaction state is the window content's own in
//! [`crate::window::library::home`] (design D1), the scroll offset and the
//! viewport are the Home list's scroll store (design D9); this component paints
//! that state and reports the two facts only the DOM holds. A press selects the
//! row, and a double-click plays it (design D12). The list keeps no cursor: the
//! keys act on the row the pointer marks (`gui-pointer-target-interaction`
//! D2).

use std::fmt::Write as _;

use dioxus_native::prelude::*;

use crate::art::GuiArtRef;
use crate::bridge::frame::library::{GuiHomeRowIdFrame, GuiHomeRowStateFrame};
use crate::window::components::art_image::ArtImage;
use crate::window::components::library_item_row::{mark, name_color};
use crate::window::components::library_list_header::LibraryListHeader;
use crate::window::components::library_virtual_list::library_virtual_list;
use crate::window::components::list_frame_notices::{ListFrameNoticeLine, ListFrameNotices};
use crate::window::input::f32_from_f64;
use crate::window::library::home_list::{HomeItemRow, HomeListRow, home_row_key};
use crate::window::library::parts::{library_home_detail, library_home_list};
use crate::window::library::state::LibraryState;
use crate::window::pointer_target::PointerList;
use crate::window::theme::canvas_var;
use crate::window::truncate::{TextStyle, Truncate};

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

/// The artwork kind word the art box's hook paints (`data-library-home-art`).
fn art_kind(art: &GuiArtRef) -> &'static str {
    match art {
        GuiArtRef::Image { .. } => "image",
        GuiArtRef::Placeholder { .. } => "placeholder",
        GuiArtRef::Loading => "loading",
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
        HomeListRow::Heading { text, height, .. } => rsx! {
            div {
                class: "flex items-end",
                style: "height: {height}px; padding: 0 8px 4px; font-size: 14px; font-weight: 600; color: var(--canvas-heading);",
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

/// One painted item row: the art thumb when the row
/// carries art, the two-line name, the date gutter, and the resume run.
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
    let mut style = format!(
        "height: {height}px; padding: 0 10px; gap: 10px; border: 0; border-radius: 4px; text-align: left; font-size: 14.5px; cursor: pointer;"
    );
    if selected_row {
        let _ = write!(style, " background: {};", canvas_var("library-raise"));
    }
    // The resume percent an active item row paints, or `None`.
    let progress = match &item.state {
        GuiHomeRowStateFrame::Active { progress } => *progress,
        _ => None,
    };
    let title_color = name_color(&item.state);
    // Every flat list's top-level row carries the Music artist and TV series
    // root weight: 600, selected or not. Only nested rows stay at 400.
    let title_weight = 600.0;
    let art = item.art.clone();
    let art_kind = art.as_ref().map(art_kind);
    let press_row = row.clone();
    let press_library = library.clone();
    let play_id = item.row_id.clone();
    let play_library = library.clone();
    let menu_item_id = home_menu_item_id(&item.row_id);
    let menu_library = library.clone();
    rsx! {
        button {
            "type": "button",
            class: "relative flex w-full items-center overflow-hidden",
            style: "{style}",
            "data-library-home-row": "item",
            "data-library-home-row-id": "{id}",
            onclick: move |_| press_library.press_home_row(&press_row),
            ondoubleclick: move |_| play_library.play_home_row(play_id.clone()),
            oncontextmenu: move |event: MouseEvent| {
                event.prevent_default();
                let coordinates = event.client_coordinates();
                menu_library.open_row_menu_at(
                    menu_item_id.clone(),
                    f32_from_f64(coordinates.x),
                    f32_from_f64(coordinates.y),
                );
            },
            ..mark("data-library-home-selected", selected_row),
            if let Some(art) = art {
                // The row's card art (design D12): the frame's image
                // reference, or the empty box while it is loading. The
                // rows without art paint no box at all.
                span {
                    class: "flex flex-none items-center justify-center",
                    style: "width: 40px; height: 40px; background: var(--canvas-library-raise); border-radius: 4px; overflow: hidden;",
                    "data-library-home-art": art_kind.unwrap_or_default(),
                    ArtImage { art: Some(art), alt: "", class: "h-full w-full object-cover" }
                }
            }
            span {
                class: "flex min-w-0 flex-1 flex-col",
                style: "gap: 2px;",
                Truncate {
                    text: item.title.clone(),
                    css: format!("color: {title_color}; font-weight: {title_weight};"),
                    style: TextStyle::sans(14.5, title_weight),
                    attrs: vec![Attribute::new("data-library-home-title", "", None, false)],
                }
                if let Some(subtitle) = item.subtitle.clone() {
                    Truncate {
                        text: subtitle,
                        css: "font-size: 12px; color: var(--canvas-muted);".to_owned(),
                        style: TextStyle::sans(12.0, 400.0),
                        attrs: vec![Attribute::new(
                            "data-library-home-subtitle",
                            "",
                            None,
                            false,
                        )],
                    }
                }
            }
            if let Some(date) = item.date.clone() {
                span {
                    class: "flex-none font-mono tabular-nums",
                    style: "font-size: 12px; color: var(--canvas-muted);",
                    "data-library-home-date": "",
                    "{date}"
                }
            }
            if let Some(progress) = progress {
                // The resume position (design D12): a thin accent run
                // under the row's bottom edge, at the frame's bounded
                // percent.
                span {
                    class: "absolute",
                    style: "left: 0; bottom: 0; height: 2px; width: {progress}%; background: var(--canvas-accent);",
                    "data-library-home-progress": "",
                }
            }
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

    // The list's two DOM facts report to the scroll store (design D9), and
    // the rows paint over the interaction state this render read.
    let measure_library = library.clone();
    let scroll_library = library.clone();
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
        move |row: &HomeListRow| paint_home_row(row, &paint_library, paint_selected.as_ref()),
    );

    let filter_library = library.clone();
    let accept_library = library.clone();

    rsx! {
        div {
            class: "flex min-h-0 min-w-0 flex-1 flex-col",
            "data-library-home-list-column": "",
            // The Music list's header (design D15): the list names itself, the
            // carried list is the whole destination, and the `/` filter field
            // narrows it.
            div {
                class: "flex flex-none flex-col",
                LibraryListHeader {
                    title: "Home".to_owned(),
                    label: "All".to_owned(),
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
