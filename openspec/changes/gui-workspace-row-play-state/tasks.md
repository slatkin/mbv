# Tasks

Each unit runs `cargo fmt` and commits only its own files (`git commit -- <paths>`).
A unit may leave dependent callers broken for the next unit if it records that
in its handoff. Tests follow `docs/invariants/14-test-ownership.md` and design "Test ownership".

## 1. Shared rule and table

- [ ] 1.1 `list_row.rs`: add `played_title_tone` and make `RowLevel::title_tone` call it for `Some(state)`. Verify: `cargo nextest run -p mbv-gui list_row` passes.
- [ ] 1.2 `library_workspace_table.rs`: change `WorkspaceRow.state` to `Option<GuiHomeRowStateFrame>`, delete `WorkspaceRowState`, `WorkspaceRow.progress`, and `name_color`; derive the title tone (D3), the state hook (D4), and the progress run (D2) from the state. Verify: `cargo check -p mbv-gui` shows errors only in the callers of 1.3.
- [ ] 1.3 `library_workspace_table/tests.rs`: update the two row fixtures and add the one `#[case]` table of design "Test ownership". Verify: `cargo nextest run -p mbv-gui library_workspace_table` passes.

## 2. Callers

- [ ] 2.1 `library_shows_hero.rs` and `library_feeds_hero.rs`: delete `row_state`, pass `Some(state.clone())`, drop the `progress` argument and unused imports. Verify: `cargo nextest run -p mbv-gui library_shows_hero library_feeds_hero` passes.
- [ ] 2.2 `library_audiobooks_hero.rs` and `library_tracklist.rs`: drop `progress: None`. Verify: `cargo nextest run -p mbv-gui library_audiobooks_hero library_tracklist` passes.
- [ ] 2.3 Confirm no `WorkspaceRowState` or per-Hero `row_state` remains. Verify: `rg "WorkspaceRowState|fn row_state" crates/mbv-gui` returns nothing.

## 3. Gates

- [ ] 3.1 Verify `cargo clippy -p mbv-gui --all-targets -- -D warnings` and `cargo fmt --all -- --check` pass, and docs that name `WorkspaceRowState` (if any) are updated.
