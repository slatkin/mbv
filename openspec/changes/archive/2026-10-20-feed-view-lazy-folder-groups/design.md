# Design

## Context

Today the feed view builds everything eagerly through one aggregate: `spawn_feed_home_video_aggregate` fetches every video under the root (unplayed-only today) via the ranged item fetch, then calls `get_ancestors` once per video to map it to a root folder, and stores the result as `FeedHomeVideoState { all_items, groups }`. The All pill and every folder pill paint from that one state; the three removal paths (`remove_watched_item_from_library`, `remove_played_item_from_library`, `refresh_completed_feed_home_videos`) strip just-played items out of it. The New pill paints the shared Latest snapshot and is untouched by this change.

Two facts make the replacement cheap: the root folder listing the view already performs is the natural source for the pills themselves, and mbv already parses `child_count` from Emby items while other mbv queries already request `ChildCount,RecursiveItemCount` in their `Fields`. The counts that decide pill membership come from a small per-folder videos-only query against the Emby client (D1, settled from documentation in `api-findings.md`). `FeedHomeVideoState` is session memory — restore re-seeds it with a loading flag and refetches — so no persisted shape changes.

## Goals / Non-Goals

**Goals:**
- Pill membership and contents come from the folder listing, per-folder video counts, and per-selection fetches; no full-library eager build, no per-video folder-membership round-trips.
- Watched videos appear in All and in their folders; watched-only folders keep their pills.
- Marking played (menu, multi-select, playback completion) keeps the row.
- Delete the aggregate machinery and its event rather than deprecating it.

**Non-Goals:**
- No change to the New pill, its source, or `destination-latest-modes`.
- No change to unplayed-only behavior for ordinary (non-feed-view) libraries.
- No Feeds (RSS) tab, config, ctrl-protocol, or daemon changes; no styling changes.

## Decisions

**D1 — Pill membership from per-folder videos-only counts; choice (b) settled from documentation.** Two candidates were weighed: (a) add `ChildCount,RecursiveItemCount` to the ranged listing's `Fields` and treat a positive recursive count as "holds videos" — zero extra calls, but the count spans all item types, so a folder holding only subfolders would produce a pill that lists no videos; (b) one tiny count-only query per folder (`IncludeItemTypes=Video`, recursive, limit 1, read the total) — exact, at M small calls where M is the folder count. **(b) is chosen.** `api-findings.md` records why: no public Emby source defines what `RecursiveItemCount` counts, and the available evidence says it likely counts sub-folders and all item types, so it cannot prove a folder holds a video.

**D2 — Lazy per-selection fetch with session cache.** Selecting a folder pill fetches that folder's videos at selection time; selecting All runs the ranged fetch the aggregate used today (same sort `DateCreated` ascending, same initial-page-then-total refill shape) minus the unplayed filter and minus all grouping. Loaded selections stay cached for the session so pill ping-pong does not refetch; a refresh drops the caches, re-reads the folder listing (counts refresh), and refetches the current selection, leaving unloaded pills unloaded — matching the spec's refresh scenario.

**D3 — State shape.** `FeedHomeVideoState` drops `all_items` and `groups`. It holds the current selection (All or a folder id), that selection's loaded items, its loading flag, and the existing cursor/scroll/selected-group fields. Group switching resolves the folder id directly from the root listing, which simplifies the selected-parent resolution. Pill-row projection (`feed_groups`/`feed_group_ids`) derives from the root listing items filtered by their per-folder video count instead of the state's group list; the empty-aggregate rationale in `is_feed_home_video_group_view` dies with the aggregate, though the "state present ⇒ pill row painted" robustness stays.

**D4 — Removal paths lose their feed-view branches.** The feed-view arms of `remove_watched_item_from_library` and `remove_played_item_from_library` go away; `refresh_completed_feed_home_videos` loses its feed-view cache stripping entirely (its Home refresh role stays). `remove_item_from_feed_home_video_cache` is deleted once it loses its last feed-view caller. The unplayed-only branches for ordinary libraries remain untouched.

**D5 — Aggregate machinery deleted.** `spawn_feed_home_video_aggregate`, the `FeedHomeVideoAggregated` event and its dispatch arm, the `maybe_aggregate_feed_*` / `maybe_refresh_feed_groups_after_refresh` re-spawn chain, and the aggregate-only helpers (`feed_home_video_visible_group_count` consumers as needed) go away. What replaces them: the existing root listing (already paginated and refresh-capable) feeds pills and counts; selection events drive the lazy fetches; refresh drives the listing refetch. `ensure_feed_home_video_root_loaded` keeps its role of guaranteeing a folder listing exists.

**D6 — Findings from documentation before code.** The D1 choice and the count-field semantics were settled from public Emby documentation and community threads, recorded in `api-findings.md` before implementation tasks proceed; no live server probe is needed.

## Risks / Trade-offs

- [Count-only queries add per-folder round trips] → M small requests run per folder (parallelizable; folders with `ChildCount == Some(0)` can be skipped). Accepted: the aggregate they replace already did M ancestor round-trips per video across the whole library, so this is a strict reduction in total server work.
- [First-selection latency] → a freshly selected pill waits on its fetch. This is strictly better than today, where every list waits for the whole-library aggregate before anything paints; the existing loading state covers it.
- [State rework touches selection/cursor logic] → the clamping helpers and identity-gated position re-seed survive unchanged in shape; the risk is confined to the feed-view tests, which are event-level and mostly additive here.

## Migration Plan

Single PR; no persisted state, config, or protocol migration — `FeedHomeVideoState` is rebuilt from the listing on every load and restore. Rollback is reverting the PR.

## Open Questions

None.
