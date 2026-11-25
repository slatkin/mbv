# Design

## Context

See `proposal.md` for the motivation and the user decisions. The requirements are in `specs/gui-client/spec.md`.

- **The crate split.** No crate `mbv` exists. `App` is in `crates/mbv-client-core/src/app.rs`, and the binary `mbv` comes from the package `mbv-tui`. The GUI Client's core is `GuiCore { app: App, gui: GuiClientState }` in `crates/mbv-gui/src/gui_core.rs`. Every GUI-only field lives in `GuiClientState` (`crates/mbv-gui/src/gui_core/gui_client_state/state.rs`), and no `gui_*` field is on `App` (ADR 0042, ADR 0043, `1-decouple-client-core` design D4). The core never names a GUI type. It reaches the GUI's stores only through the `ClientLibraryStores` seam (`crates/mbv-client-core/src/app/dispatch/library/stores.rs`): an event arm raises a `ClientLibraryEffect`, and `GuiClientState::apply_library_effect` applies it. The TUI passes no stores, so an effect does nothing there.
- **The header.** Every Library list paints its header through one component, `LibraryListHeader` (`crates/mbv-gui/src/window/components/library_list_header.rs`), from `library_list_column`. Its title line is a `justify-between` flex row: the heading (Service mark or Home title), then the collection dropdown, the Shows view dropdown, or the selection label, then the sort control. Four destinations build its props: `library_list.rs` (Music), `library_shows_list.rs` (Shows), `library_home_list.rs` (Home), `library_lettered_list.rs` (plain Emby, feed-group Emby, Audiobooks, Podcasts, Feeds).
- **Tree open state is window state.** Music keeps `LibraryUiState.open_nodes` and `filter_closed` (`crates/mbv-gui/src/window/library/state.rs`), changed through `set_node_open` (`state/rows.rs`). Shows keeps `shows_ui.open` and `closed` (`crates/mbv-gui/src/window/library/shows.rs`). Outside a filter, a node is open when `open` holds it. Under a filter, a node is open when it has a kept child and `closed` does not hold it (`build_library_rows`, `build_shows_rows`).
- **The GUI Library command.** The enum `GuiLibraryCommand` and the routing pass `dispatch_gui_library_command` are in `crates/mbv-gui/src/gui_core/gui_library.rs`: the routing pass hands the destination commands to the second pass `dispatch_gui_library_destination_command` in `crates/mbv-gui/src/gui_core/gui_library/destination.rs`, which routes into the three group passes `dispatch_gui_home_shows_command`, `dispatch_gui_provider_command`, and `dispatch_gui_panel_command`. Every pass lists every variant — the ones it does not own in one logged arm, not a wildcard — so a new variant is a compile error in every pass. The window's mirror enum and its `From` conversion are in `crates/mbv-gui/src/bridge/library.rs`.
- **Children load differently.** A Music artist's albums come with the group album lists, so an open artist needs no load. The GUI's group album store is `GuiClientState.music_albums`, filled by `gui_library::store::fetch_group_albums` for the groups that `gui_library::projection::selected_groups` names over `projection::effective_music_set`. An album's tracks load through `OpenAlbum`, then `App::fetch_album_tracks` (`crates/mbv-client-core/src/app/infra/image_fetch/fetch.rs`). A Shows series' seasons come from the core's `series_detail_cache`, which loads only through `App::fetch_series_detail` (same file). Today a pick (`ShowsSelect`, `gui_library::tv::select_tv_series`) or the ensure pass (`gui_library/ensure.rs`, `ShowsLoads`) reaches it. The cache is capped at 128 entries (`MAX_SERIES_DETAIL_CACHE`, issue #917), and `cache_series_detail` keeps the first entry for an id (`crates/mbv-client-core/src/app/dispatch/library/cursor.rs`). Each detail landing also loads the first season's episodes (`handle_series_detail_fetched`, same file).
- **Client-only fetches land in the GUI's own stores.** The group albums (`store::fetch_group_albums`), a plain Emby library's root list (`emby::fetch_emby_root_items`), and a Shows library's New episodes (`shows::spawn_shows_new`) send `LibraryFetchEvent`s. `GuiCore::drain_client_event` takes each one and calls `gui_library::client_events::install_client_event`, whose mark functions `group_albums_mark`, `emby_roots_mark`, and `shows_new_request` turn the result into the store value. A failure today replaces the store value (`Failed`, or an empty New list). These mark functions, not `App` handlers, are the landings a refresh must change.
- **GUI loads are idempotent ensure steps.** `GuiCore::ensure_gui_destination` (`gui_library/ensure.rs`) runs root, selection, and dependents steps whose guards skip a store that is `Loading` or `Loaded`. Nothing in the GUI reloads a loaded store. The TUI's F5 (`App::refresh_current_view`, `crates/mbv-client-core/src/app/dispatch/library/load.rs`) reads `self.tab` and is TUI-only.
- **Several core fetches turn an error into an empty result.** `fetch_album_tracks`, `fetch_series_detail`, and `fetch_series_season_episodes` use `unwrap_or_default`. A refetch through them would replace good content with nothing when the Service fails.
- **The Upcoming snapshot.** `App::maybe_spawn_tv_upcoming_snapshot` (`crates/mbv-client-core/src/app/dispatch/library/browse/tv.rs`) does nothing while a snapshot exists. Its failure arm `tv_upcoming_snapshot_failed` marks the fetch `Failed` and keeps the stored snapshot.
- **The Playlists screen** paints a text `SettingsButton` ("Refresh"/"Refreshing…") in `tree_column` (`crates/mbv-gui/src/window/components/playlists_panel.rs`), which sends `GuiPlaylistsCommand::Refresh { id: None }`.

