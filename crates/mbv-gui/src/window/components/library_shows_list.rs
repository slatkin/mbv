//! The Shows list (`replace-tauri-with-blitz` task 7.7, the port of
//! `LibraryShowsList`; add-gui-library-panel design D14): the Music
//! tree over the TUI's TV node kinds — series nodes, their seasons, and each
//! season's episodes — under the letter headings of the first series of each
//! letter, with the A–Z rail beside them, and the Latest and Upcoming modes'
//! flat rows with their date gutter and no rail. The list header is the
//! Music list's: the `Series` title, the active mode as the selection label,
//! and the `/` filter field.
//!
//! The rows paint at the fixed heights of [`crate::window::library::shows_list`]
//! over native scroll, and only the rows in view paint, so a long series
//! list stays fast. The open and closed nodes and the filter text are the
//! window content's own in [`crate::window::library::shows`] (design D1); the
//! scroll offset and the viewport are the Shows list's scroll store (design
//! D9). A press on a node opens or closes it and selects it for the Hero, a
//! press on a leaf or a flat row selects it, a double-click on an episode plays
//! it, and a right-click opens the item menu, as the Music list's rows do. The
//! list keeps no cursor: the keys act on the row the pointer marks
//! (`gui-pointer-target-interaction` D2).

use std::fmt::Write as _;

use dioxus_native::prelude::*;

use crate::bridge::frame::library::{GuiShowsList, GuiShowsModeFrame};
use crate::window::components::library_item_row::{LibraryItemRow, LibraryTrailPart, mark};
use crate::window::components::library_letter_heading::LibraryLetterHeading;
use crate::window::components::library_letter_rail::LibraryLetterRail;
use crate::window::components::library_list_header::LibraryListHeader;
use crate::window::components::library_virtual_list::library_virtual_list;
use crate::window::components::list_frame_notices::{ListFrameNoticeLine, ListFrameNotices};
use crate::window::input::f32_from_f64;
use crate::window::library::parts::{library_shows_detail, library_shows_list};
use crate::window::library::shows_list::{
    ShowsListRow, active_shows_mode, shows_letter_rail, shows_series_mode,
};
use crate::window::library::shows_workspace::is_played_episode;
use crate::window::library::state::LibraryState;
use crate::window::pointer_target::PointerList;
use crate::window::theme::canvas_var;
use crate::window::truncate::{TextStyle, Truncate};

/// The stable Service id a row names, or `None` on a heading: the item the
/// menu and the double-click act on.
fn row_source_id(row: &ShowsListRow) -> Option<&str> {
    match row {
        ShowsListRow::Heading(_) => None,
        ShowsListRow::Series(series) => Some(&series.source_id),
        ShowsListRow::Season(season) => Some(&season.source_id),
        ShowsListRow::Episode(episode) => Some(&episode.source_id),
        ShowsListRow::Flat(flat) => Some(&flat.row.id),
    }
}

/// Whether the row is the core's selected one: the series the Hero shows, or
/// the flat row it shows.
fn is_selected_row(row: &ShowsListRow, selected: Option<&str>) -> bool {
    row_source_id(row).is_some_and(|source_id| Some(source_id) == selected)
}

/// The row's indentation: the Music tree's depths — the series at the list's
/// own inset, the season under it, the episode under the season.
fn row_inset(row: &ShowsListRow) -> &'static str {
    match row {
        ShowsListRow::Series(_) | ShowsListRow::Flat(_) => "4px",
        ShowsListRow::Season(_) => "22px",
        ShowsListRow::Episode(_) => "40px",
        ShowsListRow::Heading(_) => "8px",
    }
}

/// Whether an episode leaf names a played episode: its carried row carries
/// the shared media state (the Workspace rows' rule).
fn episode_played(shows: &GuiShowsList, row: &ShowsListRow) -> bool {
    let ShowsListRow::Episode(episode) = row else {
        return false;
    };
    for series in &shows.series {
        for season in &series.seasons {
            for carried in season.episodes.iter().flatten() {
                if carried.episode_id == episode.source_id {
                    return is_played_episode(&carried.state);
                }
            }
        }
    }
    false
}

/// The row's name colour: the canvas's text tone, dimmed once an episode is
/// played, as the Workspace's rows are.
fn row_name_color(shows: Option<&GuiShowsList>, row: &ShowsListRow) -> String {
    if shows.is_some_and(|shows| episode_played(shows, row)) {
        canvas_var("muted")
    } else {
        canvas_var("text")
    }
}

