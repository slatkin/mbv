# Tasks

## 1. Delete the mechanism

- [x] 1.1 Delete `rules/` (three rule dirs, three fixture dirs, snapshots), `sgconfig.yml`, and `.github/workflows/architecture-boundaries.yml`.
- [x] 1.2 Delete the four inline `ast-grep-ignore` comments (`components/queue.rs` ×2, `components/music_workspace.rs`, `components/browser/mod.rs`) and the scan-rule clause in `render/arrangements/wide_hero.rs`'s `wide_hero_split` doc comment, keeping the adjudication prose. Verify the tree still compiles.

## 2. Rewrite the policy that carried it

- [x] 2.1 `openspec/specs/ui-design-system/spec.md`: replace "Common bypasses are mechanically visible" with "Screen modules do not paint" — keep the prohibitions, drop the source-check/CI/narrowing/baseline mandates, and state that the rules are review-owned.
- [x] 2.2 `AGENTS.md`: drop the `architecture: ast-grep scan` tooling line.
- [x] 2.3 `.agents/skills/mbv-frontend/SKILL.md` and `.opencode/skills/mbv-frontend/SKILL.md`: rewrite the three-mechanism section as compiler-only with a named blind-spot list, drop the scan gates from the keyboard-routing and component-boundary prose, and fix the completion checklist.
- [x] 2.4 `docs/architecture/interactive-tui-component-map.md`: rewrite the `Enforcement` section to state the ownership rules and that nothing checks them mechanically.
- [x] 2.5 `docs/architecture/interactive-surface-ledger.md`: record the gate deletion as a dated correction, drop the scan clause from every acceptance cell, and drop the static-check reference from the update rules and the verification record.

## 3. Correct the live plans

- [x] 3.1 Drop the `ast-grep scan` gate from `add-configurable-keybinds`, `add-now-playing-sidebar`, and `unify-wide-hero-content-box-frame` (tasks + D6's framing).
- [x] 3.2 `remove-shared-central-storage`: remove the two gate steps and the proposal bullet that instructed the implementer to edit the now-deleted `rules/interactive-component-boundary/no-service-client-deps.yml`.

## 4. Verify and record

- [x] 4.1 Sweep the tree for the deleted rule ids, rule dir names, `sgconfig`, and `ast-grep scan` as a gate; confirm the only surviving references are in `openspec/changes/archive/**` and the tool skills.
- [x] 4.2 `cargo fmt --all -- --check`, `cargo check -p mbv`, and the comment-only diff review.
- [x] 4.3 `openspec validate remove-ast-grep-ui-enforcement --strict`.
