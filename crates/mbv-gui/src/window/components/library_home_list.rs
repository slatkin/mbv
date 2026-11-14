//! The Home list (`replace-tauri-with-blitz` task 7.7, the port of
//! `LibraryHomeList.svelte`; add-gui-library-panel design D12, the
//! USER-APPROVED GUI Home design): the flat list in the Music list slot —
//! the two sections' heading rows, the item rows with their date gutter and
//! their art thumb when the row carries art, and the playing state's
//! styling. No letter rail: Home paints none.
//!
//! The rows paint at the fixed heights of [`crate::window::library::home_list`]
//! over native scroll, and only the rows in view paint, so a long list stays
//! fast. The cursor is the window content's own in
//! [`crate::window::library::home`] (design D1), the scroll offset and the
//! viewport are the Home list's scroll store (design D9); this component
//! paints that state and reports the two facts only the DOM holds. A press
//! selects the row, and a double-click plays it (design D12): the keys'
//! Enter plays the cursor row, and the press is the selection's half.

use std::fmt::Write as _;

use dioxus_native::prelude::*;

use crate::art::GuiArtRef;
use crate::bridge::frame::library::{GuiHomeRowIdFrame, GuiHomeRowStateFrame};
use crate::window::components::art_image::ArtImage;
use crate::window::components::library_list_header::LibraryListHeader;
use crate::window::components::library_list_notice::LibraryListNotice;
use crate::window::components::library_virtual_list::library_virtual_list;
use crate::window::input::f32_from_f64;
use crate::window::library::home_list::{HomeItemRow, HomeListRow, home_row_key};
use crate::window::library::parts::{library_home_detail, library_home_list};
use crate::window::library::state::LibraryState;
use crate::window::theme::canvas_var;

/// The item id a row's right-click menu opens for (design D8, D15): the
/// `Continue watching` row's Emby item id, which the core resolves for the
/// shared menu. A `Recently added` row's queue-item content id names no
/// single Service item the menu resolves, so it opens no menu.
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

/// The mark a row paints only while it holds the state (the Svelte
/// `undefined` values painted nothing).
fn mark(name: &'static str, on: bool) -> Vec<Attribute> {
    on.then(|| Attribute::new(name, "", None, false))
        .into_iter()
        .collect()
}

/// An item row's name colour: the canvas's text tone, dimmed on a played
/// row (design D12).
fn name_color(state: &GuiHomeRowStateFrame) -> String {
    if *state == GuiHomeRowStateFrame::Played {
        canvas_var("muted")
    } else {
        canvas_var("text")
    }
}

/// One painted row of the flat list (the Svelte row snippet): the section
/// heading, the spacer between two non-empty sections, or the item row.
fn paint_home_row(
    row: &HomeListRow,
    library: &LibraryState,
    cursor: Option<&str>,
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
        } => paint_home_item_row(id, *height, item, row, library, cursor, selected),
    }
}

/// One painted item row (the Svelte item branch): the art thumb when the row
/// carries art, the two-line name, the date gutter, and the resume run.
fn paint_home_item_row(
    id: &str,
    height: f32,
    item: &HomeItemRow,
    row: &HomeListRow,
    library: &LibraryState,
    cursor: Option<&str>,
    selected: Option<&GuiHomeRowIdFrame>,
) -> Element {
    // The cursor row's focus fill, and the core's selected row the
    // raised fill while the cursor sits elsewhere — the selection is
    // what the Hero shows, so its row stays marked, as every Library
    // list's selected row is.
    let cursor_row = cursor == Some(id);
    let selected_row = selected.is_some_and(|picked| home_row_key(picked) == id);
    let mut style = format!(
        "height: {height}px; padding: 0 10px; gap: 10px; border: 0; border-radius: 4px; text-align: left; font-size: 14px; cursor: pointer;"
    );
    if cursor_row {
        let _ = write!(style, " background: {};", canvas_var("selected"));
    } else if selected_row {
        let _ = write!(style, " background: {};", canvas_var("library-raise"));
    }
    // The resume percent an active item row paints, or `None`.
    let progress = match &item.state {
        GuiHomeRowStateFrame::Active { progress } => *progress,
        _ => None,
    };
    let title_color = name_color(&item.state);
    let title_weight = if selected_row { 600 } else { 400 };
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
            ..mark("data-library-home-cursor", cursor_row),
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
                span {
                    class: "truncate",
                    style: "color: {title_color}; font-weight: {title_weight};",
                    "data-library-home-title": "",
                    "{item.title}"
                }
                if let Some(subtitle) = item.subtitle.clone() {
                    span {
                        class: "truncate",
                        style: "font-size: 12px; color: var(--canvas-muted);",
                        "data-library-home-subtitle": "",
                        "{subtitle}"
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

/// The Home list (the Svelte markup, character for character): the header,
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
    let frame = library_home_list(carried.as_ref());
    let detail = library_home_detail(detail_section.as_ref());

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
    let paint_cursor = home_ui.cursor.clone();
    let paint_selected = detail.and_then(|detail| detail.selected.clone());
    let virtual_list = library_virtual_list(
        rows.as_ref(),
        scroll,
        "home",
        "8px 8px 8px 8px",
        Callback::new(move |height: f32| measure_library.measure_home_list(height)),
        Callback::new(move |top: f32| scroll_library.scroll_home_list(top)),
        move |row: &HomeListRow| {
            paint_home_row(
                row,
                &paint_library,
                paint_cursor.as_deref(),
                paint_selected.as_ref(),
            )
        },
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
                    on_accept: move |()| accept_library.accept_home_filter(),
                }
            }
            if let Some(frame) = frame {
                if frame.loading {
                    // The Home content has not landed yet (design D12): one
                    // placeholder line holds the list's place, as the TUI's
                    // loading slot does.
                    LibraryListNotice {
                        text: "Loading…".to_owned(),
                        attr: "data-library-home-loading",
                    }
                }
                if !frame.loading && frame.rows.is_empty() {
                    // Home holds nothing to play: one line says so, as the
                    // Music list's empty line does.
                    LibraryListNotice {
                        text: "Nothing to play".to_owned(),
                        attr: "data-library-home-empty",
                    }
                }
            }
            {virtual_list}
        }
    }
}

#[cfg(test)]
mod tests;
