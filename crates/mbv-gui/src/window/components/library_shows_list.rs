//! The Shows list (`replace-tauri-with-blitz` task 7.7, the port of
//! `LibraryShowsList`; add-gui-library-panel design D14): the Music
//! tree over the TUI's TV node kinds — series nodes, their seasons, and each
//! season's episodes — under the letter headings of the first series of each
//! letter, with the A–Z rail beside them. The GUI Shows destination paints this
//! series list alone (gui-collection-dropdown design D8). The list header is
//! the Music list's: the `Series` title and the `/` filter field, and no
//! selection label — the UI is not verbose.
//!
//! The rows paint at the fixed heights of [`crate::window::library::shows_list`]
//! over native scroll, and only the rows in view paint, so a long series
//! list stays fast. The open and closed nodes and the filter text are the
//! window content's own in [`crate::window::library::shows`] (design D1); the
//! scroll offset and the viewport are the Shows list's scroll store (design
//! D9). A press on a node selects it for the Hero, a double-click on an
//! expandable node opens or closes it, a press on a leaf selects it, a
//! double-click on an episode plays it, and a right-click opens the item menu,
//! as the Music list's rows do (gui-tree-double-click design D1, D2). The
//! list keeps no cursor: the keys act on the row the pointer marks
//! (`gui-pointer-target-interaction` D2). The series rows are top-level rows and
//! paint through the shared row part with their art thumb; their season and
//! episode rows keep their own painters (gui-delineated-rows design D1). The
//! rows the list paints are also the row art set the core fetches (design D7).

use std::collections::HashSet;
use std::fmt::Write as _;

use dioxus_native::prelude::*;

use crate::bridge::frame::library::GuiShowsList;
use crate::bridge::library::GuiRowArtScopeFrame;
use crate::window::components::library_item_row::{
    LibraryItemRow, LibraryRowThumb, LibraryTrailPart, ROW_PADDING_PX, mark,
};
use crate::window::components::library_letter_heading::paint_letter_heading;
use crate::window::components::library_letter_rail::LibraryLetterRail;
use crate::window::components::library_list_column::{
    ListColumnBody, ListColumnWiring, library_list_column,
};
use crate::window::components::library_list_header::{LibraryListHeaderProps, LibraryListHeading};
use crate::window::components::list_frame_notices::{ListFrameNoticeLine, ListFrameNoticesProps};
use crate::window::components::tree_guide::{
    LEAF_INSET_X, NESTED_INSET_X, leaf_guides, nested_parent_guides, subtree_tails,
    tail_divider_style,
};
use crate::window::components::tree_hint::TreeHint;
use crate::window::components::tree_row::{TREE_ROW_FONT_PX, tree_row_text};
use crate::window::input::f32_from_f64;
use crate::window::library::episode::is_played_episode;
use crate::window::library::parts::{library_shows_detail, library_shows_list};
use crate::window::library::shows_list::{ShowsFlatRow, ShowsListRow, shows_letter_rail};
use crate::window::library::state::LibraryState;
use crate::window::pointer_target::PointerList;
use crate::window::service_mark::ServiceMark;
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
        ShowsListRow::Flat(row) => Some(&row.id),
    }
}

/// Whether the row is the core's selected one: the series the Hero shows.
fn is_selected_row(row: &ShowsListRow, selected: Option<&str>) -> bool {
    row_source_id(row).is_some_and(|source_id| Some(source_id) == selected)
}

