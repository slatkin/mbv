# Tasks

## 1. Owner snapshots record the persisted pill scope

- [x] 1.1 `EmbyLibraryContent::launch_snapshot` (`crates/mbv-components/src/emby_library_content/panel.rs`): selector is `EmbySelectorKey::Latest` when `selector_mode` is `Letters` or `FeedGroups`, `None` when `None`; item is `None`. Verify with `cargo check -p mbv-components`.
- [x] 1.2 `TvContent::launch_snapshot` (`crates/mbv-components/src/tv_content/panel_owner.rs`): selector and item are `None` (TV mode/letter pills are session memory). Verify with `cargo check -p mbv-components`.
- [x] 1.3 `PodcastContent::launch_snapshot` (`crates/mbv-components/src/podcast_content/panel.rs`): selector is always `AudiobookshelfSelectorKey::Latest`; item is `None`. Verify with `cargo check -p mbv-components`.
- [x] 1.4 `FeedsContent::launch_snapshot` (`crates/mbv-components/src/feeds_content/panel.rs`): selector is `FeedsSelectorKey::Latest` when subscriptions exist, `None` otherwise; item is `None`. Verify with `cargo check -p mbv-components`.
- [x] 1.5 `MusicContent::launch_snapshot` keeps the group selector and drops the item (delete the artist fallback, which existed only so the saved item could restore). `BookContent::launch_snapshot` keeps the bucket selector and drops the item. `HomeContent::launch_snapshot_impl` keeps the Continue selector and drops the item. Verify with `cargo check -p mbv-components`.

## 2. Restore honors only the persisted scope

- [x] 2.1 `EmbyLibraryContent::launch_selector`: honor only `EmbySelectorKey::Latest` (emit `EmbyLatest` when the owner paints pills and is not already in Latest mode); legacy letter/group/unfiltered selectors apply nothing. Verify with `cargo check -p mbv-components`.
- [x] 2.2 `TvContent::launch_selector` returns `None` unconditionally. Verify with `cargo check -p mbv-components`.
- [x] 2.3 `PodcastContent::launch_selector` emits `AudiobookshelfLatest` only for a `Latest` snapshot; `reanchor_launch_state` sets `Latest` and ignores legacy filter/show keys. Verify with `cargo check -p mbv-components`.
- [x] 2.4 `FeedsContent::reanchor_launch_state`: with subscriptions present it always selects Latest; legacy filter/group keys resolve to Latest. Verify with `cargo check -p mbv-components`.

## 3. Legacy position document cannot resurrect a pill

- [x] 3.1 In `src/app/state/construct.rs`, clear `letter_filter_index`, `tv_content_mode`, and `feed_selected_group` on every level of every entry loaded from the position document. Verify with `cargo check -p mbv`.

## 4. Tests

- [x] 4.1 Update component snapshot tests to the new contracts: feeds (`launch_snapshot_*`), podcast (`tests/projection.rs`), book, music, and the emby/TV owner tests that asserted letter/group/item identities. Each renamed/rewritten test names the spec scenario it owns.
- [x] 4.2 Update app-level tests: `lifecycle_launch_restore.rs`, `home_latest.rs`, `tick_integration.rs`, `shell/run/tests.rs`, and any library-position test that relied on a saved pill restoring across restart. Add one test owning the spec scenario *Restart selects Latest* at the app level (exit with a letter pill active → snapshot records Latest) and one owning *Restart resolves the count-dependent default* for TV if not already covered.
- [x] 4.3 Run `cargo nextest run -p mbv-components -p mbv`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo fmt`.

## Workflow follow-up

- Manual check by the user: exit mbv on a letter-pill movies library, a podcast show pill, and a TV range; relaunch lands on Latest with the top row selected; Music group and book bucket still restore; a second tab switch within one session keeps the pill.
- Sync the four deltas into `openspec/specs/` and archive the change.
