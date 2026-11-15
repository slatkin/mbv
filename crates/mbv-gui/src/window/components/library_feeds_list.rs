//! The Feeds list (`replace-tauri-with-blitz` task 7.7, the port of
//! `LibraryFeedsList`; add-gui-library-panel design D12, row 11.1):
//! the active pill's flat entry rows — the entry's title over its
//! subscription, and the entry date in the gutter — with no letter headings,
//! because the list is sorted by date, and so no letter rail.
//!
//! The rows paint at the fixed heights of [`crate::window::library::feeds_list`]
//! over native scroll, and only the rows in view paint, so a long Latest
//! merge stays fast. The cursor is the window content's own in
//! [`crate::window::library::feeds`] (design D1); the scroll offset and the
//! viewport are the Feeds list's scroll store (design D9). A press selects
//! the entry the Hero shows, and a double-click plays it (design D12): the
//! keys' Enter plays the cursor row, and the press is the selection's half.

use dioxus_native::prelude::*;

use crate::window::components::library_item_row::{
    LibraryItemRow, LibraryTrailPart, name_color, row_background,
};
use crate::window::components::library_list_header::LibraryListHeader;
use crate::window::components::library_list_notice::LibraryListNotice;
use crate::window::components::library_virtual_list::library_virtual_list;
use crate::window::library::feeds_list::{FEEDS_ROW_HEIGHT, FeedsItemRow};
use crate::window::library::parts::{library_feeds_detail, library_feeds_list};
use crate::window::library::shows_workspace::episode_progress;
use crate::window::library::state::LibraryState;

/// One painted row of the flat list: the entry row.
/// A feed entry names no right-click menu — the former row paints none.
fn paint_feeds_row(
    row: &FeedsItemRow,
    library: &LibraryState,
    cursor: Option<&str>,
    selected: Option<&str>,
) -> Element {
    let cursor_row = Some(row.id.as_str()) == cursor;
    let selected_row = Some(row.id.as_str()) == selected;
    let background = row_background(cursor_row, selected_row);
    let press_row = row.clone();
    let press_library = library.clone();
    let play_id = row.row.id.clone();
    let play_library = library.clone();
    rsx! {
        LibraryItemRow {
            attr: "feeds",
            kind: "entry".to_owned(),
            id: row.id.clone(),
            height: FEEDS_ROW_HEIGHT,
            title: row.row.title.clone(),
            subtitle: row.row.subtitle.clone(),
            trail: row.row.date.clone(),
            trail_part: LibraryTrailPart::Date,
            title_color: name_color(&row.row.state),
            background,
            cursor: cursor_row,
            selected: selected_row,
            progress: episode_progress(&row.row.state).map(f32::from),
            on_press: Callback::new(move |()| press_library.press_feeds_row(&press_row)),
            on_play: Callback::new(move |()| play_library.play_feeds_row(&play_id)),
        }
    }
}

/// The Feeds list: the header,
/// the load, failure, and empty lines, and the flat entry rows over native
/// scroll.
#[component]
pub fn LibraryFeedsList() -> Element {
    let library = consume_context::<LibraryState>();

    // The carried Feeds content and the core's selection (design D12, row
    // 11.1): the clones subscribe the list to the frames that carry them, as
    // the Music list's read does; the row model borrows the frame from the
    // section store's own storage through the memo's revision key.
    let carried = library.sections.library_list.cloned();
    let detail_section = library.sections.library_detail.cloned();
    let frame = library_feeds_list(carried.as_deref());
    let detail = library_feeds_detail(detail_section.as_deref());

    // The interaction state and the scroll store this list paints (design
    // D1, D9).
    let feeds_ui = library.feeds_ui.cloned();
    let scroll = library.scrolls.feeds.cloned();

    // The header's selection label (design D15): the active pill's name.
    let selection_label = frame
        .and_then(|frame| {
            frame
                .pills
                .iter()
                .find(|pill| pill.selected)
                .map(|pill| pill.name.clone())
        })
        .unwrap_or_else(|| "Latest".to_owned());

    // The painted rows of the flat list (row 11.1): the same model
    // [`crate::window::library::feeds`] builds from the carried content,
    // narrowed by the filter's text.
    let rows = library.feeds_rows();

    // The list's two DOM facts report to the scroll store (design D9), and
    // the rows paint over the interaction state this render read.
    let measure_library = library.clone();
    let scroll_library = library.clone();
    let paint_library = library.clone();
    let paint_cursor = feeds_ui.cursor.clone();
    let paint_selected = detail.and_then(|detail| detail.selected.clone());
    let virtual_list = library_virtual_list(
        rows.as_ref(),
        scroll,
        "feeds",
        "8px 8px 8px 8px",
        Callback::new(move |height: f32| measure_library.measure_feeds_list(height)),
        Callback::new(move |top: f32| scroll_library.scroll_feeds_list(top)),
        move |row: &FeedsItemRow| {
            paint_feeds_row(
                row,
                &paint_library,
                paint_cursor.as_deref(),
                paint_selected.as_deref(),
            )
        },
    );

    let filter_library = library.clone();
    let accept_library = library.clone();

    rsx! {
        div {
            class: "flex min-h-0 min-w-0 flex-1 flex-col",
            "data-library-feeds-list-column": "",
            // The Music list's header (design D15): the list names itself, the
            // active pill is the carried scope, and the `/` filter field
            // narrows it.
            div {
                class: "flex flex-none flex-col",
                LibraryListHeader {
                    title: "Feeds".to_owned(),
                    label: selection_label.clone(),
                    label_attr: Some("data-library-feeds-selection-label"),
                    filter: feeds_ui.filter.clone(),
                    placeholder: format!("Filter {selection_label}"),
                    aria_label: "Filter entries".to_owned(),
                    on_input: move |value: String| filter_library.set_feeds_filter(value),
                    on_accept: move |()| accept_library.accept_feeds_filter(),
                }
            }
            if let Some(frame) = frame {
                if frame.loading {
                    // The tab's fetch results have not landed yet (row 11.1):
                    // one placeholder line holds the list's place, as every
                    // Library list's does.
                    LibraryListNotice {
                        text: "Loading…".to_owned(),
                        attr: "data-library-feeds-loading",
                    }
                }
                if frame.failed {
                    // A fetch failed (row 11.1): the failure line, and no row
                    // until the destination opens again.
                    LibraryListNotice {
                        text: "Couldn’t load this destination.".to_owned(),
                        attr: "data-library-feeds-failed",
                    }
                }
                if !frame.loading && !frame.failed && frame.rows.is_empty() {
                    // The active view holds no entry (row 11.1): the TUI's
                    // empty slot.
                    LibraryListNotice {
                        text: "No entries".to_owned(),
                        attr: "data-library-feeds-empty",
                    }
                }
            }
            {virtual_list}
        }
    }
}

#[cfg(test)]
mod tests;