/// A row's painted fill (design D14): the core's selected row paints the raised
/// fill — the selection is what the Hero shows, so its row stays marked.
fn row_background(row: &ShowsListRow, selected: Option<&str>) -> Option<String> {
    if matches!(row, ShowsListRow::Heading(_)) {
        return None;
    }
    is_selected_row(row, selected).then(|| canvas_var("library-raise"))
}

/// The open node's caret: the former chevron svg, whose rotation names the
/// open state.
fn paint_chevron(open: bool) -> Element {
    let angle = if open { 90 } else { 0 };
    rsx! {
        span {
            class: "flex flex-none items-center justify-center",
            style: "width: 18px; color: var(--canvas-muted);",
            ..mark("data-library-shows-chevron", true),
            svg {
                width: "10",
                height: "10",
                view_box: "0 0 24 24",
                fill: "none",
                stroke: "currentColor",
                stroke_width: "3",
                stroke_linecap: "round",
                stroke_linejoin: "round",
                "aria-hidden": "true",
                style: "transform: rotate({angle}deg);",
                path { d: "m9 5 7 7-7 7" }
            }
        }
    }
}

/// A series or season node row: the chevron, the
/// name, and the press that opens or closes the node and selects it.
fn paint_shows_node(
    row: &ShowsListRow,
    library: &LibraryState,
    shows: Option<&GuiShowsList>,
    selected: Option<&str>,
) -> Element {
    let (kind, title, open, expandable, height) = match row {
        ShowsListRow::Series(series) => (
            "series",
            &series.title,
            series.open,
            series.expandable,
            series.height,
        ),
        ShowsListRow::Season(season) => (
            "season",
            &season.title,
            season.open,
            season.expandable,
            season.height,
        ),
        _ => return rsx! {},
    };
    let selected_row = is_selected_row(row, selected);
    let mut style = format!(
        "height: {height}px; padding: 0 8px; padding-left: {}; gap: 8px; border: 0; border-radius: 4px; text-align: left; font-size: 14.5px; cursor: pointer;",
        row_inset(row)
    );
    if let Some(background) = row_background(row, selected) {
        let _ = write!(style, " background: {background};");
    }
    let weight = if matches!(row, ShowsListRow::Series(_)) || selected_row {
        600.0
    } else {
        400.0
    };
    let name_color = row_name_color(shows, row);
    let press_row = row.clone();
    let press_library = library.clone();
    let menu_id = row_source_id(row).map(str::to_owned);
    let menu_library = library.clone();
    // The expandable node's `aria-expanded` names the open state (the former client
    // `undefined` painted none); the marks beyond it paint only while they
    // hold.
    let mut row_marks = Vec::new();
    if selected_row {
        row_marks.push(Attribute::new(
            "data-library-shows-selected",
            "",
            None,
            false,
        ));
    }
    if open {
        row_marks.push(Attribute::new("data-library-shows-open", "", None, false));
    }
    if expandable {
        row_marks.push(Attribute::new(
            "aria-expanded",
            if open { "true" } else { "false" },
            None,
            false,
        ));
    }
    rsx! {
        button {
            "type": "button",
            class: "flex w-full items-center overflow-hidden",
            style: "{style}",
            "data-library-shows-row": "{kind}",
            "data-library-shows-row-id": "{row.id()}",
            onclick: move |_| press_library.press_shows_row(&press_row),
            oncontextmenu: move |event: MouseEvent| {
                event.prevent_default();
                let coordinates = event.client_coordinates();
                menu_library.open_row_menu_at(
                    menu_id.clone(),
                    f32_from_f64(coordinates.x),
                    f32_from_f64(coordinates.y),
                );
            },
            ..row_marks,
            if expandable {
                // The chevron's rotation: the open node's caret points down.
                { paint_chevron(open) }
            } else {
                span {
                    class: "flex flex-none items-center justify-center",
                    style: "width: 18px; color: var(--canvas-muted);",
                }
            }
            Truncate {
                text: title.clone(),
                class: "min-w-0 flex-1".to_owned(),
                css: format!("color: {name_color}; font-weight: {weight};"),
                style: TextStyle::sans(14.5, weight),
            }
        }
    }
}

