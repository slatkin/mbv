# Tasks

Test layer for every group below: app lifecycle tests
(`src/app/tests/lifecycle/`), where the launch-restore contract already lives.
No new test asserts geometry or drives a live service. Regression tests cite
umbrella #810 (invariant 11) in a comment.

## 1. Delete legacy launch migration

- [ ] 1.1 Delete `legacy_launch_tab`, `legacy_launch_migration_attempted`, `library_tab_pending` (`app_struct.rs`, `construct.rs`), the numeric fallback branch and `migrate_legacy_launch_state` / `legacy_tab_identity` / `legacy_position_for_key` / `legacy_identities` (`cw_library_tab.rs`), and the `apply_tab_position` resets of the deleted fields. Verify: `cargo check -p mbv` has no unused-import or dead-code warnings from this change.
- [ ] 1.2 Delete `lifecycle_launch_migration.rs` tests that only exercise migration (`legacy_launch_migration_uses_stable_item_and_ignores_cursor_index`, `malformed_and_nonnumeric_legacy_preferences_fall_back_cleanly_through_construct`, `legacy_audiobookshelf_podcast_item_is_not_migrated`); rewrite `versioned_launch_state_takes_precedence_over_legacy_migration` only if its remaining claim (a stored snapshot is used as-is) is not owned by another test, else delete it. Delete `tick_integration.rs:84` `library_tab_pending` test and the `panel_focus.rs:102` assert. Verify: `cargo nextest run -p mbv` passes and no deleted test's claim is left unowned (state each surviving owner in the commit message).

## 2. `LaunchRestore` state machine and `select_tab`

- [ ] 2.1 Add `select_tab(tab)` (assign + `settle_tab_selection`) and route `apply_tab_position` through it. Verify: `restored_launch_tab_loads_its_library_content_not_just_the_tab` and the existing explicit-tab-movement test still pass.
- [ ] 2.2 Replace `pending_launch_state` + `pending_launch_tab_resolved` with `enum LaunchRestore { Pending, TabSettled { state, tab }, Done }` on `App` (`app_struct.rs`, `construct.rs`); `Pending → TabSettled` only inside `resolve_library_tab_pending` via `select_tab`, recording `self.tab` after settling. Update `reanchor_pending_launch_destination` (`shell/library_panel.rs`) to match only `TabSettled`, apply only while `self.tab == tab`, and move to `Done` on accept or on mismatch. Update all tests that assign the old fields. Verify: contract "the re-anchor applies only to the tab it was resolved for" has one new test (tab changed via `normalize_stale_browse_destination` before re-anchor leaves the new tab untouched); `cargo nextest run -p mbv` passes.

## 3. Resolve on catalog arrival; delete the readiness bools

- [ ] 3.1 Add `resolve_launch_service_tab(kind)` and call it at the end of `rebuild_library_tabs_from_views` (`load.rs`) and `apply_audiobookshelf_catalog` (`drains.rs`). Reduce `resolve_library_tab_pending` (sync pass, `shell/run.rs:24`) to Home/Feeds only. Verify: the existing "pending launch tab resolves after catalog arrival" test, rewritten to drive catalog arrival instead of setting a marker, passes.
- [ ] 3.2 Delete `emby_catalog_ready`, `audiobookshelf_catalog_ready`, `resolve_service_tab`'s marker checks, and the sets at `emby_service_completion.rs:73,246`, `load.rs:365`, `drains.rs:121`; update `shell/run/tests.rs:314,442` and lifecycle tests. Verify: `rg "catalog_ready" src` is empty and the daemon-attach path (catalog via `fetch_home`) has a test restoring a Service tab with no Emby startup worker (regression for invariant 11 property 1).

## 4. Expiry by Service outcome

- [ ] 4.1 A snapshot naming an unconfigured Service starts `Done` at build (`construct.rs`). Verify: test "saved Service tab of an unconfigured Service stays on Home and a later configure does not move the tab" (spec scenarios *not configured* and *configured after launch*).
- [ ] 4.2 Add `expire_launch_service(kind)` and call it at every path where a Service's startup outcome becomes a failure: Emby startup Err and setup Err (`emby_service_completion.rs`, incl. `handle_emby_runtime_failure`), `fetch_home` failure on daemon attach, and Audiobookshelf catalog `Err` including the non-auth arm (`drains.rs`). Verify: one test per Service (Emby, Audiobookshelf) that a failed startup followed by a successful connection leaves the tab unchanged (spec scenario *fails at startup*); list the paths covered in the commit message.

## 5. Docs and integration

- [ ] 5.1 Rewrite `docs/invariants/11-launch-state-catalog-boundary.md` (rename if the title no longer fits) to the residual only: `App::tab` has several production writers and no type stops a new one skipping settle. State that properties 1 and 2 are enforced by `LaunchRestore` and by resolving at catalog arrival. Verify: doc no longer claims the marker rule; links from `docs/invariants/` index (if any) resolve.
- [ ] 5.2 Integration: `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo nextest run` for the workspace, and `make check-code-file-lines` just before pushing. Verify: all green; record any edited file over 800 lines and split it before the PR.
