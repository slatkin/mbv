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
//! episode rows paint through the nested row part at the first and second depth
//! (gui-unified-list-rows design D3). The rows the list paints are also the row
//! art set the core fetches (design D7).

use std::collections::HashSet;

use dioxus_native::prelude::*;

use crate::bridge::frame::library::{GuiHomeRowStateFrame, GuiShowsList};
use crate::bridge::library::GuiRowArtScopeFrame;
use crate::window::components::library_letter_heading::paint_letter_heading;
use crate::window::components::library_letter_rail::LibraryLetterRail;
use crate::window::components::library_list_column::{
    ListColumnBody, ListColumnWiring, library_list_column,
};
use crate::window::components::library_list_header::{LibraryListHeaderProps, LibraryListHeading};
use crate::window::components::list_frame_notices::{ListFrameNoticeLine, ListFrameNoticesProps};
use crate::window::components::list_row::{
    FlatRow, LibraryRowThumb, LibraryTrailPart, TreeDepth, TreeLevel, TreeRow, TreeTrail,
};
use crate::window::components::tree_guide::subtree_tails;
use crate::window::library::parts::{library_shows_detail, library_shows_list};
use crate::window::library::shows_list::{ShowsFlatRow, ShowsListRow, shows_letter_rail};
use crate::window::library::state::LibraryState;
use crate::window::pointer_target::PointerList;
use crate::window::service_mark::ServiceMark;

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

/// The carried play state of an episode leaf (`gui-unified-list-rows` design
/// D4): the shared media state of the carried episode row, which picks the
/// title's tone through the row part. `None` while the frame carries no such
/// episode.
fn episode_state(shows: &GuiShowsList, row: &ShowsListRow) -> Option<GuiHomeRowStateFrame> {
    let ShowsListRow::Episode(episode) = row else {
        return None;
    };
    shows
        .series
        .iter()
        .flat_map(|series| &series.seasons)
        .flat_map(|season| season.episodes.iter().flatten())
        .find(|carried| carried.episode_id == episode.source_id)
        .map(|carried| carried.state.clone())
}

/// A top-level series row (gui-delineated-rows design D1): the shared top-level
/// row part over the series' name and its art thumb. An expandable series is a
/// tree parent (gui-tree-double-click design D1, D2, D4), so it paints no caret:
/// its press selects it, its double-click toggles the node, and its hover offers
/// Play, Add, and Shuffle over the series' Service id plus Expand
/// (gui-row-hover-actions design D3). A series that does not expand takes no
/// double-click and paints no Expand control.
fn paint_shows_series(
    row: &ShowsListRow,
    library: &LibraryState,
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
    let hover = library.shows_row_hover_presses(series.source_id.clone());
    rsx! {
        FlatRow {
            attr: "shows",
            kind: "series".to_owned(),
            id: row.id().to_owned(),
            title: series.title.clone(),
            subtitle: None,
            trail: None,
            trail_part: LibraryTrailPart::Date,
            selected: selected_row,
            thumb: Some(LibraryRowThumb {
                art: series.art.clone(),
                media: series.media,
            }),
            tree_open: series.expandable.then_some(series.open),
            tree_guide: series.shows_children,
            on_press: Callback::new(move |()| press_library.press_shows_row(&press_row)),
            on_play: Callback::new(|()| {}),
            on_toggle,
            on_hover_play: hover.play,
            on_hover_queue: hover.queue,
            on_hover_shuffle: hover.shuffle,
            on_context: Some(Callback::new(move |(x, y): (f32, f32)| {
                menu_library.open_row_menu_at(menu_id.clone(), x, y);
            })),
        }
    }
}

/// A nested season node row under an open series (gui-unified-list-rows design
/// D3): the nested row part at the first depth, so it paints the Music album
/// look. An expandable season is a tree parent (gui-tree-double-click design
/// D1, D2), so it paints no caret: its press selects it, its double-click
/// toggles the node, and its hover offers Play, Add, and Shuffle over the
/// season's Service id plus Expand (gui-row-hover-actions design D3). It passes
/// the series' guide through and, while it shows episode rows, starts its own
/// under its label (gui-tree-indent-guides design D2, D3).
fn paint_shows_season(
    row: &ShowsListRow,
    library: &LibraryState,
    selected: Option<&str>,
    tail: bool,
) -> Element {
    let ShowsListRow::Season(season) = row else {
        return rsx! {};
    };
    let selected_row = is_selected_row(row, selected);
    let press_row = row.clone();
    let press_library = library.clone();
    let toggle_node = season.expandable.then(|| row.id().to_owned());
    let toggle_library = library.clone();
    let menu_id = row_source_id(row)
        .map(str::to_owned)
        .map(LibraryState::row_menu_item);
    let menu_library = library.clone();
    let hover = library.shows_row_hover_presses(season.source_id.clone());
    rsx! {
        TreeRow {
            attr: "shows",
            kind: "season".to_owned(),
            id: row.id().to_owned(),
            level: TreeLevel::Parent,
            depth: TreeDepth::One,
            title: season.title.clone(),
            selected: selected_row,
            open: season.expandable.then_some(season.open),
            shows_children: season.shows_children(),
            tail,
            on_press: Callback::new(move |()| press_library.press_shows_row(&press_row)),
            on_play: Callback::new(|()| {}),
            on_toggle: toggle_node.map(|node_id| {
                Callback::new(move |()| toggle_library.toggle_shows_node(&node_id))
            }),
            on_hover_play: hover.play,
            on_hover_queue: hover.queue,
            on_hover_shuffle: hover.shuffle,
            // The part presses the row first, so the menu's View Detail opens
            // the Hero for this row.
            on_context: Some(Callback::new(move |(x, y): (f32, f32)| {
                menu_library.open_row_menu_at(menu_id.clone(), x, y);
            })),
        }
    }
}

