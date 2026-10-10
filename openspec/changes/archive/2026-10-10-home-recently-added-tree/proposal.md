# Proposal

## Why

Home shows only Emby Continue Watching. To find new content you have to visit
each library's Latest pill one by one. Also, Latest loading is inconsistent
across Services. Emby fetches a library's Latest only when you open or select
it. Audiobookshelf podcasts fetch their Newest Episodes shelf for every library
at startup. As a result, the tab markers for Emby libraries you have not opened
never light. Home should show one short, time-bounded view of what is new
everywhere. Every destination's full Latest list should load the same lazy way.

## What Changes

- Home becomes a full tree list in the shared `TreeBrowser`, replacing its flat
  canonical media list. It has two collapsible roots: **Continue watching** and
  **Recently added**.
- Continue Watching is trimmed from 20 to 10 items.
- **Recently added** combines the Latest content of every destination that
  offers a Latest pill: Emby libraries other than Music, Audiobookshelf podcast
  libraries, and Feeds. Items are merged newest first. Each row shows its
  added or published date in the right gutter.
- Recently added holds only items from the last 14 days. It groups them into
  three non-overlapping branches: **New** (since the previous launch),
  **Last 7 days**, and **Last 14 days**. Empty branches are not shown. Older
  items never appear.
- Home loads its Recently added data from every eligible source at startup and
  on Home refresh, independent of the destination Latest lists.
- Destination Latest lists all load lazily. Audiobookshelf podcast Latest stops
  fetching its shelf for every library at startup. It fetches when the
  destination's Latest is selected, like Emby.
- Destination tab and pill new-content markers are now evaluated from Home's
  Recently added data. Every eligible destination can mark at launch without
  loading its full Latest list.
- Home rows are addressed by a section-qualified, Service-qualified stable
  target. The same item can appear under both Continue watching and Recently
  added.
- The shared tree row gains an optional secondary title part. Home keeps its
  established container-plus-title split-row palette.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `home-latest-sections`: Home is now a Continue watching plus Recently added
  tree with time branches, instead of Continue Watching only. Continue Watching
  is limited to 10 items.
- `destination-latest-modes`: Latest lists load lazily at every destination.
  Markers are evaluated from Home's Recently added data, not from
  already-loaded destination lists.
- `shared-list-components`: tree nodes may carry a secondary title part,
  painted with the split-row palette.

## Impact

- `crates/mbv-components/src/home_content.rs` and `home_content/launch_state.rs`:
  the owner moves from `MediaListCarrier<String>` to `TreeBrowser<HomeTreeTarget>`.
- `crates/mbv-ui-model/src/targets.rs`: `HomeRowTarget` is replaced by a typed
  Home tree target. Consumers update in `mbv-ui-msg` (`ShellRequest::Home*`),
  `mbv-ui-model` (`context_menu.rs`), and `src/app/shell/home.rs` and
  `home_content.rs`.
- `crates/mbv-components/src/list/tree_browser/types.rs` and
  `crates/mbv-render/src/components/tree_browser.rs`: optional secondary title.
- `src/app/dispatch/library/load.rs` and `crates/mbv-emby/src/client_auth.rs`:
  the Continue Watching limit drops from 20 to 10.
- New Home Recently added fetch and projection in `src/app/shell/` and
  `src/app/dispatch/`. It reuses `EmbyClient::get_latest` and
  `get_latest_episodes`, `start_audiobookshelf_shelves`, and
  `feed_tab.all_entries`.
- `src/app/dispatch/run_loop/drains.rs`: the startup podcast shelf fetch becomes
  Home's fetch. The destination fetch moves to Latest selection.
- `src/app/shell/home_content.rs`: `destination_latest_marker` reads Home's
  Recently added data. `tv_latest_snapshots` stops owning marker state.
- No new dependencies, protocol changes, or persisted-state changes.