/// The row's indentation (gui-tree-double-click design D2): the Music tree's
/// depths — the season under its series and the episode under its season, each
/// inset far enough to clear the guides that pass through the row
/// (gui-tree-indent-guides design D2). A series row is a top-level row and
/// paints through the shared row part, so its own inset is never read.
fn row_inset(row: &ShowsListRow) -> f32 {
    match row {
        ShowsListRow::Season(_) => NESTED_INSET_X,
        ShowsListRow::Episode(_) => LEAF_INSET_X,
        // The top-level rows: a series paints through the shared row part, so
        // its own inset is never read, and a heading and a flat row sit at the
        // list's own inset.
        ShowsListRow::Series(_) | ShowsListRow::Heading(_) | ShowsListRow::Flat(_) => 8.0,
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

/// A top-level series row (gui-delineated-rows design D1): the shared row part
/// over the series' name and its art thumb. An expandable series is a tree
/// parent (gui-tree-double-click design D1, D2, D4), so it paints no caret: its
/// press selects it, its double-click toggles the node, and its hover shows the
/// hint. A series that does not expand takes no double-click.
fn paint_shows_series(
    row: &ShowsListRow,
    library: &LibraryState,
    shows: Option<&GuiShowsList>,
    selected: Option<&str>,
) -> Element {
    let ShowsListRow::Series(series) = row else {
        return rsx! {};
    };
    let selected_row = is_selected_row(row, selected);
    let press_row = row.clone();
    let press_library = library.clone();
    let toggle_id = row.id().to_owned();
    let toggle_library = library.clone();
    let on_toggle = series
        .expandable
        .then(|| Callback::new(move |()| toggle_library.toggle_shows_node(&toggle_id)));
    let menu_id = row_source_id(row)
        .map(str::to_owned)
        .map(LibraryState::row_menu_item);
    let menu_library = library.clone();
    rsx! {
        LibraryItemRow {
            attr: "shows",
            kind: "series".to_owned(),
            id: row.id().to_owned(),
            height: series.height,
            title: series.title.clone(),
            subtitle: None,
            trail: None,
            trail_part: LibraryTrailPart::Date,
            title_color: row_name_color(shows, row),
            background: row_background(row, selected),
            selected: selected_row,
            progress: None,
            thumb: Some(LibraryRowThumb {
                art: series.art.clone(),
                media: series.media,
            }),
            tree_open: series.expandable.then_some(series.open),
            tree_guide: series.shows_children,
            on_press: Callback::new(move |()| press_library.press_shows_row(&press_row)),
            on_play: Callback::new(|()| {}),
            on_toggle,
            on_context: Some(Callback::new(move |(x, y): (f32, f32)| {
                menu_library.open_row_menu_at(menu_id.clone(), x, y);
            })),
        }
    }
}

/// A nested season node row under an open series (design D1): the name and the
/// press that selects it. An expandable season is a tree parent
/// (gui-tree-double-click design D1, D2), so it paints no caret: its
/// double-click toggles the node and its hover shows the hint. It keeps its own
/// painter and height. It passes the series' guide through and, while it shows
/// episode rows, starts its own under its label (gui-tree-indent-guides design
/// D2, D3).
fn paint_shows_season(
    row: &ShowsListRow,
    library: &LibraryState,
    shows: Option<&GuiShowsList>,
    selected: Option<&str>,
    tail: bool,
) -> Element {
    let (kind, title, open, expandable, shows_children, height) = match row {
        ShowsListRow::Season(season) => (
            "season",
            &season.title,
            season.open,
            season.expandable,
            season.shows_children(),
            season.height,
        ),
        _ => return rsx! {},
    };
    let selected_row = is_selected_row(row, selected);
    let mut style = format!(
        "height: {height}px; padding: 0 {ROW_PADDING_PX}px; padding-left: {}px; gap: 8px; border: 0; border-radius: 4px; text-align: left; font-size: {TREE_ROW_FONT_PX}px; cursor: pointer;",
        row_inset(row)
    );
    // The expanded area's fill under an open series, unless the row is the
    // selected one.
    let fill = row_background(row, selected).unwrap_or_else(|| canvas_var("tree-subtree"));
    let _ = write!(style, " background: {fill};");
    style.push_str(&tail_divider_style(tail));
    let weight = if selected_row { 600.0 } else { 400.0 };
    let name_color = row_name_color(shows, row);
    let press_row = row.clone();
    let press_library = library.clone();
    let toggle_node = expandable.then(|| row.id().to_owned());
    let toggle_library = library.clone();
    let row_class = if expandable {
        "tree-parent relative flex w-full items-center"
    } else {
        // A season that does not expand still paints the series' guide
        // through, so it carries the span's containing block
        // (gui-tree-indent-guides design D1).
        "relative flex w-full items-center overflow-hidden"
    };
    let menu_id = row_source_id(row)
        .map(str::to_owned)
        .map(LibraryState::row_menu_item);
    let menu_row = row.clone();
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
            class: "{row_class}",
            style: "{style}",
            "data-library-shows-row": "{kind}",
            "data-library-shows-row-id": "{row.id()}",
            onclick: move |_| press_library.press_shows_row(&press_row),
            ondoubleclick: move |_| {
                if let Some(node_id) = toggle_node.as_deref() {
                    toggle_library.toggle_shows_node(node_id);
                }
            },
            oncontextmenu: move |event: MouseEvent| {
                event.prevent_default();
                // A right-click selects the row first, as a press does, so the
                // menu's View Detail opens the Hero for this row.
                menu_library.press_shows_row(&menu_row);
                let coordinates = event.client_coordinates();
                menu_library.open_row_menu_at(
                    menu_id.clone(),
                    f32_from_f64(coordinates.x),
                    f32_from_f64(coordinates.y),
                );
            },
            ..row_marks,
            Truncate {
                text: title.clone(),
                class: "min-w-0 flex-1".to_owned(),
                css: format!("color: {name_color}; font-weight: {weight};"),
                style: TextStyle::sans(TREE_ROW_FONT_PX, weight),
            }
            {nested_parent_guides(height, shows_children)}
            if expandable {
                TreeHint { open }
            }
        }
    }
}