/// An episode leaf row: the number, the name,
/// and the runtime, with the double-click's play.
fn paint_shows_episode(
    row: &ShowsListRow,
    library: &LibraryState,
    shows: Option<&GuiShowsList>,
    selected: Option<&str>,
) -> Element {
    let ShowsListRow::Episode(episode) = row else {
        return rsx! {};
    };
    let selected_row = is_selected_row(row, selected);
    let mut style = format!(
        "height: {}px; padding: 0 8px; padding-left: {}; gap: 8px; border: 0; border-radius: 4px; text-align: left; font-size: 14px; cursor: pointer;",
        episode.height,
        row_inset(row)
    );
    if let Some(background) = row_background(row, selected) {
        let _ = write!(style, " background: {background};");
    }
    let name_color = row_name_color(shows, row);
    let press_row = row.clone();
    let press_library = library.clone();
    let play_id = episode.source_id.clone();
    let play_library = library.clone();
    let menu_id = episode.source_id.clone();
    let menu_library = library.clone();
    rsx! {
        button {
            "type": "button",
            class: "flex w-full items-center overflow-hidden",
            style: "{style}",
            "data-library-shows-row": "episode",
            "data-library-shows-row-id": "{episode.id}",
            onclick: move |_| press_library.press_shows_row(&press_row),
            ondoubleclick: move |_| play_library.play_shows_row(&play_id),
            oncontextmenu: move |event: MouseEvent| {
                event.prevent_default();
                let coordinates = event.client_coordinates();
                menu_library.open_row_menu_at(
                    Some(menu_id.clone()),
                    f32_from_f64(coordinates.x),
                    f32_from_f64(coordinates.y),
                );
            },
            ..mark("data-library-shows-selected", selected_row),
            span {
                class: "flex-none font-mono tabular-nums",
                style: "font-size: 12px; color: var(--canvas-ordinal);",
                "data-library-shows-episode-number": "",
                "{episode.number}"
            }
            Truncate {
                text: episode.title.clone(),
                class: "min-w-0 flex-1".to_owned(),
                css: format!("color: {name_color};"),
                style: TextStyle::sans(14.0, 400.0),
                attrs: vec![Attribute::new("data-library-shows-title", "", None, false)],
            }
            if !episode.duration.is_empty() {
                span {
                    class: "flex-none font-mono tabular-nums",
                    style: "font-size: 12px; color: var(--canvas-muted);",
                    "data-library-shows-episode-duration": "",
                    "{episode.duration}"
                }
            }
        }
    }
}

