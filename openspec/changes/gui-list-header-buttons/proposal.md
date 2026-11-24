# Proposal: GUI list header buttons — expand all, collapse all, refresh

## Why

The GUI Library tree lists (Music, and Shows in its All view) open and close one parent row at a time by double-click, so a user who wants to see a whole tree, or close it again, must act on every row. The GUI Library lists also have no way to reload their content: the TUI has F5, but the GUI Client has no refresh for any Library destination. The Playlists screen has a refresh, but it is a text button that does not match the other header controls.

## What Changes

- The list header of each GUI tree list (Music; Shows in the All view) shows two icon buttons, Expand all and Collapse all, directly to the left of the header's dropdown (the collection dropdown in Music, the view dropdown in Shows).
- Expand all opens every top-level parent row that the list shows: every artist in Music, every series in Shows. Albums and seasons stay as they were. Collapse all closes every open parent row at every level.
- A Shows series that is open and in view loads its seasons when they are not loaded, and an open season in view loads its episodes. The loads run a few at a time. This makes Expand all show seasons as the user scrolls, and it keeps the 128-entry series detail memory limit (issue #917) in force.
- The list header of every GUI Library destination (Home, Music, Shows in all three views, each plain Emby library, Audiobooks, Podcasts, Feeds) shows a refresh icon button. It reloads that destination's list. In a tree list, the children of the open parent rows in view also reload, and no load starts for an open row out of view. The rows stay on screen until the new rows arrive. A failed refresh keeps the rows.
- The refresh button sits to the left of the Expand all button on a tree list, and to the left of the dropdown or the sort control on other lists. Where the line has neither, it is the line's rightmost item.
- The Playlists screen's text Refresh button becomes the same refresh icon button. Its command does not change.
- No keybind changes. The TUI does not change.

## Decisions made with the user

- Expand all opens the top level only (artists, series). It does not open albums or seasons.
- In Shows, Expand all loads seasons for the open series in view, as the user scrolls, not for every series at once. The series detail memory limit stays.
- "Every list" for refresh means the list header of every Library destination. The Queue and the Search results get no refresh button. The Playlists screen keeps its own refresh, now with the same icon.
- The refresh icon is Lucide `refresh-ccw`.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `gui-client`: adds the tree header Expand all and Collapse all buttons, the in-view season and episode loads for open Shows nodes, and the Library list header refresh. Changes the Podcasts header scenario that says the title line holds nothing but the mark, the dropdown, and the sort control, and the two title-line scenarios of the sort-control requirement, so that each names the refresh button.

## Impact

The GUI window (package `mbv-gui`):

- `crates/mbv-gui/src/window/components/library_list_header.rs`: new tree-action and refresh props and the trailing control group; `header_icon_button.rs` is the new icon button part.
- `crates/mbv-gui/src/window/icons.rs`: three Lucide icons (`refresh-ccw`, `chevrons-up-down`, `chevrons-down-up`).
- `crates/mbv-gui/src/window/library/state/rows.rs`, `crates/mbv-gui/src/window/library/shows.rs`: expand-all and collapse-all over the window's open-node state, the Shows in-view report, and the refresh sends.
- `crates/mbv-gui/src/window/components/library_list.rs`, `library_shows_list.rs`, `library_home_list.rs`, `library_lettered_list.rs`: header wiring; the Shows list reports its open nodes in view.
- `crates/mbv-gui/src/window/components/playlists_panel.rs`: the refresh control. The open change `gui-playlists-in-queue-column` also edits this file; the edit here touches only the header button.

The GUI core (package `mbv-gui`):

- `crates/mbv-gui/src/gui_core/gui_library.rs` and `crates/mbv-gui/src/bridge/library.rs`: two new Library commands (`Refresh`, `ShowsOpenInView`).
- `crates/mbv-gui/src/gui_core/gui_library/`: a new `refresh.rs` (per-destination refresh) and `shows_open_loads.rs` (the bounded in-view Shows loader); replacement-fetch paths in `store.rs`, `emby.rs`, `shows.rs`; the feed-group refresh queue that the ensure step takes first; the refresh landing rule in `client_events.rs`.
- `crates/mbv-gui/src/gui_core/gui_client_state/state.rs`: the loader's report and started-load sets, the refresh in-flight sets, and the store-effect arm that runs the loader.

The Client core (package `mbv-client-core`):

- `crates/mbv-client-core/src/app/infra/image_fetch/fetch.rs`, `dispatch/library/cursor.rs`, `dispatch/library/event.rs`: the quiet series-detail fetch, the series-detail, season, and album-track refetches, and their failure events.
- `crates/mbv-client-core/src/app/dispatch/library/stores.rs`: a new `ClientLibraryEffect` variant that tells a Client that series content landed.
- `crates/mbv-client-core/src/app/dispatch/library/browse/tv.rs`, `dispatch/library/load.rs`: the Upcoming refetch and the Home refresh as its own method.

No new dependency. No `App` field carries GUI state, and no core function names a GUI type.
