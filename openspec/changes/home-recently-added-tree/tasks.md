# Tasks

## 1. Continue Watching limit

- [ ] 1.1 Change `get_continue_watching(20)` to `get_continue_watching(10)` in `App::fetch_home` (`src/app/dispatch/library/load.rs`) and in `EmbyClient::load_startup_data_bounded` (`crates/mbv-emby/src/client_auth.rs`). Leave `crates/mbv-core/examples/cast_spike.rs` unchanged. Verify: `cargo check -p mbv -p mbv-emby` passes. No new test: the value is a request parameter, not a contract any test owns.

## 2. Split title in the shared tree (design D9)

- [ ] 2.1 Add `secondary: Option<String>` and `with_secondary` to `TreeNode` (`crates/mbv-components/src/list/tree_browser/types.rs`). Include the secondary part in the node's search text. Fix every `TreeNode { .. }` literal the compiler flags. Verify: `cargo check -p mbv-components`.
- [ ] 2.2 In the shared tree Render Component (`crates/mbv-render/src/components/tree_browser.rs`), paint a node that has a secondary part with the flat media list's split-row role policy. Reuse that policy from `components/media_list/row.rs`; do not copy it. Contract: the shared-list-components "Tree rows can carry a split title" requirement. Verify: one focused buffer test in the tree painter's test module. It asserts the primary uses the playback-context role, the secondary uses the playback-title role, and a played node mutes only the secondary. Existing Music and TV tree tests stay green under `cargo nextest run -p mbv-render -p mbv-components`.

## 3. Home target types (design D1, D2)

- [ ] 3.1 In `crates/mbv-ui-model/src/targets.rs`, add `HomeTreeTarget`, `HomeRoot`, `RecentAge`, and `HomeItemTarget` (`Continue(ItemId)` and `Recent(QueueItemContentId)`), and add the `mbv-ids` dependency to `mbv-ui-model`. Delete `HomeRowTarget`. Replace it with `HomeItemTarget` in `ShellRequest::Home*` (`crates/mbv-ui-msg/src/shell.rs`), `ContextMenuTargets::Home`, `src/app/shell/home.rs`, `src/app/shell/home_content.rs`, and `src/app/dispatch/context_menu/actions.rs`. For now, the Home owner keeps emitting `Continue` targets. Update the existing tests in `src/app/shell/home.rs` and `src/app/tests/context_menu_placement.rs` to build `HomeItemTarget::Continue`. Verify: `cargo check --workspace --all-targets`, and the existing Home tests pass under `cargo nextest run -p mbv`.
- [ ] 3.2 Add `recent_age(timestamp: Option<u64>, window: HomeLatestLaunchWindow) -> Option<RecentAge>` and the shared 14-day cutoff constant to `crates/mbv-ui-model/src/home_latest.rs`. Contract: the spec's branch assignment, plus the cases for a previous launch older than 14 days and for a first launch. Verify: one `#[case]` table in `crates/mbv-ui-model/src/home_latest/tests.rs`, one case per distinct outcome: New, Last 7 days, Last 14 days, too old, no timestamp, first launch.

## 4. Shell Recently added data and fetch (design D2, D3)

- [ ] 4.1 Extract the Latest-eligibility predicate for Emby libraries (not `music` and not `playlists`) into one function. Call it from `spawn_destination_latest_snapshot` and from the new Home fetch. Verify: `cargo check -p mbv`.
- [ ] 4.2 Add `ModelContentEvent::HomeRecentlyAddedFetched { source, generation, items }`, `App::spawn_home_recently_added` for Emby (per eligible library: TV uses `get_latest_episodes(id, 30)`, every other library uses `get_latest(id, 30)`) and for Audiobookshelf (per podcast library: `shelves_bounded`, then `newest_episodes_items`, then queue items), and `Model::home_recently_added`. Filter each source's items to the 14-day cutoff when they are stored, and drop a stale generation. Verify: `cargo check -p mbv`.
- [ ] 4.3 Trigger the Home fetch on Emby setup completion, on Audiobookshelf setup completion, and on Home refresh (the `TabSelection::Home` arm in `load.rs`). Verify: `cargo check -p mbv`.
- [ ] 4.4 Extend `push_home_content` to send the merged, newest-first Recently added rows to the Home owner. Each row is a `HomeRecentRow { item, feed_name }`, built from the per-source map plus Feeds rows derived from `feed_tab.all_entries`, with `feed_name` resolved from subscriptions. Also send the launch window. Re-push Home when feeds refresh. Contract: the spec's interleaving, its 14-day cutoff, and "a failing Service contributes nothing". Verify: one shell test in `src/app/shell/home.rs`. It injects fetched Emby and Audiobookshelf events plus feed entries, including one item older than 14 days. It asserts the projected rows are interleaved newest first and the old item is absent. `cargo nextest run -p mbv` passes.

