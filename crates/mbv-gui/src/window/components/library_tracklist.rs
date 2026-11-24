//! The Hero's tracklist (`replace-tauri-with-blitz` task 7.3, the port of
//! `LibraryTracklist`; add-gui-library-panel design D5, D8, D15): the
//! picked album's cached tracks, painted by the one Workspace table every
//! destination shares. The tracklist keeps no cursor: Enter and `a` act
//! on the row the pointer marks (`gui-pointer-target-interaction` D2), and a
//! press on a row changes nothing.
//!
//! The hover control runs add to queue on that track
//! (design D8); a press on it sends the track's own id. A track plays
//! from its row's play button, which replaces the track number on hover, and
//! from a double-click on the row: the queue is replaced, and under autoload
//! the album plays from that track onward. The row of the track playing at
//! the playhead shows a play glyph beside its title (the `GuiQueue` frame
//! names `active`); a highlight marks focus only, so it keeps its own fill.

use dioxus_native::prelude::*;

use crate::bridge::frame::library::GuiMusicTrackFrame;
use crate::bridge::library::GuiLibraryCommand;
use crate::window::components::library_workspace_table::{
    LibraryWorkspaceTable, WorkspaceAction, WorkspaceActionIcon, WorkspaceRow,
};
use crate::window::library::state::LibraryState;
use crate::window::pointer_target::PointerList;

/// The props of the tracklist (the `LibraryTracklist` props, narrowed
/// to what the tracklist reads): the album's cached tracks — the rows' own
/// data, moved from the carried Hero, so a Hero repaint never clones the
/// album's tracklist for the rows (the 7.3-7.4 review's whole-list finding)
/// — and the id of the track playing at the playhead (design D8).
#[derive(Props, Debug, Clone, PartialEq)]
pub struct LibraryTracklistProps {
    /// The picked album's cached tracks (design D5): the rows the table
    /// paints.
    pub tracks: Vec<GuiMusicTrackFrame>,
    /// The id of the track playing at the playhead, or `None` while idle:
    /// the frame's playing fact (design D8).
    pub playing_track_id: Option<String>,
}

/// The Hero's tracklist: the album's rows over the shared Workspace table,
/// with the tracklist's own actions and row menu.
#[component]
pub fn LibraryTracklist(props: LibraryTracklistProps) -> Element {
    let library = consume_context::<LibraryState>();

    let rows: Vec<WorkspaceRow> = props
        .tracks
        .iter()
        .map(|track| WorkspaceRow {
            id: track.track_id.clone(),
            number: track.number.to_string(),
            name: track.name.clone(),
            duration: track.duration.clone(),
            playing: Some(track.track_id.as_str()) == props.playing_track_id.as_deref(),
            state: None,
            progress: None,
        })
        .collect();

    // The hover actions (design D8): add to queue appends to the tail.
    let queue_library = library.clone();
    let actions = vec![WorkspaceAction {
        key: "addToQueue".to_owned(),
        label: "Add to queue".to_owned(),
        icon: WorkspaceActionIcon::Plus,
        run: Callback::new(move |track_id: String| {
            queue_library
                .link
                .library(GuiLibraryCommand::AddToQueue { item_id: track_id });
        }),
    }];

    // A press on a row changes nothing (design D2): the keys act on the row
    // under the pointer, and the row's own controls run its actions. A play —
    // the play control or the double-click — plays the track; a right-press
    // opens the row menu at the pointer.
    let on_press = Callback::new(|_track_id: String| {});
    let play_library = library.clone();
    let on_play = Callback::new(move |track_id: String| {
        play_library.play_library_track(track_id);
    });
    let menu_library = library.clone();
    let on_context = Callback::new(move |(x, y, track_id): (f32, f32, String)| {
        menu_library.open_library_row_menu(LibraryState::row_menu_item(track_id), x, y);
    });

    rsx! {
        LibraryWorkspaceTable {
            root: "data-library-tracklist",
            label: "Tracklist",
            hook: "track",
            noun: "Track",
            pointer_list: PointerList::Tracklist,
            rows,
            actions,
            on_press,
            on_play,
            on_context: Some(on_context),
        }
    }
}