/// One painted row of the Shows list: the letter
/// heading, the series and season nodes, the episode leaf, or the flat row.
fn paint_shows_row(
    row: &ShowsListRow,
    library: &LibraryState,
    shows: Option<&GuiShowsList>,
    selected: Option<&str>,
) -> Element {
    match row {
        ShowsListRow::Heading(heading) => rsx! {
            LibraryLetterHeading {
                attr: "shows",
                letter: heading.letter.to_string(),
                height: heading.height,
            }
        },
        ShowsListRow::Series(_) | ShowsListRow::Season(_) => {
            paint_shows_node(row, library, shows, selected)
        }
        ShowsListRow::Episode(_) => paint_shows_episode(row, library, shows, selected),
        ShowsListRow::Flat(flat) => {
            let selected_row = is_selected_row(row, selected);
            let press_row = row.clone();
            let press_library = library.clone();
            let play_id = flat.row.id.clone();
            let play_library = library.clone();
            let menu_id = flat.row.id.clone();
            let menu_library = library.clone();
            rsx! {
                LibraryItemRow {
                    attr: "shows",
                    kind: "flat".to_owned(),
                    id: flat.id.clone(),
                    height: flat.height,
                    title: flat.row.title.clone(),
                    subtitle: flat.row.subtitle.clone(),
                    trail: flat.row.date.clone(),
                    trail_part: LibraryTrailPart::Date,
                    title_color: canvas_var("text"),
                    background: row_background(row, selected),
                    selected: selected_row,
                    progress: None,
                    on_press: Callback::new(move |()| press_library.press_shows_row(&press_row)),
                    on_play: Callback::new(move |()| play_library.play_shows_row(&play_id)),
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

/// The Shows list: the header,
/// the load and failure lines, the rows over native scroll, and the All
/// series mode's letter rail.
#[component]
pub fn LibraryShowsList() -> Element {
    let library = consume_context::<LibraryState>();

    // The carried Shows content and the core's selection (design D14): the
    // clones subscribe the list to the frames that carry them, as the Music
    // list's read does; the row model borrows the frame from the section
    // store's own storage through the memo's revision key.
    let carried = library.sections.library_list.cloned();
    let detail_section = library.sections.library_detail.cloned();
    let frame = library_shows_list(carried.as_deref());
    let detail = library_shows_detail(detail_section.as_deref());

    // The interaction state and the scroll store this list paints (design
    // D9).
    let shows_ui = library.shows_ui.cloned();
    let scroll = library.scrolls.shows.cloned();

    // The active view mode: the pill the core marked.
    let mode = frame.map(active_shows_mode);

    // The painted rows (design D14): the tree in the All mode, the flat rows
    // in the Latest and Upcoming modes, narrowed by the filter's text.
    let rows = library.shows_rows();

    // The rail's A–Z letters over the painted rows: the All series mode
    // paints it, and the flat modes paint none.
    let rail = shows_letter_rail(&rows);

    let selected = detail.and_then(|detail| detail.selected.clone());

    // The header's right line, in the Music list's shape: the active mode
    // and its sort rule. The All mode is sorted by name, the flat modes by
    // date.
    let first_collection = frame
        .and_then(|frame| frame.collection_names.first().cloned())
        .unwrap_or_else(|| "All".to_owned());
    let selection_label = if mode == Some(GuiShowsModeFrame::All) {
        format!("{first_collection} · sort name")
    } else {
        first_collection
    };

    // The filter field's placeholder, naming the selection as the Music list
    // does.
    let filter_placeholder = format!(
        "Filter {}",
        frame
            .and_then(|frame| frame.collection_names.first().cloned())
            .unwrap_or_else(|| "shows".to_owned())
    );

    // The list's two DOM facts report to the scroll store (design D9), and
    // the rows paint over the interaction state this render read.
    let measure_library = library.clone();
    let scroll_library = library.clone();
    let paint_library = library.clone();
    let paint_selected = selected.clone();
    let paint_shows = frame;
    let virtual_list = library_virtual_list(
        rows.as_ref(),
        scroll,
        "shows",
        PointerList::Shows,
        "8px 0 8px 8px",
        Callback::new(move |height: f32| measure_library.measure_shows_list(height)),
        Callback::new(move |top: f32| scroll_library.scroll_shows_list(top)),
        move |row: &ShowsListRow| {
            paint_shows_row(row, &paint_library, paint_shows, paint_selected.as_deref())
        },
    );

    // The All series mode paints the rail (design D12); the flat modes paint
    // none.
    let paints_rail = mode.is_some_and(shows_series_mode);

    // The rail's press scrolls to the letter's first row (design D7).
    let jump_library = library.clone();
    let filter_library = library.clone();
    let accept_library = library.clone();

    rsx! {
        div {
            class: "flex min-h-0 min-w-0 flex-1 flex-col",
            "data-library-shows-list-column": "",
            // The Music list's header (design D14): the title, the active mode
            // as the selection label, and the `/` filter field.
            div {
                class: "flex flex-none flex-col",
                LibraryListHeader {
                    title: "Series".to_owned(),
                    title_attr: Some("data-library-shows-list-title"),
                    label: selection_label,
                    label_attr: Some("data-library-shows-selection-label"),
                    filter: shows_ui.filter.clone(),
                    placeholder: filter_placeholder,
                    aria_label: "Filter series".to_owned(),
                    on_input: move |value: String| filter_library.set_shows_filter(value),
                    on_accept: move |()| accept_library.accept_destination_filter(),
                }
            }
            if let Some(frame) = frame {
                // The active mode's list has not landed yet, or the Upcoming
                // mode's eager fetch failed (design D14): one line per state,
                // and the failure line only over a landed list — the pill is
                // the retry.
                ListFrameNotices {
                    loading: frame.loading.then(|| {
                        ListFrameNoticeLine::loading("data-library-shows-loading")
                    }),
                    failed: (!frame.loading && frame.upcoming_failed).then(|| {
                        ListFrameNoticeLine::new(
                            "data-library-shows-failed",
                            "Upcoming fetch failed — select the pill to retry",
                        )
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
                            jump_library.jump_shows_to_letter(letter);
                        })),
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests;
