## 1. Reassign the panel-mode chord to UiRoot

- [ ] 1.1 In `src/app/key_policy.rs`, change the `panel_mode_cycle_x` entry's
      `owner` from `KeyPolicyOwner::Sub(ComponentId::Library)` to
      `KeyPolicyOwner::Sub(ComponentId::UiRoot)` (around line 267). Do not
      touch its `binding`, `gate`, or `blocking` fields. Verify:
      `rtk cargo check -p mbv` compiles.
- [ ] 1.2 Add or extend a `key_policy`/`router` test asserting that with
      `RouterSnapshot { text_entry_focused: true, blocking_overlay_open:
      false, .. }` the `x` chord resolves to `RouterOutcome::FallThrough`
      (character reaches the field), and with `text_entry_focused: false` it
      still resolves to `RouterOutcome::Command(Command::CyclePanelMode)`.
      The existing router tests live in `src/app/key_policy.rs`'s `#[cfg(test)]`
      module (see the `queue_only` / `PanelMode` fixtures around line 449) and
      `src/app/router.rs`. Verify: `rtk cargo nextest run -p mbv key_policy`
      and `rtk cargo nextest run -p mbv router` pass.

## 2. Remove the phantom ComponentId variant

- [ ] 2.1 Delete the `Library,` variant from `enum ComponentId` in
      `src/app/components/component_id.rs` (line 20). Verify:
      `rtk cargo check -p mbv` — let the compiler report every non-exhaustive
      `match` or stale reference; there should be none outside the
      `key_policy.rs` line already fixed in task 1.1. If the compiler flags a
      site not mentioned in this plan, stop and record it (it means the survey
      missed a live use of the phantom).
- [ ] 2.2 Fix the stale doc comment in `src/app/shell_library.rs:12-14`: it
      still describes `sync_library_parent` mirroring routing state "into
      `LibraryComponent` each tick and read it back". Reword to state that the
      child is derived directly from `App.tab` via `library_child_id` with no
      component and no mirror. Comment only — do not change
      `sync_active_destination`'s behavior. Verify: `rtk cargo check -p mbv`.
- [ ] 2.3 Run `rtk cargo clippy --workspace --all-targets` and
      `rtk ast-grep scan`; both stay green with no new diagnostics beyond the
      repository's pre-existing screen-boundary set.

## 3. Reconcile the ledger

- [ ] 3.1 Rewrite row 64 of `docs/architecture/interactive-surface-ledger.md`
      ("Root | Library parent | migrated (2026-08-27) | ..."). The "Primary
      current ownership" cell must state: no component — Library-parent
      routing is pure derivation over `App.tab` in
      `src/app/shell_library.rs` (`sync_active_destination` +
      `library_child_id`), which routes TuiRealm focus to the mounted
      destination child (`Home`, `Feeds`, `Browser(..)`, ABS/TV/Music
      workspace) or `UiRoot` when the destination has no mounted surface
      component. Remove every mention of `LibraryComponent` and
      `src/app/components/library.rs`. Keep the `migrated` state and the 2026
      date. Update the "Notes" cell to drop the `library_parent` nextest
      reference (that test name no longer exists) and record that
      `ComponentId::Library` was removed and `panel_mode_cycle_x` reassigned
      to `UiRoot` under `resolve-library-parent-phantom`. Verify: the row
      contains no reference to a non-existent module or test.
- [ ] 3.2 Leave the `| Library | ...` rows (65–72, 89) unchanged — "Library"
      there is the parent-category label for the destination surfaces, not a
      reference to the removed `ComponentId::Library`. Do not relabel them in
      this change. Verify: `git diff docs/architecture/interactive-surface-ledger.md`
      touches only row 64.

## 4. Final verification gate

- [ ] 4.1 `rtk cargo check -p mbv`, `rtk cargo nextest run -p mbv`,
      `rtk cargo clippy --workspace --all-targets`, `rtk ast-grep scan`,
      `rtk cargo fmt --all -- --check` — all green.
- [ ] 4.2 `openspec validate resolve-library-parent-phantom --strict` passes.
- [ ] 4.3 Manual check (or a shell-level test if one already exercises the
      inline search sidebar): with the global search overlay focused, the
      `x` key inserts a literal `x` into the query instead of cycling the
      panel layout.