## Goals / Non-Goals

**Goals:**

- One header layout owner: every destination gets the refresh button, and the tree lists get the tree buttons, through `LibraryListHeader` props.
- Expand all and Collapse all are window-only. They change open state and send no command. The only command near them is the Shows in-view report, which every open Shows row drives.
- A refresh never blanks a loaded list and never replaces good content with the result of a failed fetch, on every path this change adds.
- One bounded loader for Shows children, driven by what is open and in view.
- No GUI state on `App`, and no GUI name in a core function.

**Non-Goals:**

- No keybind, no status bar hint, no TUI change.
- The Music tree loads no tracks for albums in view. Expand all does not open albums, so no Music in-view loader is needed.
- The existing refresh paths that this change reuses (Home, Audiobooks, Podcasts, Feeds, the Shows root level) keep their own loading and failure behavior.
- A feed-group refresh does not re-read the folder pills or their counts (D7a).
- No busy state on any refresh button, the Playlists one included.
- The Hero's Workspace reloads only through the tree rows: when its series is an open row in view, its seasons reload with that row.

## Decisions

### D1. The title line gets one trailing control group

`LibraryListHeader`'s title line keeps `justify-between` with two children: the heading, and a new trailing `div` (`flex flex-none items-center`, with a 6 px gap inside the group in place of the line's 12 px gap). The group holds, in this order, the refresh button, then the Expand all and Collapse all buttons when the destination passes tree actions, then the dropdown or view dropdown or selection label (unchanged rules), then the sort control. This keeps the dropdown at the line's right end where there is no sort control, and puts the tree buttons directly left of the dropdown, as the user asked.

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