/// A flat New or Upcoming row (gui-collection-dropdown design D9;
/// gui-delineated-rows design D1, D2): the shared top-level row part over the
/// two title lines and the date gutter the old Home sections carried, with the
/// row's art thumb. An Upcoming row paints no play affordance, takes no
/// select, and opens no row menu, because it is display-only; a New row is a
/// playable episode and opens the row menu its episode rows do.
fn paint_shows_flat(row: &ShowsFlatRow, library: &LibraryState, selected: Option<&str>) -> Element {
    let id = row.id.clone();
    let selected_row = Some(id.as_str()) == selected;
    let display_only = row.display_only;
    let press_row = row.clone();
    let press_library = library.clone();
    let play_id = id.clone();
    let play_library = library.clone();
    let menu_id = (!display_only).then(|| LibraryState::row_menu_item(id.clone()));
    let menu_library = library.clone();
    rsx! {
        LibraryItemRow {
            attr: "shows",
            kind: "flat".to_owned(),
            id,
            height: row.height,
            title: row.title.clone(),
            subtitle: row.subtitle.clone(),
            trail: row.date.clone(),
            trail_part: LibraryTrailPart::Date,
            title_color: canvas_var("text"),
            background: selected_row.then(|| canvas_var("library-raise")),
            selected: selected_row,
            progress: None,
            thumb: Some(LibraryRowThumb {
                art: row.art.clone(),
                media: row.media,
            }),
            on_press: Callback::new(move |()| {
                press_library.select_shows_row(&ShowsListRow::Flat(press_row.clone()));
            }),
            on_play: Callback::new(move |()| {
                if !display_only {
                    play_library.play_shows_row(&play_id);
                }
            }),
            // The shared row part presses on a right-click before it calls
            // `on_context`, so the menu's View Detail shows this row.
            on_context: Some(Callback::new(move |(x, y): (f32, f32)| {
                menu_library.open_row_menu_at(menu_id.clone(), x, y);
            })),
        }
    }
}