/// A flat New or Upcoming row (gui-collection-dropdown design D9;
/// gui-delineated-rows design D1, D2): the shared top-level row part over the
/// two title lines and the date gutter the old Home sections carried, with the
/// row's art thumb. An Upcoming row paints no play affordance and no hover
/// control, takes no select, and opens no row menu, because it is display-only;
/// a New row is a playable episode: its hover offers Play and Add and no
/// Shuffle (gui-row-hover-actions design D3), and it opens the row menu its
/// episode rows do.
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
    // An Upcoming row is display-only, so it paints no hover control (design
    // D3); a New row offers Play and Add over its own id.
    let (hover_play, hover_queue) = if display_only {
        (None, None)
    } else {
        let hover = library.shows_row_hover_presses(id.clone());
        (hover.play, hover.queue)
    };
    rsx! {
        FlatRow {
            attr: "shows",
            kind: "flat".to_owned(),
            id,
            title: row.title.clone(),
            subtitle: row.subtitle.clone(),
            trail: row.date.clone(),
            trail_part: LibraryTrailPart::Date,
            selected: selected_row,
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
            on_hover_play: hover_play,
            on_hover_queue: hover_queue,
            // The shared row part presses on a right-click before it calls
            // `on_context`, so the menu's View Detail shows this row.
            on_context: Some(Callback::new(move |(x, y): (f32, f32)| {
                menu_library.open_row_menu_at(menu_id.clone(), x, y);
            })),
        }
    }
}

/// An episode leaf row (gui-unified-list-rows design D3): the nested row part at
/// the second depth, so it paints the Music track look — the number cell, the
/// muted name, and the runtime — with the double-click's play. It is a leaf of
/// the tree, so it paints both guides that pass through it — the series' and its
/// season's (gui-tree-indent-guides design D3). Its hover offers Play and Add and
/// no Shuffle, because an episode is no collection (gui-row-hover-actions design
/// D3).
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
    // The carried episode names the row's play state, which picks the title's
    // tone (design D4).
    let play = shows.and_then(|shows| episode_state(shows, row));
    let press_row = row.clone();
    let press_library = library.clone();
    let play_id = episode.source_id.clone();
    let play_library = library.clone();
    let menu_id = LibraryState::row_menu_item(episode.source_id.clone());
    let menu_library = library.clone();
    let hover = library.shows_row_hover_presses(episode.source_id.clone());
    rsx! {
        TreeRow {
            attr: "shows",
            kind: "episode".to_owned(),
            id: episode.id.clone(),
            level: TreeLevel::Leaf,
            depth: TreeDepth::Two,
            title: episode.title.clone(),
            number: Some(episode.number.to_string()),
            trail: Some(TreeTrail::Duration(episode.duration.clone())),
            play,
            selected: selected_row,
            tail,
            on_press: Callback::new(move |()| press_library.press_shows_row(&press_row)),
            on_play: Callback::new(move |()| play_library.play_shows_row(&play_id)),
            on_hover_play: hover.play,
            on_hover_queue: hover.queue,
            // The part presses the row first, so the menu's View Detail opens
            // the Hero for this row.
            on_context: Some(Callback::new(move |(x, y): (f32, f32)| {
                menu_library.open_row_menu_at(Some(menu_id.clone()), x, y);
            })),
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
        ShowsListRow::Series(_) => paint_shows_series(row, library, selected),
        ShowsListRow::Season(_) => paint_shows_season(row, library, selected, tail),
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
        Some(crate::gui_core::GuiShowsView::New) => GuiRowArtScopeFrame::ShowsNew,
        Some(crate::gui_core::GuiShowsView::Upcoming) => GuiRowArtScopeFrame::ShowsUpcoming,
        Some(crate::gui_core::GuiShowsView::All) | None => GuiRowArtScopeFrame::ShowsSeries,
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
