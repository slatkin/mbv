# Proposal

## Why

The Home tree (change `home-recently-added-tree`) reads wrong on screen. Home has two roots and three age branches, with collapse and indentation. No other list in mbv looks like this. Continue watching is a plain list and does not need a tree. The Feeds list already shows the same idea in a simpler way: a flat list with group headers `New` and `Recent`. Home must look and behave like those lists.

## What Changes

- Home becomes one flat list. It uses the shared flat media list, with the same group headers as the TV and Feeds lists. It no longer uses the tree list.
- `Continue watching` is a group header over the flat list of Continue items.
- `Recently added` is a group header. Under it, the age groups `New` and `Recent` are group headers of the same style. There are no other age groups.
- Age logic matches the Feeds list (`feed_age_group`, measured from now): `New` is 0–1 days old, `Recent` is 2–13 days old. An older item is not shown.
- **BREAKING (spec)**: remove roots, branches, expand and collapse, the `Last 7 days` and `Last 14 days` branches, and the "dated after the previous launch" rule for `New`. `New` no longer depends on the previous launch.
- Remove the tree types and code that only Home used: `HomeTreeTarget`, `HomeRoot`, `RecentAge`, `recent_age`, and `TreeNode` secondary titles (`with_secondary` and its painter branch). The flat list already paints split titles.
- Keep unchanged: the shell fetch and merge of Recently added rows, row actions and context menus per Service, the lazy Audiobookshelf Latest fetch, and the tab markers (they keep the launch window).
- Fix the standards findings from the PR 930 review that stay valid after this change (wildcard arm in `handle_home_request`, inline test module in `load.rs`, citations to the archived change).

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `home-latest-sections`: Home is a flat list with group headers. Age groups are `New` and `Recent`, taken from the Feeds rule. Remove the collapse requirement. Update the wording of the requirements that name roots and branches.
- `shared-list-components`: remove the requirement "Tree rows can carry a split title". No tree uses it any more.

## Impact

- `crates/mbv-components/src/home_content.rs` and its children: `tree.rs` (replaced by a flat projection), `keyboard.rs`, `launch_state.rs`, `tests.rs`.
- `crates/mbv-ui-model`: `targets.rs` (remove `HomeTreeTarget` and `HomeRoot`), `home_latest.rs` (remove `RecentAge` and `recent_age`; keep the launch window and the 14-day cutoff for storage and markers).
- `crates/mbv-components/src/list/tree_browser/types.rs` and `crates/mbv-render/src/components/tree_browser.rs`: remove the secondary title.
- `src/app/shell/home.rs` and `home_content.rs`: the push sends the current time for grouping.
- `CONTEXT.md`: update "Home view" and "Recently added".
- No change to Service requests, the queue, or persistence.
