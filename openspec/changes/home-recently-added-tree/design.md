# Design

## Context

See proposal.md for motivation and specs/ for the required behavior.

Current state:

- `HomeContent` (`crates/mbv-components/src/home_content.rs`) is a flat
  `MediaListCarrier<String>` over Emby Continue Watching. Its target is the
  bare `item.id()` string. Its requests carry
  `HomeRowTarget { item_id, source, from_continue_watching: bool }`.
- Continue Watching is fetched with `get_continue_watching(20)` in two places:
  `App::fetch_home` (`src/app/dispatch/library/load.rs`) and
  `EmbyClient::load_startup_data_bounded` (`crates/mbv-emby/src/client_auth.rs`).
- Destination Latest sources:
  - **Emby non-TV:** `get_latest(lib, 30)`, fetched lazily into
    `Model::tv_latest_snapshots`.
  - **Emby TV:** `get_latest_episodes(lib, 30)`, fetched lazily into the TV level.
  - **Audiobookshelf podcasts:** the `/personalized` Newest Episodes shelf,
    fetched for every podcast library at startup
    (`start_audiobookshelf_catalog_fetches`) and on refresh, into
    `App::audiobookshelf_shelf_cache`.
  - **Feeds:** `feed_tab.all_entries` (no Latest fetch).
- `Model::destination_latest_marker` reads three different places. Emby reads
  `snapshot.has_new_content`, Audiobookshelf reads the shelf cache, and Feeds
  reads `all_entries`.
- Every helper the rows need already exists:
  - `mbv_ui_model::home_latest::provider_timestamp_secs`,
    `timestamp_in_launch_window`, and `HomeLatestLaunchWindow`;
  - `ui_util::fmt_publish_date_short`;
  - `QueueItem::playback_title_parts`;
  - `hero_content_queue`;
  - `QueueItemContentId`;
  - the shared `TreeBrowser<Target>` with `TreeNode::trailing`, `TreeMarkPolicy::Aggregate`,
    and `TreeOperation::ToggleExpansionTarget`.

## Goals / Non-Goals

**Goals:**

- Home adopts the shared `TreeBrowser` as a consumer only, under the
  shared-list-components rule "Nesting destinations use one complete shared
  TreeBrowser". It adds no Home-specific tree state.
- One stable, typed identity for every Home row. Illegal targets, such as a
  Continue row with a Feed identity, cannot be represented.
- One marker source for every destination.

**Non-Goals:**

- No server-side date filter. Each source keeps its existing query and limit,
  and Home filters by date on the client (see D3).
- Home does not share data with the destination Latest lists. Home's fetch
  never fills a destination list, and a destination fetch never fills Home.
- No change to Continue Watching's source, order, or Emby-only scope.
- No persisted collapse state.

## Decisions

### D1. Typed Home tree target replaces `HomeRowTarget`

In `crates/mbv-ui-model/src/targets.rs`:

```rust
pub enum HomeTreeTarget {
    Root(HomeRoot),          // Continue | RecentlyAdded
    Age(RecentAge),          // New | Last7Days | Last14Days
    Item(HomeItemTarget),
}
pub enum HomeItemTarget {
    Continue(mbv_ids::ItemId),               // Continue Watching is Emby-only
    Recent(mbv_queue::QueueItemContentId),   // any Service
}
```

`mbv-ui-model` gains a path dependency on `mbv-ids` for `ItemId`.
`HomeRowTarget` is deleted. `ShellRequest::HomePlay`, `HomeEnqueue`,
`HomeToggleWatched`, `HomeDelete`, `HomeRowActivate`, `HomeRowClick`, and
`ContextMenuTargets::Home` carry `HomeItemTarget`. The `Root` and `Age`
targets never cross the component boundary. The component resolves them
locally into expansion changes or into the item targets they contain. The
section is part of the identity, so one item under both roots gives two
distinct tree targets.

Alternative: keep a `String` target with a section prefix. Rejected because it
is stringly typed and leaves the bool-as-section problem in place.

### D2. Shell owns per-source Recently added items. The component owns bucketing.

`Model` gains `home_recently_added: HashMap<DestinationLatestSource, Vec<QueueItem>>`.
Each entry holds that source's items, already filtered to the 14-day cutoff.
`push_home_content` sends one plain projection to the component:

- the Continue items;
- the merged Recently added rows (`HomeRecentRow { item, feed_name }`, newest
  first);
- the `HomeLatestLaunchWindow`.

`feed_name` is resolved by the shell from the subscriptions. Branch assignment
is a pure function in `mbv_ui_model::home_latest`:
`recent_age(timestamp, window) -> Option<RecentAge>`. It returns `None` for an
item older than `window.current - 14d` and is shared by the cutoff filter and
by the projection. Feeds has no entry in the map. Its rows are derived at push
time from `feed_tab.all_entries`, so a feed refresh re-pushes Home.

### D3. Home fetch reuses each destination's query

The new `App::spawn_home_recently_added(service)` runs one background thread
per Service and sends `ModelContentEvent::HomeRecentlyAddedFetched { source, generation, items }`
once per library:

- **Emby:** for each eligible library, TV uses `get_latest_episodes(id, 30)` and
  every other eligible library uses `get_latest(id, 30)`. Eligibility is the
  same predicate as the Latest pill: not `music` and not `playlists`. Extract
  that predicate from `spawn_destination_latest_snapshot` into one function
  that both sites call.
