# Design

## Context

See `proposal.md` for the motivation and the user decisions. The requirements are in `specs/gui-client/spec.md`.

- **The header.** Every Library list paints its header through one component, `LibraryListHeader` (`crates/mbv-gui/src/window/components/library_list_header.rs`), from `library_list_column`. Its title line is a `justify-between` flex row: the heading (Service mark or Home title), then the collection dropdown, the Shows view dropdown, or the selection label, then the sort control. Four destinations build its props: `library_list.rs` (Music), `library_shows_list.rs` (Shows), `library_home_list.rs` (Home), `library_lettered_list.rs` (plain Emby, feed-group Emby, Audiobooks, Podcasts, Feeds).
- **Tree open state is window state.** Music keeps `LibraryUiState.open_nodes` and `filter_closed` (`window/library/state.rs`), changed through `set_node_open` (`state/rows.rs`). Shows keeps `shows_ui.open` and `closed` (`window/library/shows.rs`). Outside a filter, a node is open when `open` holds it. Under a filter, a node is open when it has a kept child and `closed` does not hold it (`build_library_rows`, `build_shows_rows`).
- **Children load differently.** A Music artist's albums come with the group album lists (`gui_music_albums`), so an open artist needs no load. An album's tracks load through `OpenAlbum` → `fetch_album_tracks`. A Shows series' seasons come from `series_detail_cache`, which loads only through `fetch_series_detail`, today reached by a pick (`ShowsSelect`) or the ensure pass. The cache is capped at 128 entries (`MAX_SERIES_DETAIL_CACHE`, issue #917), and `cache_series_detail` keeps the first entry for an id. Each detail landing also loads the first season's episodes (`handle_series_detail_fetched`).
- **GUI loads are idempotent ensure steps.** `ensure_gui_destination` (`src/app/gui_library/ensure.rs`) runs root, selection, and dependents steps whose guards skip a store that is `Loading` or `Loaded`. Nothing in the GUI reloads a loaded store. The TUI's F5 (`refresh_current_view`, `src/app/dispatch/library/load.rs`) reads `self.tab` and is `ActionScope::TuiOnly`.
- **Several fetches turn an error into an empty result** (`fetch_album_tracks`, `fetch_series_detail`, `fetch_series_season_episodes` use `unwrap_or_default`). A refetch through them would replace good content with nothing when the Service fails.
- **The Playlists screen** paints a text `SettingsButton` ("Refresh"/"Refreshing…") in `tree_column` (`playlists_panel.rs`), which sends `GuiPlaylistsCommand::Refresh { id: None }`.

## Goals / Non-Goals

**Goals:**

- One header layout owner: every destination gets the refresh button, and the tree lists get the tree buttons, through `LibraryListHeader` props.
- Expand all and Collapse all are window-only. They change open state and send no command, except the Shows in-view report that every open Shows row now drives.
- A refresh never blanks a loaded list and never replaces good content with the result of a failed fetch, on every path this change adds.
- One bounded loader for Shows children, driven by what is open and in view.

**Non-Goals:**

- No keybind, no status bar hint, no TUI change.
- The Music tree loads no tracks for albums in view. Expand all does not open albums, so no Music in-view loader is needed.
- The existing refresh paths that this change reuses (Home, Audiobooks, Podcasts, Feeds, feed-group Emby, the Shows root level) keep their own loading and failure behavior.
- No busy state on any refresh button, the Playlists one included.

## Decisions

### D1. The title line gets one trailing control group

`LibraryListHeader`'s title line keeps `justify-between` with two children: the heading, and a new trailing `div` (`flex flex-none items-center`, the existing 12 px gap shrunk to 6 px inside the group) that holds, in order: the refresh button, the Expand all and Collapse all buttons when the destination passes tree actions, the dropdown or view dropdown or selection label (unchanged rules), and the sort control. This keeps the dropdown at the line's right end where there is no sort control, and puts the tree buttons directly left of the dropdown, as the user asked.

New props on `LibraryListHeaderProps`:

```rust
/// The list's refresh press (every destination paints one).
pub on_refresh: Callback<()>,
/// The tree list's Expand all and Collapse all presses; `None` on a flat list.
#[props(default)]
pub tree: Option<LibraryTreeActions>,

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LibraryTreeActions {
    pub on_expand_all: Callback<()>,
    pub on_collapse_all: Callback<()>,
}
```

