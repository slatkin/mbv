# Invariant 29 — A play activation resolves the autoload policy one way

**Scope:** `select_item`, `play_tv_episode`, and `play_autoloaded_children`
(`src/app/dispatch/navigation.rs`); the `TvEpisodeActivate` arm
(`src/app/shell/tv_workspace.rs`); the `EmbyLibraryActivate` arm
(`src/app/shell/emby_library.rs`).

## The invariant

1. When `mpv.autoload` is on, an activation that plays one item of a folder
   fills the queue with that folder's direct playable children, ordered by
   natural sort key, starting at the activated item.
2. When the policy is off, the same activation queues that item alone.
3. One resolver applies that rule to every browse-like activation. A view
   supplies the parent folder; it never decides the queue itself.
4. A single-item play is the fallback, not the rule: it is correct only when
   the policy is off, the parent fetch fails, or the activated item is not in
   its resolved parent (a flat Latest/Upcoming row, a playlist entry).

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
  group's parent) and keeps the existing queue source.
- `play_tv_episode` supplies the episode's season, resolved from the shell's
  cached series detail — the same cache the TV Workspace renders from — and
  passes `QueueSource::Series`.
- Guard tests in `src/app/dispatch/actions/route_tests/library_routing.rs`:
  `tv_episode_play_queues_the_cached_season_when_autoload_is_on` and
  `tv_episode_play_queues_only_that_episode_when_autoload_is_off`, plus the
  browse-path gate test `library_autoplay_on_a_populated_queue_does_not_raise_the_replace_modal`.

## Where it currently fails / how it could regress

- The TV Workspace's flat Latest/Upcoming rows carry no cached season, so they
  take the single-item path even with the policy on. That is a known gap, not
  a decision recorded anywhere else.
- Other playback entry points (context menu play, playlists, series
  continuation, feeds) do not autoload by design. A new browse-like Enter path
  that calls `play_item` directly would join them silently; only the TV path
  has a guard test today.