- **Audiobookshelf:** for each podcast library, call `shelves_bounded`, then
  `App::newest_episodes_items`, then
  `QueueItem::Audiobookshelf(Episode(from_catalog(..)))`.

Triggers:

- Emby setup completion;
- Audiobookshelf setup completion, where it replaces the startup shelf call in
  `start_audiobookshelf_catalog_fetches`;
- Home refresh (the `TabSelection::Home` arm of the refresh dispatch).

Stale generations are dropped, matching how `ShelfFetched` is handled today. A
failed source logs, sends no event, and leaves the other sources unaffected.

The design uses the same queries, not a new cross-library query, so every Home
row is also on that destination's Latest list (spec: "Each source SHALL use
that destination's own Latest rules"). It needs no new endpoint and no live
probe. Alternative: a single Emby `/Items?SortBy=DateCreated` query. Rejected
because its rows would differ from the TV and grouped Latest rules.

### D4. Audiobookshelf destination Latest becomes lazy

The destination fetch `start_audiobookshelf_shelves` (into
`audiobookshelf_shelf_cache`) runs in these cases:

- the podcast Latest pill is selected;
- launch restoration on Latest;
- refresh while Latest is selected.

It no longer runs in the startup catalog loop or in a refresh with another pill
selected. Emby is already lazy, and Feeds needs no fetch.

### D5. Markers read Home's data

`destination_latest_marker(source)` becomes the same test for every source: any
item in that source's Home Recently added rows (the map, or the derived Feeds
rows) is in the launch window and the source is not acknowledged.
`DestinationLatestSnapshot::has_new_content`, its recompute, and
`recompute_destination_latest_marker` are deleted.
`tv_latest_snapshots` keeps only the items that feed the Emby destination
Latest projection. Acknowledgement (`acknowledged_home_latest_sources`) is
unchanged.

### D6. Home's input translation over the shared tree

- **Enter** applies `TreeOperation::Activate`:
  - on an `Item`, it emits `HomePlay`;
  - on a `Root` or `Age`, Home applies `ToggleExpansionTarget`.
- **Right** applies `TreeOperation::Right`. An `Activate` intent from Right on
  an expanded `Root` is consumed with no effect, so Right never collapses.
- **Ctrl+Enter, Ctrl+W, Delete, `.`:** each sends its request with the selected
  item target, or the marked item targets in display order. The shared mark set
  already expands an aggregate root or branch to its items.
- **Roots and branches:** declared with `expandable(true)` and
  `TreeMarkPolicy::Aggregate`. Items use `Direct`.
- **Initial expansion:** every root and branch is expanded on first projection.
  Reconciliation keeps it afterwards.

### D7. Shell resolution of item targets

`home_stable_target(&HomeItemTarget) -> Option<QueueItem>`:

- `Continue(id)` looks up `home_content.continue_items`;
- `Recent(content_id)` searches the Recently added rows by `QueueItem::content_id()`.

Play and enqueue use the existing `home_play_target` and `home_enqueue_target`.
The non-Continue arms already call `play_item` or `submit_queue_item`.

Context menu:

- **Single row:**
  - a Continue row keeps today's path;
  - a Recent row opens the destination menu for its Service:
    `ContextMenuTargets::Emby`, `Feeds`, or `Audiobookshelf` (an
    `AudiobookshelfMenuTarget::Episode`).
- **Multi-selection:**
  - all Continue rows keep today's bulk menu;
  - all Emby rows use the Emby multi menu;
  - a selection spanning Services gets Play and Add to Queue only.
- **Ctrl+W:** dispatches the same played-state action that the item's context
  menu offers as Mark Played or Mark Unplayed.
- **Delete:** acts only on `Continue` targets. On a `Recent` target it is a
  documented no-op arm.

### D8. Hero and restoration

- **Hero:** an item row uses `hero_content_queue(item)`. A root or branch row
  uses its first descendant item, so the hero never blanks while you move over
  a branch row.
- **Launch:** `reanchor_launch_state_impl` selects the first `Item` target in
  display order. `launch_snapshot_impl` is unchanged: a fixed Continue scope
  and no item.

### D9. Split title in the shared tree

`TreeNode` gains `secondary: Option<String>` (`with_secondary`). The shared
tree painter, under the `mbv-render` tree_browser component, paints a node with
a secondary part using the same split-row role policy as the flat media list's
`Item` row. Reuse that policy; do not copy it. Search text covers both parts.
Music and TV supply no secondary part and are unaffected.

## Risks / Trade-offs

- [A busy source adds more than 30 items in 14 days, so older in-window items
  are cut] → This is the same 30-item limit as the destination Latest list.
  Home shows a subset of that list, which is consistent.
- [N Emby requests at launch, one per eligible library] → They run on a
  background thread, are bounded by library count, and arrive one by one.
- [Duplicate Audiobookshelf shelf fetch when Home and the restored podcast
  Latest both load at launch] → Accepted. The user chose to keep Home and
  destination data independent.
- [Markers lose items older than 14 days when the previous launch is older]
  → Accepted and recorded as a spec scenario.
- [`home_content.rs` grows past the line cap] → Split along responsibility
  seams before push: projection in `home_content/tree.rs`, input in
  `home_content/keyboard.rs`.
