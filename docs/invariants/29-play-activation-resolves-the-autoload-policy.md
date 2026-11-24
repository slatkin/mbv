# Invariant 29 — A play activation resolves the autoload policy one way

**Scope:** `select_item`, `play_tv_episode`, `play_autoloaded_siblings`,
`play_autoloaded_containing_folder`, and `play_autoloaded_children`
(`mbv_client_core::dispatch::navigation`); `home_play_target` and `play_home_cw_item`
(`mbv_client_core::dispatch::home`); the `TvEpisodeActivate` arm
(`crates/mbv-tui/src/tui/tv_workspace.rs`); the `EmbyLibraryActivate` arm
(`crates/mbv-tui/src/tui/emby_library.rs`).

## The invariant

1. When `mpv.autoload` is on, an activation that plays one item of a folder
   fills the queue with that folder's direct playable children, ordered by
   natural sort key, starting at the activated item.
2. When the policy is off, the same activation queues that item alone.
3. One resolver applies that rule to every browse-like activation. A view
   supplies the parent folder; it never decides the queue itself.
4. A flat row — a Home `Continue watching` or `Recently added` row, a Shows
   `New` or `Upcoming` row, any Latest list's row — names no browse level, so
   its own containing item resolves the parent: an episode's season, a home
   video's folder. A row that names no containing item, and a playlist entry,
   still take the single-item play.
5. A single-item play is the fallback, not the rule: it is correct only when
   the policy is off, the parent fetch fails, or the activated item has no
   resolved parent holding it.

## Why it matters

The policy is a user setting, not a view feature. A playback entry point that
calls `play_item` directly looks correct in isolation and silently drops the
setting: the item plays, so nothing errors and no test fails. The TV
Workspace's episode Enter did exactly that for a season of releases while the
generic browser kept autoloading.

## How the code maintains it today

- `play_autoloaded_children(parent_id, fresh, queue_source)` is the one
  resolver: it checks the policy, fetches the parent's direct playable
  children, drops folders, sorts naturally, locates `fresh`, and hands the list
  to `play_items_routed` (which routes the replacement to the Player owner).
  It returns `false` — leaving playback to the caller — in the four fallback
  cases above.
- `select_item` supplies the browse level's parent (or the feed home video
  group's parent) and keeps the existing queue source. When that parent does
  not hold the item — a flat row in a Latest list — it falls back to the
  item's own containing item.
- `play_autoloaded_containing_folder(fresh, queue_source)` is the flat-row
  resolution: it hands `fresh.parent_id` to `play_autoloaded_children`, and
  returns `false` when the item names no containing item.
- `home_play_target` and `play_home_cw_item` (the latter after refreshing the
  item) take that resolution before `play_item`, so a Home row autoloads its
  container in both Clients. A resuming item still starts at its position,
  because the queue's start item carries it.
- `play_tv_episode` supplies the episode's season, resolved from the client
  core's cached series detail — the same cache the TV Workspace renders from — and
  from the episode's own containing item when no cache entry holds the
  series, which is every Latest row. It passes `QueueSource::Series`.
- Guard tests in `mbv_client_core::app::dispatch::actions::route_tests::library_routing`:
  `tv_episode_play_queues_the_cached_season_when_autoload_is_on` and
  `tv_episode_play_queues_only_that_episode_when_autoload_is_off`, plus the
  browse-path gate test `library_autoplay_on_a_populated_queue_does_not_raise_the_replace_modal`.

## Where it currently fails / how it could regress

- An item whose payload declares no containing item (`ParentId` empty) takes
  the single-item path even with the policy on. Queue items the Owner restored
  from an older snapshot carry the field as empty until their next refresh.
  Emby omits `ParentId` from every response whose request supplies a `Fields`
  list that does not name it — the flat-row fetches (`get_continue_watching`,
  `get_latest`, `get_latest_episodes`) and the `get_items_by_ids` refresh all
  name it for this reason; a new Emby fetch that feeds a play activation must
  name it too, or the flat-row resolution silently degrades to the
  single-item play.
- A movie's containing item is its library, so autoloading a movie from a Home
  or New row queues the whole library. That is the same queue the policy
  produces when the movie plays from the library's own root listing, not a
  separate rule.
- Other playback entry points (context menu play, playlists, series
  continuation, feeds) do not autoload by design. A new browse-like Enter path
  that calls `play_item` directly would join them silently; only the TV path
  has a guard test today.
