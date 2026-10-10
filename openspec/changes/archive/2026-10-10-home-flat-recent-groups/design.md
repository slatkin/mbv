# Design

## Context

Change `home-recently-added-tree` (archived, PR 930) built Home on `TreeBrowser<HomeTreeTarget>`: two roots, three age branches, expand and collapse. On screen it looks wrong next to the other lists. Before that change, Home was a flat `MediaListCarrier<String>` with one non-selectable `MediaListRow::Heading` over the Continue items. The Feeds list is also flat: `feed_display_rows` emits `Heading(FeedAgeGroup)` and `Spacer` rows, and `feed_age_group` gives `New` (0–1 days) and `Recent` (2–13 days), then older groups.

What stays valid from the archived change: the shell fetch and merge (`HomeRecentRow`, `home_recently_added`, `push_home_content`), `HomeItemTarget`, per-Service row actions and context menus, lazy Audiobookshelf Latest, and tab markers that read Home rows against the launch window.

## Goals / Non-Goals

**Goals:**
- Home is a flat list: group headers and item rows, in the style of the TV and Feeds lists.
- Age groups follow the Feeds rule, `New` and `Recent` only.
- Delete the tree-only types and the tree secondary title.

**Non-Goals:**
- No change to what each Service fetches, to the 14-day storage cutoff, or to markers and the launch window.
- No change to the Feeds list or `feed_age_group` itself.
- No change to the hero beyond what a flat selection needs.

## Decisions

**D1. Reuse the flat media list carrier.** `HomeContent` holds `MediaListCarrier<HomeItemTarget>`. Item rows use the target as their stable identity. `Continue(ItemId)` and `Recent(QueueItemContentId)` are different variants, so the same item under both sections stays two separately selectable rows. Headers are `MediaListRow::Heading`, which the carrier already skips when moving the cursor. Alternative: keep the tree and only restyle it. Rejected: the tree adds expansion state and depth that the user does not want.

**D2. Header structure.** Rows in order: `Heading("Continue watching")`, Continue items, `Spacer`, `Heading("Recently added")`, `Heading("New")`, its items, `Heading("Recent")`, its items. A header with no rows below it is omitted, and so is its spacer. The `Recently added` and age headers use the same heading style, as the user asked for no second style. Assumption: a `Spacer` separates sections and age groups as `feed_display_rows` does. If the TV list uses no spacer, drop it in the projection only.

**D3. Age logic is `feed_age_group` against now.** Reuse `feed_age_group(timestamp, now)`. Keep only `New` and `Recent`. Drop `Older*` and `Unknown`, which also omits items with no valid date as the spec requires. Delete `RecentAge` and `recent_age`. Keep `HomeLatestLaunchWindow` and `RECENTLY_ADDED_CUTOFF_SECS`: the shell store and the tab markers still use them. Trade-off: `New` is no longer "since the last launch", but markers still are.

**D4. The push carries `now_secs`.** The Home owner cannot read the clock, and the projection needs it. `set_recent_rows(rows, launch_window)` becomes `set_recent_rows(rows, now_secs)`, and the shell passes the current time (`current_time_secs`). The owner drops its `launch_window` field and accessor. The grouping updates when the shell next pushes, which is on fetch, refresh, and feed refresh. A list left open past a group edge keeps old groups until then. This is acceptable and matches the Feeds list.

**D5. Input.** Port the flat-list behavior from the pre-change owner (`git show 0128027567:crates/mbv-components/src/home_content.rs`) and the current `home_content/keyboard.rs`: Enter plays the selected item, Ctrl+Enter, Ctrl+W, Delete and `.` act on the selection, and marked items act in display order. Keep the widened shell payloads (`Vec<HomeItemTarget>`). Visual-mode and marking follow the carrier's own surface, as the other flat lists do. No tree-only keys (Right on a root, collapse) remain.

**D6. Hero and restoration.** The selected row is always an item, so the hero resolves the item directly (`hero_content_queue`). No stem fallback. `reset_presentation` and re-anchor select the first item row.

**D7. Delete the tree secondary title.** `TreeNode.secondary`, `with_secondary`, the painter branch, its test, and the `shared-list-components` requirement go. Only Home used them. Flat rows already paint split titles through `media_list_title_parts`.

**D8. Standards findings from the PR 930 review.** Keep two that survive: `handle_home_request` must list its remaining variants instead of a `_` arm, and the test module in `src/app/dispatch/library/load.rs` moves to `load/tests.rs`. New and rewritten comments cite `openspec/specs/…`, not the archived change. The other findings apply to code this change deletes (`tree.rs` tests, `recent_age` tests, tree-only keyboard tests) or rewrites. Do not recreate them in the new code: one projection test, no duplicate two-targets test.

## Risks / Trade-offs

- A big rewrite of files that PR 930 just added. Mitigation: tasks go in small steps, each compiles, and the shell side stays unchanged except the push argument.
- Two heading levels with one style may look flat. If so, a style change is a later design step, not part of this change.
- `New` loses its "since last launch" meaning on Home. The user chose the Feeds rule explicitly.
- PR 930 is open. This change lands on the same branch, so its review sees the final design only if the PR is updated before review.