## 5. Home tree owner (design D6, D8)

- [ ] 5.1 Replace `MediaListCarrier<String>` in `HomeContent` with `TreeBrowser<HomeTreeTarget>`. Project roots, age branches (using `recent_age`), and item nodes. Items use split titles and date gutters through `provider_timestamp_secs` and `fmt_publish_date_short`. Omit empty roots and branches. Roots and branches use `expandable(true)` and `Aggregate`, start expanded, and keep their state through reconciliation. Move projection into `home_content/tree.rs`. Verify: `cargo check -p mbv-components`.
- [ ] 5.2 Port Home's key and pointer translation to the tree, following design D6:
  - Enter toggles a root or branch and plays an item.
  - Right on an expanded root does nothing.
  - Ctrl+Enter, Ctrl+W, Delete, and `.` emit `HomeItemTarget`s, or marked items in display order.

  Move input handling into `home_content/keyboard.rs`. Contract: the spec requirements "Home branches collapse and items act" and "Same item in both roots". Delete the old carrier tests in `home_content.rs`. Write fresh interactive-component tests:
  - Enter on a branch collapses it and keeps it selected;
  - Right on an expanded root keeps it expanded;
  - marking `New` and pressing Ctrl+Enter emits its item targets in order;
  - one item under both roots yields two separately selectable targets.

  Verify: `cargo nextest run -p mbv-components`.
- [ ] 5.3 Hero: an item row uses `hero_content_queue`, and a root or branch row uses its first descendant item. Restoration: `reanchor_launch_state_impl` selects the first item target. `reset_presentation` re-expands every branch and selects the first item. Rewrite the `home_content/launch_state.rs` tests against the tree: a legacy Latest selector lands on the first Continue item. Verify: `cargo nextest run -p mbv-components`.

## 6. Recently added row actions (design D7)

- [ ] 6.1 Implement `home_stable_target(&HomeItemTarget)`: `Continue` looks up `continue_items`, and `Recent` searches the Recently added rows by `content_id()`. Wire play and enqueue through `home_play_target` and `home_enqueue_target`. Delete is an explicit no-op arm for `Recent`. Ctrl+W dispatches the same played-state action the item's context menu offers. Verify: `cargo check -p mbv`.
- [ ] 6.2 Context menu:
  - a single `Recent` row opens its Service's destination menu (`ContextMenuTargets::Emby`, `Feeds`, or `Audiobookshelf`);
  - an all-Continue selection keeps today's menu;
  - an all-Emby selection uses the Emby multi menu;
  - a selection spanning Services offers Play and Add to Queue only.

  Contract: the spec scenarios "Podcast row context menu" and "Mixed-Service multi-selection". Verify: two shell tests in `src/app/shell/home.rs` for those scenarios. `cargo nextest run -p mbv` passes.

## 7. Lazy Audiobookshelf Latest and markers from Home data (design D4, D5)

- [ ] 7.1 Remove the destination `start_audiobookshelf_shelves` call from `start_audiobookshelf_catalog_fetches`, where Home's fetch from 4.3 replaces it. In `audiobookshelf_refresh`, call it only when the podcast Latest pill is selected. Also call it when the podcast Latest pill is selected and on launch restoration on Latest. Verify: `cargo check -p mbv`.
- [ ] 7.2 Change `destination_latest_marker` to test the source's Home Recently added rows against the launch window and the acknowledgement set. Delete `DestinationLatestSnapshot::has_new_content`, its recompute, and `recompute_destination_latest_marker`. Contract: the spec scenario "Unvisited Emby library marks at launch". Verify: update `src/app/tests/home_latest.rs` so each marker test feeds Home Recently added events instead of destination snapshots. Add one test showing that an Emby library which was never opened has its tab marked after only the Home fetch. `cargo nextest run -p mbv` passes.

## 8. Docs and integration

- [ ] 8.1 Update `CONTEXT.md`. "Home view" becomes the Continue watching plus Recently added tree. Add a `Recently added` term (its 14-day age branches; avoid "Home Latest"). In "Destination Latest", remove "without Home duplicates" and note that Latest loads lazily. Verify: `qmd query "Recently added"` returns the new term.
- [ ] 8.2 Run the gates: `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo nextest run --workspace`. All pass.
- [ ] 8.3 Just before push, run `make check-code-file-lines`. Split any governed file over 800 lines along responsibility seams. The check passes.

## Workflow follow-up

- Sync the delta specs into `openspec/specs/` and archive the change after review.
