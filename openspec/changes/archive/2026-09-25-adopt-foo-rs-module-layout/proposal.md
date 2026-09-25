# Proposal

## Why

AGENTS.md now forbids `mod.rs`. A module with children is `foo.rs` plus a `foo/` directory, with tests as `foo/tests.rs`, or `foo/tests.rs` + `foo/tests/` when split. The tree still has 123 `foo/mod.rs` files across `src/` and `crates/`. Decision 3 of the archived `tidy-repo-layout` (#772) chose `mod.rs` only because it meant fewer moves at the time. The later modularize changes made it the majority by accident, not on purpose. Nothing enforces either style, so the tree drifts. Closes #793.

## What Changes

- Rename every `foo/mod.rs` to `foo.rs`. These are pure file moves. `mod foo;` resolves to either layout, so no `mod` declarations change.
- Add `mod_module_files = "deny"` under a new `[workspace.lints.clippy]`, plus `[lints] workspace = true` in `mbv`, `mbv-core` and `mbvd`, so a new `mod.rs` fails clippy.
- Rewrite current-state `…/mod.rs` path references to the new paths. This covers AGENTS.md, skills, agent definitions, ADRs, invariants, palette docs, source comments and the unstarted `encode-local-queue-owner` change.
- This supersedes `tidy-repo-layout` decision 3. That archived change stays as written.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

None. This is a layout and lint change with no behaviour change (`skip_specs: true`).

## Impact

- Touches almost every directory under `src/` and `crates/*/src/`, so it conflicts with any in-flight code work. Land it on a clean tree, in one commit, when no other change is mid-apply.
- It adds the first `[lints]` tables to the workspace.
- It makes no runtime, API or dependency change.