A new component `crates/mbv-gui/src/window/components/header_icon_button.rs` paints a 28 × 28 px `button` (the title line's height) with the sort control's border (`1px solid var(--canvas-border)`, 6 px radius), no fill at rest, `var(--canvas-muted)` icon colour, and a 14 px icon at stroke width 1.6. Its props are `label: &'static str` (the `aria-label` and the `title` tooltip), `hook: &'static str` (a `data-*` attribute name), `onpress: Callback<()>`, and `children: Element` (the icon). The header uses it for all three buttons, with the labels "Refresh", "Expand all", and "Collapse all" and the hooks `data-library-refresh`, `data-library-expand-all`, and `data-library-collapse-all`.

The Playlists screen's `tree_column` replaces its `SettingsButton` with this part (label "Refresh", hook `data-playlists-refresh`, icon `RefreshCcw`). The `refresh_label` text goes. The command stays `GuiPlaylistsCommand::Refresh { id: None }`. The open change `gui-playlists-in-queue-column` rewrites other parts of `playlists_panel.rs`. This edit touches only the header button, so the two changes merge cleanly in either order.

### D3. Three Lucide icons join `window/icons.rs`

The `lucide_icons!` table in `crates/mbv-gui/src/window/icons.rs` gets three entries, with the child elements copied from Lucide (the same paths in `@lucide/svelte` 1.70.0, which the module vendors; the ISC notice is already present):

- `RefreshCcw("refresh-ccw")`: `path M21 12a9 9 0 0 0-9-9 9.75 9.75 0 0 0-6.74 2.74L3 8`, `path M3 3v5h5`, `path M3 12a9 9 0 0 0 9 9 9.75 9.75 0 0 0 6.74-2.74L21 16`, `path M16 16h5v5`. The user chose this icon.
- `ChevronsUpDown("chevrons-up-down")` for Expand all: `path m7 15 5 5 5-5`, `path m7 9 5-5 5 5`.
- `ChevronsDownUp("chevrons-down-up")` for Collapse all: `path m7 20 5-5 5 5`, `path m7 4 5 5 5-5`.

The module doc's icon count goes up by three. No dependency changes.

### D4. Expand all and Collapse all change window open state only

Music, in `crates/mbv-gui/src/window/library/state/rows.rs`, gets two methods. `expand_all_music(&self)` reads `self.rows_of(&ui)`, and for each `LibraryRow::Artist` row it adds the row id to `open_nodes` and removes it from `filter_closed`. `collapse_all_music(&self)` clears `open_nodes`, and while the filter text is not empty it adds the id of every painted `Artist` and `Album` row to `filter_closed`.

Shows, in `crates/mbv-gui/src/window/library/shows.rs`, gets the same pair. `expand_all_shows(&self)` reads `self.shows_rows()`, and for each `ShowsListRow::Series` row it adds the row id to `shows_ui.open` and removes it from `shows_ui.closed`. `collapse_all_shows(&self)` clears `shows_ui.open`, and while the filter text is not empty it adds the id of every painted `Series` and `Season` row to `shows_ui.closed`.

Neither method sends a command, picks a row, or touches the Hero. "Painted rows" are the rows the current filter keeps, so a filter limits both actions to its matches (spec). Adding the artist ids to `open_nodes` under a filter is what keeps them open after the filter clears. Music passes `tree: Some(..)` always. Shows passes it only while the view is `GuiShowsView::All`.

*Alternative:* open every node in the carried content, not only the painted rows. Rejected: under a filter it would open hidden series, and in Shows it would widen the set the in-view loader reads for no visible gain.

### D5. Open Shows rows in view drive one bounded loader in the GUI core

**Window.** `library_shows_list.rs` computes, in the All view, the open nodes among the painted window (`list_window(rows, scroll_top, viewport_height)`): the `source_id` of each open `Series` row, and the `(series_id, source_id)` pair of each open `Season` row, in paint order. In the New and Upcoming views both lists are empty, so a view change stops the loader. A `use_effect(use_reactive(..))` keyed on that value sends it through a new `LibraryState::report_shows_open_in_view`, as the row-art report does. The report fires when a node opens or closes, when the view scrolls, and when rows land.

**Command.** `GuiLibraryCommand::ShowsOpenInView { lib_idx: usize, series: Vec<String>, seasons: Vec<(String, String)> }` goes in both enums, with its conversion arm in `bridge/library.rs`. It is a destination command: its arm is in `dispatch_gui_library_destination_command`, and the routing pass names it in its hand-over list. A report for a library that is not a Shows library is an ignored command.

**State.** `GuiClientState` gets three fields, named as the struct's other fields are, with no `gui_` prefix: `shows_open_in_view: Option<GuiShowsOpenInView>` (one Shows list paints at a time, so each report replaces the last), `shows_open_series_loads: HashSet<String>`, and `shows_open_season_loads: HashSet<(String, String)>` (the loads this loader started). The manual `Default` for `GuiClientState` sets all three empty.

**Pump.** A new module `crates/mbv-gui/src/gui_core/gui_library/shows_open_loads.rs` holds `const SHOWS_OPEN_LOAD_LIMIT: usize = 4` and the free function `pump_shows_open_loads(app: &mut App, gui: &mut GuiClientState)`. `GuiCore::pump_gui_shows_open_loads(&mut self)` calls it over its own `app` and `gui`. The pump does nothing while the active destination (`gui_library::tabs::active_destination`) is not the report's Shows library. Otherwise it first drops each started series load whose detail has landed or whose fetch the core no longer runs (`App::series_detail_loading`, `App::series_season_loading`; a series whose detail landed while its season fetch still runs counts as landed): it retains a series entry only while `App::series_detail` holds no detail for it AND a fetch runs for it. Then, while the loader's in-flight count is under the limit, it starts the next missing load in report order. A series is missing when `App::series_detail` holds no detail for it and no fetch runs for it; the pump starts it through `App::fetch_series_detail_quiet` (D6). A season is missing when its series' detail is cached, the detail's `episodes` map lacks the season, and no fetch runs for it; the pump starts it through `App::fetch_series_season_episodes`. Group 5 widens "missing" to stale entries (D9). A row that leaves the view drops out of the next report, so its load never starts (spec).

**When the pump runs.** It runs on each `ShowsOpenInView` command, after each `Refresh` of the Shows All view (D7), and after each series event landing. The core cannot call a GUI function from `handle_series_event`, so the landing reaches the pump through the store seam. `ClientLibraryEffect` gets a new variant `SeriesContentChanged`. `handle_series_event` takes the `stores` argument that `handle_lib_event` already holds (as `handle_music_event` does), and after each arm it raises `SeriesContentChanged` through `apply_library_effects`. `GuiClientState::apply_library_effect` applies it by calling `shows_open_loads::pump_shows_open_loads(app, self)`. `handle_series_event` names no GUI type. The TUI's drain passes no stores, so the effect does nothing there.

The cache cap stays at 128. The view never shows more than 128 open series, so an in-view series is never evicted by its own neighbours. A series evicted after it scrolled out reloads when it comes back.

*Alternatives:* loading every series at once (the user rejected it because of the cap); a new select-free command per expand press (it would still need the cap fix and a request queue).

### D6. The first-season skip is a core-owned quiet mark

`handle_series_detail_fetched` starts the first season's episode fetch on each landing. Expand all keeps seasons closed, so those requests would be wasted. The skip must not read GUI state, so it is a core fetch mode, not a GUI query. `App` gets the field `series_detail_quiet: HashSet<String>` and the method `fetch_series_detail_quiet(&mut self, series_id: &str)` in `crates/mbv-client-core/src/app/infra/image_fetch/fetch.rs`. That method runs `fetch_series_detail` and, when the fetch started, records the id in `series_detail_quiet`. In `handle_series_detail_fetched` (`cursor.rs`), a landing whose id is in `series_detail_quiet` removes the mark and does not start the first-season fetch. The pending season expansions still run. A plain `fetch_series_detail` call removes the id from `series_detail_quiet` before its early return, so a pick of that series while the quiet load runs still loads the first season. A pick of a series whose quiet detail already landed is also safe, because the GUI ensure pass (`ShowsLoads::ensure_dependents`) then loads the selected season's episodes. When a fetch is already running for the id, a quiet call that joins a running plain fetch records nothing (the landing stays plain), and a quiet call that joins a running quiet fetch keeps the existing mark (the landing stays quiet).

*Alternative:* a seam query on `ClientLibraryStores` ("does this Client want the first season of this series?"). Rejected: it adds a second trait method that every Client must answer, for a fact that the fetch's caller already knows when it starts the fetch.

### D7. One `Refresh` command, one per-destination dispatcher

**Window.** Each destination passes `on_refresh` to the header. Home, Shows, and the lettered lists call `LibraryState::refresh_library_list()`, which sends `Refresh { album_ids: Vec::new() }`. Music calls `LibraryState::refresh_music_list()` in `state/rows.rs`, which reads the painted rows and the Music scroll (`music_scroll`), takes `list_window(rows, scroll_top, viewport_height)`, and sends the album ids of the open `Album` rows in that window. The window keeps its pick, filter, scroll, and open state.

**Command.** `GuiLibraryCommand::Refresh { album_ids: Vec<String> }` goes in both enums, with its conversion arm. It is a destination command, with its arm in `dispatch_gui_library_destination_command`, which calls `GuiCore::refresh_gui_destination(album_ids)`.

**Scope of a tree refresh (spec).** A tree refresh reloads the children of the open parent rows in view, and starts no load for an open row out of view. Music gets the in-view open album ids from the window. Shows gets the in-view open series and seasons from the last `shows_open_in_view` report, which the window keeps current on each scroll.

**Dispatcher.** A new module `crates/mbv-gui/src/gui_core/gui_library/refresh.rs` holds `GuiCore::refresh_gui_destination`. It resolves `self.gui_active_destination()` (`None` gives an ignored-command log) and matches `GuiDestinationKind` exhaustively, with no wildcard arm:

| Destination | Refresh |
| --- | --- |
| `Home` | `App::refresh_home_content()`, the Home arm of `refresh_current_view` moved into its own `pub` method that both call |
| `Music { lib_idx }` | `store::refetch_group_albums(app, gui, lib_idx, group)` for each group that `projection::selected_groups` names over `projection::effective_music_set`, then `App::refetch_album_tracks(id)` for each id in `album_ids` |
| `Shows` in All | `App::refresh_lib(lib_idx)` (the root level's in-place refresh), then `App::mark_series_detail_stale(id)` for each series and `App::mark_series_season_stale(series_id, season_id)` for each season in `gui.shows_open_in_view`, then the pump (D5) |
| `Shows` in New | `shows::refetch_shows_new(app, gui, library_id)` |
| `Shows` in Upcoming | `App::refetch_tv_upcoming_snapshot(lib_idx)` |
| `Library` in feed-group mode | `GuiCore::refresh_gui_feed_group(lib_idx)`, which refetches the checked targets and keeps their rows (D7a) |
| `Library` plain | `emby::refetch_emby_root_items(app, gui, lib_idx)` |
| `Audiobooks { lib_idx }` | `App::audiobookshelf_book_refresh()`, under the same active-tab guard that `ensure_gui_destination` uses |
| `Podcasts { lib_idx }` | `App::audiobookshelf_refresh(false)`, under that guard, with the same argument the GUI root step passes |
| `Feeds` | `App::refresh_feeds()` |

The F5 key stays TUI-only. The GUI gets no key for refresh.

### D7a. A feed-group refresh refetches its items and keeps the rows it paints

The TUI's path is wrong for this destination. `App::refresh_lib` refreshes the hidden root folder listing, and `App::complete_feed_home_video_refresh` (`crates/mbv-client-core/src/app/dispatch/feeds/feed.rs`) then clears `feed_home_video.items`. The GUI paints its feed-group rows from that map, so the press would blank the list until its ensure pass refetched every target. The user rejected that blank. The GUI therefore refetches the targets themselves and never clears the map.

`GuiClientState` gets `emby_feed_refresh_targets: HashMap<usize, Vec<FeedHomeVideoSelection>>`, the targets one library's refresh wants, in paint order. `GuiCore::refresh_gui_feed_group(lib_idx)` fills that queue with the targets `emby::emby_feed_effective_set` names (All, or each checked folder) and clears the library's failed claims, so the press retries a target that failed before.

`ensure_emby_feed_fetch` (`gui_library/emby.rs`) takes a queued target before its walk over the uncached targets, and it takes it even while `feed_home_video.items` holds that target's list, which the uncached walk skips. The fetch starts through the existing `App::spawn_feed_home_video_fetch`, and the queue drops the target when the fetch starts. The one-fetch-at-a-time rule stays: `feed_home_video.fetching` holds one target, and the next queued target starts on a later frame.

A landing replaces that target's items through the existing `FeedHomeVideoSelectionLoaded` arm, so the painted rows change only when the new rows arrive. A failure keeps the old items, the existing `FeedHomeVideoSelectionFailed` arm marks the target failed through `ClientLibraryEffect::FeedTargetFailed`, and both that arm and `FeedTargetLanded` remove the target from the refresh queue.

This refresh does not re-read the folder pills or their counts; a folder added on the server appears at the destination's next own listing load. Refreshing the listing would clear the items and bring the blank back.

### D8. GUI store refetches keep loaded content and report a failure

The three GUI stores refetch with one rule. When the store holds no loaded value, the refetch clears a failed mark and calls the existing first-load path. When the store holds a loaded value, the refetch leaves that value in place, records the key in an in-flight set on `GuiClientState`, and spawns the fetch, so a second press while that fetch runs starts nothing. When no Emby client exists, the refetch does nothing.

- Group albums (`gui_library/store.rs`): `refetch_group_albums`, with the in-flight set `music_albums_refreshing: HashSet<(usize, String)>`. The spawn body of `fetch_group_albums` moves into `spawn_group_albums_fetch`, which both call.
- Plain Emby roots (`gui_library/emby.rs`): `refetch_emby_root_items`, with `emby_roots_refreshing: HashSet<usize>`. The spawn body of `fetch_emby_root_items` moves into `spawn_emby_root_items_fetch`.
- Shows New (`gui_library/shows.rs`): `refetch_shows_new`, with `shows_new_refreshing: HashSet<String>`. The spawn body of `spawn_shows_new` moves into `start_shows_new_fetch`, which runs without the existing-entry guard.

The landing rule lives in `client_events::install_client_event`. For a key in its in-flight set, a success replaces the stored value and a failure keeps it, and both remove the key. For any other key the mark functions `group_albums_mark`, `emby_roots_mark`, and `shows_new_request` keep today's first-load rule. `install_client_event` returns a `ClientEventLanding` value (`Installed` or `RefreshFailed`). `GuiCore::drain_client_event`, which holds the `App`, flashes `"Refresh failed"` as `ToastSeverity::Error` through `App::flash` on `RefreshFailed`. The test-support callers of `install_client_event` ignore the value.

### D9. Core refetches keep loaded content and never cache a failure

The core refetches live in `crates/mbv-client-core/src/app/`. They are neutral `App` methods that any Client can call, and none of them has a `gui_` name.

- **Album tracks** (`infra/image_fetch/fetch.rs`): `refetch_album_tracks(album_id)` calls `fetch_album_tracks` when the cache lacks the album, and skips while `album_tracks_loading` holds it. Otherwise it marks the album loading and spawns the fetch with the cache entry in place. On success the fetch sends the existing `MusicEvent::AlbumTracksFetched`, whose landing replaces the entry through `insert_capped`. On failure it sends a new `MusicEvent::AlbumTracksRefreshFailed { album_id }`, whose arm in `handle_music_event` clears `album_tracks_loading` and flashes `"Refresh failed"`.
- **Series detail** (`fetch.rs`, `dispatch/library/cursor.rs`): the core gets `series_detail_stale: HashSet<String>`, set by `mark_series_detail_stale(id)` only for an id the cache holds, and the read `series_detail_stale(id) -> bool`. `refetch_series_detail(id)` skips while a fetch runs for the id. Otherwise it marks the id loading and quiet (D6) and spawns the fetch with the cache entry in place. On failure it sends a new `SeriesEvent::DetailRefreshFailed { series_id }`, whose arm clears the loading, quiet, and stale marks, keeps the cached detail, and flashes. In `handle_series_detail_fetched`, a landing for a stale id replaces the cached detail instead of the first-wins rule, keeps the old `episodes` entries of the seasons the new detail still holds, and clears the stale mark.
- **Season episodes** (`fetch.rs`): the core gets `series_season_stale: HashSet<(String, String)>`, set by `mark_series_season_stale`, and the read `series_season_stale(series_id, season_id) -> bool`. `refetch_series_season_episodes` skips while a fetch runs. On failure it sends `SeriesEvent::SeasonEpisodesRefreshFailed { series_id, season_id }`, whose arm clears the loading and stale marks, keeps the episodes, and flashes. A success replaces the season's episodes and clears its stale mark.
- **The pump and stale entries.** The pump (D5) treats a stale series or season in the report as missing, and it starts it through `refetch_series_detail` or `refetch_series_season_episodes`. A stale entry that leaves the view stays stale and reloads when it comes back. A failed refetch clears the stale mark, so a failure never retries on each landing.
- **Upcoming** (`dispatch/library/browse/tv.rs`): `refetch_tv_upcoming_snapshot(lib_idx)` calls `maybe_spawn_tv_upcoming_snapshot` when no snapshot exists, and skips while the fetch state is `InFlight`. Otherwise it sets `InFlight` and runs the spawn body of `maybe_spawn_tv_upcoming_snapshot`, moved into `spawn_tv_upcoming_snapshot_fetch`. The existing failure arm `tv_upcoming_snapshot_failed` already keeps the snapshot and marks the fetch `Failed`, so the Shows list keeps its rows and shows its existing "Upcoming fetch failed" line. A test reads the in-flight state through a new `App::tv_upcoming_fetch_in_flight_for_test(library_id) -> bool` in `state/shell_reads.rs`, behind `cfg(any(test, feature = "test"))`, beside `series_detail_loading_for_test`.
- **Home**: `refresh_home_content()` is the Home arm of `refresh_current_view` (`dispatch/library/load.rs`), moved into its own `pub` method. Its failure already keeps the Home content and flashes.

Every new `MusicEvent` and `SeriesEvent` variant gets an explicit arm in `handle_music_event` or `handle_series_event` (`crates/mbv-client-core/src/app/dispatch/library/event.rs`), never a wildcard. The TUI's drain (`crates/mbv-tui/src/tui/run/drains.rs`) matches `SeriesEvent::DetailFetched` by name and passes every other event on to `handle_inline_search_lib_event`, so it needs no new arm. The first-load paths keep their `unwrap_or_default` behavior. A change to a first load's failure semantics is out of scope.

## Risks / Trade-offs

- [Many failures in one refresh show many toasts.] → Every refresh failure flashes the same text and severity, and a toast equal to a live one replaces it (`toast-notification-semantics`), so a failed refresh shows one toast.
- [The reused refresh paths (Home, Audiobooks, Podcasts, Feeds, the Shows root level) keep their own failure reporting, which differs.] → The spec promises only that a failed refresh keeps the rows. Each reused path keeps its published rows on failure.
- [A feed-group refresh does not re-read the folder pills.] → Accepted: the alternative clears the painted rows (D7a), and a new folder appears at the next listing load.
- [Feeds' refresh flashes "Refreshing feeds..." and the others do not.] → Accepted. It is the existing Feeds refresh text, and the TUI shows it too.
- [A Shows refresh replaces a stale series detail while its season rows are open.] → The old episodes stay until the new ones land (D9), so open seasons do not empty.
- [The Playlists button loses its "Refreshing…" text.] → The user asked for the icon to match the Library buttons, which show no busy state.
- [`window/library/state.rs` is 755 lines, and `gui_core/gui_library.rs` is 745 lines.] → The new Music methods go in `state/rows.rs`, and the new GUI core code goes in the new modules `shows_open_loads.rs` and `refresh.rs`. The two new variants add their docs and arms to `gui_library.rs`. If that file passes 800 lines, the implementer moves the destination pass into its own module along that seam.
- [`ShowsOpenInView` fires on each scroll and logs at info, as `RowsInView` does.] → Accepted. The report is small, and the log matches the existing row-art report.
