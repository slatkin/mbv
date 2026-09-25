# Tasks

## 1. Shared marker predicate

- [ ] 1.1 Add `Model::destination_latest_marker(&self, source: &DestinationLatestSource) -> bool` (design D1) in `src/app/shell/home_content.rs`: `Emby(id)` → `tv_latest_snapshots.get(id).is_some_and(|s| s.has_new_content)`; `Audiobookshelf(id)` → any item of `app.audiobookshelf_shelf_cache.get(id)` passes `home_latest::is_new_in_launch_window`; `Feeds` → the existing `feed_tab.all_entries` `pub_date_secs` window check moved verbatim from `shell/feeds.rs`; every arm ANDed with `!acknowledged_home_latest_sources.contains(source)`. Verify: `cargo check -p mbv`.
- [ ] 1.2 Replace the inline `has_new && !acknowledged` / `latest_has_new_content && !latest_acknowledged` computations in `shell/emby_library_content.rs`, `shell/tv_workspace.rs`, `shell/audiobookshelf_podcast.rs` and `shell/feeds.rs` with calls to the predicate. Keep each site's acknowledgement-recording block before the call. Delete locals that become unused. Verify: the existing marker tests (`src/app/tests/tick_integration/tv/latest.rs`, `.../emby_library/latest.rs`, feeds/podcast tick tests) pass unchanged under `cargo nextest run -p mbv latest`.

## 2. Tab bar projection and paint

- [ ] 2.1 Add `markers: &'a [bool]` to `TabBarModel` (`src/app/render/components/chrome_tabs.rs`). In `render_tab_bar`, for a marked tab paint `•` in `palette::ACCENT_ACTIVE` in place of the first trailing padding space (selected: `"▐ {n}•  "` → keep the total width identical to the unmarked form; unselected: `"  {n}• "`). An empty or short slice means unmarked. Do not touch `tab_title_widths`/`visible_tab_range`. Verify: `cargo check -p mbv`.
- [ ] 2.2 Add `markers: Vec<bool>` to `TabPanel` (`src/app/components/tab_panel.rs`), taken by `set_content` and passed into `TabBarModel`. Update `set_content` callers and the existing tab_panel tests' helper. Verify: the existing `tab_panel.rs` tests pass.
- [ ] 2.3 In `Model::sync_tab_panel` (`src/app/shell/chrome_panels.rs`), build `markers` in the same order as the titles (design D2): Home `false`, each `app.libs` entry `destination_latest_marker(Emby(lib.library.id))`, each `app.audiobookshelf_libraries` entry `destination_latest_marker(Audiobookshelf(lib.id))`, Feeds (when subscribed) `destination_latest_marker(Feeds)`. Verify: `cargo check -p mbv`.

## 3. Tests (follow the `writing-tests` skill: hermetic, no sleeps, no geometry assertions)

- [ ] 3.1 In `tab_panel.rs` tests: paint titles `["Home","Movies","TV"]` with markers `[false,true,false]`. Assert that the `•` glyph with the Iris foreground appears within the MOVIES label's painted text and nowhere else on the tab row. Verify: `cargo nextest run -p mbv tab_panel`.
- [ ] 3.2 Tick-integration test beside `src/app/tests/tick_integration/emby_library/latest.rs`: with a Movies snapshot containing an in-window item and another tab active, the mounted `TabPanel` has Movies' marker set (add a `#[cfg(test)] test_markers()` accessor like `test_selected`). After selecting Movies' Latest pill it is cleared, and it stays cleared after a further snapshot update. Verify: `cargo nextest run -p mbv latest`.

## 4. Gate

- [ ] 4.1 `cargo fmt`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo nextest run -p mbv` all pass. Manual check by the user: launch with new Emby items, and the Movies tab shows `•`.
