# Tasks

## 1. The ensure contract

- [x] 1.1 Add `src/app/gui_library/ensure.rs`: the `GuiDestinationLoads` trait (`ensure_root`, `ensure_selection`, `ensure_dependents`) and `App::ensure_gui_destination(GuiDestinationKind)` with an exhaustive match and no wildcard. Home and Feeds take no-op steps. Verify with `cargo check -p mbv`.
- [x] 1.2 Add one unit test in `src/app/tests/` that a driver run calls root, then selection, then dependents, using a recording site. Verify it passes with `cargo nextest run -p mbv`.

## 2. Move the destinations behind the trait

- [ ] 2.1 Music: move `ensure_gui_music_selection`'s body into the selection step (root and dependents no-op). Port its tests in `src/app/tests/gui_library.rs` to `ensure_gui_destination`. Verify they pass.
- [ ] 2.2 Shows: split `ensure_gui_shows_workspace` into root, selection, and dependents steps, keeping the return-after-a-started-load order. `gui_tv_open` keeps the All-mode force and calls the driver. Port `gui_library_tv.rs` and `gui_library_shows.rs` tests. Verify they pass.
- [ ] 2.3 Emby library: move the plain-library root fetch to the root step and the feed-view fetch to the selection step. `gui_emby_open` keeps the failed-mark clear and calls the driver. Port `gui_library_emby.rs` and `gui_library_emby/feed_fetch.rs` tests. Verify they pass.
- [ ] 2.4 Audiobooks: move the `gui_abs_open` catalog refresh and its `unloaded` guard into the root step, and the effective pick's detail load into the selection step. Add a test that a restored Audiobooks destination starts the catalog load from the driver, and one that an in-flight `loading_pages` or `replacement` starts no second chain. Port `gui_library_audiobooks.rs`. Verify they pass.
- [ ] 2.5 Podcasts: move the `gui_podcast_open` show-list refresh and guard into the root step, the scope commit into the selection step, and All's shelf and the fan-out into the dependents step. Add the same two tests as 2.4. Check how a failed catalog load leaves the browse state; if it leaves it unloaded, add a failed mark so a repaint does not retry (design, Risks). Verify tests pass.

## 3. Callers

- [ ] 3.1 In `crates/mbv-gui/src/sections/library.rs`, replace the per-destination ensure calls with one driver call before the parts are built. Verify the `mbv-gui` core-thread library tests pass with `cargo nextest run -p mbv-gui`.
- [ ] 3.2 Make `open_gui_active_destination` run the user-action work, then the driver, with an exhaustive match. Make the five per-destination ensure functions non-`pub`. Verify `cargo check --workspace --all-targets` and `cargo clippy --workspace --all-targets -- -D warnings` pass.
- [ ] 3.3 Document the root, selection, dependents order in `docs/architecture/gui-client.md` and in `docs/recipes.md` step 7 (browse destination). Verify the text names the trait and the driver.

## 4. Close out

- [ ] 4.1 Run `cargo fmt`. Commit the change. Verify `git status` shows a clean worktree.

## Workflow follow-up

- Sync the delta into `openspec/specs/gui-client/spec.md` and archive the change when the PR is approved for merge.
