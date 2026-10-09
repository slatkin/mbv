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
- [x] 4.2 Exactly two new tests own the restart scenarios, plus one fixture update: (a) one app-level test in `src/app/tests/lifecycle/lifecycle_launch_restore.rs` owning *Restart selects Latest* (`destination-latest-modes`: exit with a letter pill active → snapshot records `Latest` → restore opens `Latest` with the first row selected); (b) one TV test in `crates/mbv-components/src/tv_content/tests/launch_state_tests.rs` owning *Restart resolves the count-dependent default* (`tv-library-content-modes`: a saved `Upcoming`/`All`/range mode resolves to the count default, not the previous mode); (c) the `src/app/shell/run/tests.rs:504` fixture `music_launch_state_aaliyah` keeps its saved `item: Some(Emby { id: "Aaliyah" })` as legacy-decode input only — the saved item SHALL NOT restore (`tui-launch-state/spec.md:82`). (`home_latest.rs` needs no update; `tick_integration.rs` is covered by 5.1, not here.)
- [x] 4.3 Run `cargo nextest run -p mbv-components -p mbv`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo fmt`.

## 5. Accompanying fixes shipped in this PR (commits `7d239e821`, `ca8fdab33`, `4d2f4d38b`, `ba48238c8`)

- [x] 5.1 F4 toggles the Playlists sidebar (F2/F3 parity): `src/app/dispatch/action.rs:270` routes `Command::OpenPlaylists` through `request_sidebar_toggle`; `src/app/shell/messages/navigation.rs` routes `ShellRequest::OpenPlaylists` through `toggle_sidebar`; playlist load spawn moves into `mount_sidebar` (`src/app/shell/overlays/sidebars.rs`); `open_playlists_panel` and its hand-rolled dismissals are dropped (`src/app/dispatch/library/load.rs`, `src/app/shell/settings.rs`). Owns `f4_playlists_sidebar_toggles_open_and_closed` in `src/app/tests/tick_integration.rs`. Verify with `cargo check -p mbv`.
- [x] 5.2 Selected icon-only Home tab (2026-10-09 user rule): `crates/mbv-ui-model/src/ui_util.rs:16` (`is_home_icon_title`) with `crates/mbv-render/src/components/chrome_tabs.rs:186` — no eighth-block runs, Iris active colour. Owns the `chrome_tabs/tests.rs` and `ui_util/tests.rs` updates in commits `ca8fdab33` + `ba48238c8`. Verify with `cargo check -p mbv-render -p mbv-ui-model`.
- [x] 5.3 Shuffle-sourced queue paints no pill (2026-10-09 user rule; commit `4d2f4d38b`): `src/app/state/projection/chrome_status.rs:466` returns `None` for `QueueSource::Shuffle`; other source labels unchanged. No dedicated test (single-branch removal). Verify with `cargo check -p mbv`.

## Workflow follow-up

- Manual check by the user: exit mbv on a letter-pill movies library, a podcast show pill, and a TV range; relaunch lands on Latest with the top row selected; Music group and book bucket still restore; a second tab switch within one session keeps the pill.
- Sync the four deltas into `openspec/specs/` and archive the change.