/// An episode leaf row: the number, the name,
/// and the runtime, with the double-click's play. It is a leaf of the tree, so
/// it paints both guides that pass through it — the series' and its season's
/// (gui-tree-indent-guides design D3).
fn paint_shows_episode(
    row: &ShowsListRow,
    library: &LibraryState,
    shows: Option<&GuiShowsList>,
    selected: Option<&str>,
    tail: bool,
) -> Element {
    let ShowsListRow::Episode(episode) = row else {
        return rsx! {};
    };
    let selected_row = is_selected_row(row, selected);
    let mut style = format!(
        "height: {}px; padding: 0 {ROW_PADDING_PX}px; padding-left: {}px; gap: 8px; border: 0; border-radius: 4px; text-align: left; font-size: {TREE_ROW_FONT_PX}px; cursor: pointer;",
        episode.height,
        row_inset(row)
    );
    // The expanded area's fill under an open series, unless the row is the
    // selected one.
    let fill = row_background(row, selected).unwrap_or_else(|| canvas_var("tree-subtree"));
    let _ = write!(style, " background: {fill};");
    style.push_str(&tail_divider_style(tail));
    let name_color = row_name_color(shows, row);
    let press_row = row.clone();
    let press_library = library.clone();
    let play_id = episode.source_id.clone();
    let play_library = library.clone();
    let menu_id = episode.source_id.clone();
    let menu_row = row.clone();
    let menu_library = library.clone();
    rsx! {
        button {
            "type": "button",
            class: "relative flex w-full items-center overflow-hidden",
            style: "{style}",
            "data-library-shows-row": "episode",
            "data-library-shows-row-id": "{episode.id}",
            onclick: move |_| press_library.press_shows_row(&press_row),
            ondoubleclick: move |_| play_library.play_shows_row(&play_id),
            oncontextmenu: move |event: MouseEvent| {
                event.prevent_default();
                // A right-click selects the row first, as a press does, so the
                // menu's View Detail opens the Hero for this row.
                menu_library.press_shows_row(&menu_row);
                let coordinates = event.client_coordinates();
                menu_library.open_row_menu_at(
                    Some(LibraryState::row_menu_item(menu_id.clone())),
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
                style: tree_row_text(),
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
            {leaf_guides()}
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
    tails: &HashSet<String>,
) -> Element {
    let tail = tails.contains(row.id());
    match row {
        ShowsListRow::Heading(heading) => paint_letter_heading("shows", heading),
        ShowsListRow::Series(_) => paint_shows_series(row, library, shows, selected),
        ShowsListRow::Season(_) => paint_shows_season(row, library, shows, selected, tail),
        ShowsListRow::Episode(_) => paint_shows_episode(row, library, shows, selected, tail),
        ShowsListRow::Flat(flat) => paint_shows_flat(flat, library, selected),
    }
}

/// The Shows list: the header, the load and failure lines, the rows over
/// native scroll, and the series list's letter rail.
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

    // The painted rows (design D14): the series tree, narrowed by the
    // filter's text. The GUI Shows destination paints this list alone (design
    // D8).
    let rows = library.shows_rows();

    // The rail's A–Z letters over the painted rows.
    let rail = shows_letter_rail(&rows);

    // The sort control the header paints last on the heading's line
    // (gui-flat-list-sort design D7): the New view's list is the one it orders,
    // so the frame carries it only while New paints.
    let sort = frame.and_then(|frame| frame.sort.clone());

    let selected = detail.and_then(|detail| detail.selected.clone());

    // The filter field's placeholder, naming the Music selection as the Music
    // list does: the Shows list paints no collection selector (design D8), so
    // the placeholder names the whole library.
    let filter_placeholder = "Filter shows".to_owned();

    // The list's two DOM facts report to the scroll store (design D9), and
    // the rows paint over the interaction state this render read.
    let measure_library = library.clone();
    let scroll_library = library.clone();
    let art_library = library.clone();
    // The destination list the painted rows' art belongs to (gui-delineated-rows
    // design D7): the visible view names its own scope.
    let art_scope = match frame.map(|frame| frame.view) {
        Some(mbv::GuiShowsView::New) => GuiRowArtScopeFrame::ShowsNew,
        Some(mbv::GuiShowsView::Upcoming) => GuiRowArtScopeFrame::ShowsUpcoming,
        Some(mbv::GuiShowsView::All) | None => GuiRowArtScopeFrame::ShowsSeries,
    };
    let paint_library = library.clone();
    // The rows that close an open series' subtree paint its divider.
    let tails = subtree_tails(&rows, |row| {
        matches!(row, ShowsListRow::Season(_) | ShowsListRow::Episode(_))
    });
    let paint_selected = selected.clone();
    let paint_shows = frame;
    // The column's wiring is the one scaffold every list paints through
    // (design D14): its two DOM facts report to the scroll store (design D9),
    // the rows in view report the scope they paint under to the core for their
    // row art fetches (gui-delineated-rows design D7).
    let wiring = ListColumnWiring {
        rows: rows.as_ref(),
        view: scroll,
        attr: "shows",
        list: PointerList::Shows,
        padding: "8px 0 8px 8px",
        on_measure: Callback::new(move |height: f32| measure_library.measure_shows_list(height)),
        on_scroll: Callback::new(move |top: f32| scroll_library.scroll_shows_list(top)),
        rows_in_view: Some(Callback::new(move |ids: Vec<String>| {
            art_library.report_rows_in_view(art_scope, ids);
        })),
    };

    // The rail paints exactly while the rows carry letters (gui-flat-list-sort
    // design D8): the All view's series tree, and the New view while its sort
    // compares a name. The date-sorted views show no heading and no rail (spec
    // "The letter rail jumps a name-sorted list").
    let paints_rail = rows.iter().any(|row| row.letter().is_some());

    // The rail's press scrolls to the letter's first row (design D7).
    let jump_library = library.clone();
    let rail = paints_rail.then(|| {
        rsx! {
            LibraryLetterRail {
                letters: rail,
                jump: Some(Callback::new(move |letter: char| {
                    jump_library.jump_shows_to_letter(letter);
                })),
            }
        }
    });

    let filter_library = library.clone();
    let accept_library = library.clone();
    // The header's facts (design D14, user override): the Shows destination is
    // an Emby destination, so its Emby mark names it under every view
    // (gui-library-header-service-marks design D5), and the view dropdown names
    // the view; no title repeats it.
    let header = LibraryListHeaderProps {
        heading: LibraryListHeading::Mark(ServiceMark::Emby),
        label: String::new(),
        label_attr: None,
        collections: None,
        view: frame.map(|frame| frame.view),
        sort,
        filter: shows_ui.filter.clone(),
        placeholder: filter_placeholder,
        aria_label: "Filter series".to_owned(),
        on_input: Callback::new(move |value: String| filter_library.set_shows_filter(value)),
        on_accept: Callback::new(move |()| accept_library.accept_destination_filter()),
    };

    // The series list has not landed yet, or the Upcoming mode's eager fetch
    // failed (design D14): one line per state, and the failure line only over a
    // landed list.
    let notices = frame.map(|frame| ListFrameNoticesProps {
        loading: frame
            .loading
            .then(|| ListFrameNoticeLine::loading("data-library-shows-loading")),
        failed: (!frame.loading && frame.upcoming_failed).then(|| {
            ListFrameNoticeLine::new("data-library-shows-failed", "Upcoming fetch failed")
        }),
        empty: None,
    });

    library_list_column(
        &wiring,
        move |row: &ShowsListRow| {
            paint_shows_row(
                row,
                &paint_library,
                paint_shows,
                paint_selected.as_deref(),
                &tails,
            )
        },
        header,
        notices,
        ListColumnBody::Rail(rail),
    )
}

#[cfg(test)]
mod tests;