*Alternative:* `margin-left: auto` on the first trailing item with no wrapper. Rejected: the selection-label and no-dropdown cases each need their own first item, so the rule would have three owners.

### D2. One header icon button part

A new component `crates/mbv-gui/src/window/components/header_icon_button.rs` paints a 28 × 28 px `button` (the title line's height) with the sort control's border (`1px solid var(--canvas-border)`, 6 px radius), no fill at rest, `var(--canvas-muted)` icon colour, and a 14 px icon at stroke width 1.6. Props: `label: &'static str` (the `aria-label` and the `title` tooltip), `hook: &'static str` (a `data-*` attribute name), `onpress: Callback<()>`, and `children: Element` (the icon). The header uses it for all three buttons, with labels "Refresh", "Expand all", "Collapse all" and hooks `data-library-refresh`, `data-library-expand-all`, `data-library-collapse-all`.

The Playlists screen's `tree_column` replaces its `SettingsButton` with this part (label "Refresh", hook `data-playlists-refresh`, icon `RefreshCcw`). The `refresh_label` text goes. The command stays `GuiPlaylistsCommand::Refresh { id: None }`. The open change `gui-playlists-in-queue-column` rewrites other parts of `playlists_panel.rs`. This edit touches only the header button, so the two changes merge cleanly in either order.

### D3. Three Lucide icons join `window/icons.rs`

The `lucide_icons!` table gets three entries, with the child elements copied from Lucide (the same paths in `@lucide/svelte` 1.70.0, which the module vendors; ISC notice already present):

- `RefreshCcw("refresh-ccw")`: `path M21 12a9 9 0 0 0-9-9 9.75 9.75 0 0 0-6.74 2.74L3 8`, `path M3 3v5h5`, `path M3 12a9 9 0 0 0 9 9 9.75 9.75 0 0 0 6.74-2.74L21 16`, `path M16 16h5v5`. The user chose this icon.
- `ChevronsUpDown("chevrons-up-down")` for Expand all: `path m7 15 5 5 5-5`, `path m7 9 5-5 5 5`.
- `ChevronsDownUp("chevrons-down-up")` for Collapse all: `path m7 20 5-5 5 5`, `path m7 4 5 5 5-5`.

The module doc's icon count goes up by three. No dependency changes.

### D4. Expand all and Collapse all change window open state only

Music, in `window/library/state/rows.rs`:

- `expand_all_music(&self)`: read `self.rows_of(&ui)`. For each `LibraryRow::Artist` row, add its id to `open_nodes` and remove it from `filter_closed`.
- `collapse_all_music(&self)`: clear `open_nodes`. While the filter text is not empty, add the id of every painted `Artist` and `Album` row to `filter_closed`.

Shows, in `window/library/shows.rs`:

- `expand_all_shows(&self)`: read `self.shows_rows()`. For each `ShowsListRow::Series` row, add its id to `shows_ui.open` and remove it from `shows_ui.closed`.
- `collapse_all_shows(&self)`: clear `shows_ui.open`. While the filter text is not empty, add the id of every painted `Series` and `Season` row to `shows_ui.closed`.

Neither sends a command, picks a row, or touches the Hero. "Painted rows" are the rows the current filter keeps, so a filter limits both actions to its matches (spec). Adding the artist ids to `open_nodes` under a filter is what keeps them open after the filter clears. Music passes `tree: Some(..)` always. Shows passes it only while the view is `GuiShowsView::All`.

*Alternative:* open every node in the carried content, not only the painted rows. Rejected: under a filter it would open hidden series, and in Shows it would widen the set the in-view loader reads for no visible gain.

### D5. Open Shows rows in view drive one bounded loader

**Window.** `library_shows_list.rs`, in the All view, computes the open nodes among the painted window (`list_window(rows, scroll_top, viewport_height)`): the `source_id` of each open `Series` row, and the `(series_id, source_id)` of each open `Season` row, in paint order. A `use_effect(use_reactive(..))` keyed on that value sends it through a new `LibraryState::report_shows_open_in_view`, as the row-art report does. The report fires when a node opens or closes, when the view scrolls, and when rows land.

**Command.** `GuiLibraryCommand::ShowsOpenInView { lib_idx: usize, series: Vec<String>, seasons: Vec<(String, String)> }` in both enums (`crates/mbv-gui/src/bridge/library.rs`, `src/app/gui_library.rs`), with an exhaustive dispatch arm.

**Core.** A new module `src/app/gui_library/shows_open_loads.rs` owns:

- `gui_shows_open_in_view: Option<GuiShowsOpenInView>` on `App` (one Shows list paints at a time), replaced by each report. A report for a library that is not a Shows library is an ignored command.
- `gui_shows_open_series_loads: HashSet<String>` and `gui_shows_open_season_loads: HashSet<(String, String)>`: the loads this loader started.
- `const SHOWS_OPEN_LOAD_LIMIT: usize = 4`.
- `pump_gui_shows_open_loads(&mut self)`: drop the loader's entries that are no longer in `series_detail_loading` / `series_season_loading`. While the loader's in-flight count is under the limit, start the next missing load in report order: first a series whose detail is not cached (or is stale, D7) and not loading, through `fetch_series_detail` (or `refetch_series_detail`); then a season whose series detail is cached, whose `episodes` map lacks it (or it is stale), and that is not loading, through `fetch_series_season_episodes` (or `refetch_series_season_episodes`).

The pump runs on each `ShowsOpenInView` command and after each `SeriesEvent` landing, from `handle_series_event` (`src/app/dispatch/library/event.rs`). In the TUI the report is always `None`, so the pump is a no-op there. A row that leaves the view drops out of the next report, so its load never starts (spec).

**First-season auto-load.** `handle_series_detail_fetched` skips its first-season episodes fetch when the series id is in `gui_shows_open_series_loads` and no `gui_tv_shows` entry selects that series. Expand all keeps seasons closed, so those episodes would be wasted requests. Pending season expansions still run.

The cache cap stays at 128. The view never shows more than 128 open series, so an in-view series is never evicted by its own neighbours. A series evicted after it scrolled out reloads when it comes back.

*Alternatives:* loading every series at once (the user rejected it, cap); a new select-free command per expand press (would still need the cap fix and a request queue).

### D6. One `Refresh` command, one per-destination dispatcher

**Window.** Each destination passes `on_refresh` to the header, calling `LibraryState::refresh_library_list(album_ids)`. Music passes the album ids of the painted `Album` rows that are open. Every other destination passes an empty list. The window keeps its pick, filter, scroll, and open state.

**Command.** `GuiLibraryCommand::Refresh { album_ids: Vec<String> }` in both enums, with an exhaustive arm calling `App::refresh_gui_destination(album_ids)`.

**Core.** A new module `src/app/gui_library/refresh.rs` resolves `self.gui_active_destination()` (`None` → ignored command log) and matches `GuiDestinationKind` exhaustively:

| Destination | Refresh |
| --- | --- |
| `Home` | `refresh_home_content()`, the Home arm of `refresh_current_view` moved into its own method that both call |
| `Music { lib_idx }` | `refetch_gui_group_albums(lib_idx, group)` for each selected group (`gui_selected_groups` over `gui_effective_music_set`), then `refetch_album_tracks(id)` for each named album |
| `Shows` in All | `refresh_lib(lib_idx)` (the root level's in-place refresh), then mark every cached series detail stale (D7), `refetch_series_detail` for the selected series, and pump (D5) |
| `Shows` in New | `refetch_gui_shows_new(library_id)` |
| `Shows` in Upcoming | `refetch_tv_upcoming_snapshot(lib_idx)` |
| `Library` in feed-group mode | `refresh_lib(lib_idx)`, the TUI F5 path, which re-reads the shared `feed_home_video` state the GUI paints |
| `Library` plain | `refetch_gui_emby_root_items(lib_idx)` |
| `Audiobooks { lib_idx }` | `audiobookshelf_book_refresh()`, under the same active-tab guard `ensure_gui_destination` uses |
| `Podcasts { lib_idx }` | `audiobookshelf_refresh(false)`, under that guard, with the same argument the GUI root step passes |
| `Feeds` | `refresh_feeds()` |

The F5 key stays TUI-only. The GUI gets no key for refresh.

### D7. Refetch paths keep loaded content and never cache a failure

Each new `refetch_*` helper follows one rule: when the store holds no loaded content, it calls the existing first-load path (after it clears a failed mark); when the store is loaded, it starts a replacement fetch, leaves the loaded value in place, and marks the replacement in flight so a second press starts nothing; the landing replaces the value on success, and on failure keeps it, clears the in-flight mark, and flashes `"Refresh failed"` as `ToastSeverity::Error`.

- `refetch_gui_group_albums` (`store.rs`): in-flight set `gui_music_albums_refreshing: HashSet<(usize, String)>`. The spawn body of `fetch_gui_group_albums` moves into `spawn_gui_group_albums_fetch`, which both call. `handle_gui_group_albums` keeps a `Loaded` value on `Err` when the pair was refreshing.
- `refetch_gui_emby_root_items` (`emby.rs`): in-flight set `gui_emby_roots_refreshing: HashSet<usize>`, the same split and landing rule in `handle_gui_emby_root_items`.
- `refetch_gui_shows_new` (`shows.rs`): in-flight set `gui_shows_new_refreshing: HashSet<String>`; the landing keeps the landed rows on `Err` when refreshing.
- `refetch_tv_upcoming_snapshot` (`dispatch/library/browse/tv.rs`): skips while `TvUpcomingFetchState::InFlight`; otherwise sets `InFlight` and runs the spawn body of `maybe_spawn_tv_upcoming_snapshot`, moved into `spawn_tv_upcoming_snapshot_fetch`. The `TvUpcomingSnapshotFailed` arm must keep an existing snapshot; it changes only if it drops one today.
- `refetch_album_tracks` (`infra/image_fetch/fetch.rs`): skips while loading; otherwise spawns with the cache entry in place. On `Err` it sends a new `MusicEvent::AlbumTracksRefreshFailed { album_id }`, whose arm clears `album_tracks_loading` and flashes. Success lands through `AlbumTracksFetched`, and `insert_capped` already replaces.
- `refetch_series_detail` (`fetch.rs`): skips while loading; on `Err` sends a new `SeriesEvent::DetailRefreshFailed { series_id }`. A new `series_detail_stale: HashSet<String>` marks ids whose cached detail a refresh replaces. In `handle_series_detail_fetched`, a stale id is replaced, not kept first-wins, and the new detail keeps the old `episodes` entries of the seasons it still holds; each such open season becomes stale in `series_season_stale: HashSet<(String, String)>`.
- `refetch_series_season_episodes` (`fetch.rs`): skips while loading; on `Err` sends `SeriesEvent::SeasonEpisodesRefreshFailed { series_id, season_id }`. Success replaces the season's episodes and clears its stale mark.

Every new `MusicEvent` and `SeriesEvent` variant gets an explicit arm in `handle_music_event` / `handle_series_event` (`src/app/dispatch/library/event.rs`), never a wildcard. `src/app/shell/run/drains.rs` matches `SeriesEvent::DetailFetched` by name and sends the rest on, so it needs no new arm.

The first-load paths keep their `unwrap_or_default` behavior. Changing a first load's failure semantics is out of scope.

## Risks / Trade-offs

- [Many failures in one refresh show many toasts.] → Every refresh failure flashes the same text and severity, and a toast equal to a live one replaces it (`toast-notification-semantics`), so a failed refresh shows one toast.
- [The reused refresh paths (Home, Audiobooks, Podcasts, Feeds, feed-group Emby, the Shows root level) keep their own failure reporting, which differs.] → The spec promises only that a failed refresh keeps the rows. Each reused path already keeps its published rows on failure.
- [Feeds' refresh flashes "Refreshing feeds..." and the others do not.] → Accepted. It is the existing Feeds refresh text, and the TUI shows it too.
- [A Shows refresh replaces a stale series detail while its season rows are open.] → The old episodes stay until the new ones land (D7), so open seasons do not empty.
- [The Playlists button loses its "Refreshing…" text.] → The user asked for the icon to match the Library buttons, which show no busy state.
- [`state.rs` is 750 lines.] → The new Music methods go in `state/rows.rs` and the new core code in new modules, so no file passes 800 lines.
