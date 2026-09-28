# Proposal

## Why

The workspace is pinned to Rust edition 2021, which rejects let-chains
(`if let … && …`). The code carries roughly 225 nested `if`/`if let`
pyramids that exist only because of that (ast-grep count, 2026-09-28:
`src/` 118, `mbv-components` 44, `mbv-daemon` 26, the rest spread thin),
and our own standard `M-LATEST-EDITION` already says crates target the
latest edition. The blocker named in issue #840 (#824) has landed.

The migration is only worth doing if we actually use what it unlocks. On
edition 2024 with `rust-version = 1.88`, clippy's default-on
`collapsible_if` flags nested `if let` blocks as collapsible into
let-chains (verified in a scratch crate: 3/3 flagged on 2024, 0/3 on
2021). Under `-D warnings` that makes let-chain use enforced, not
aspirational: the migration must collapse every site, and new pyramids
cannot come back.

## What Changes

- Workspace `edition` 2021 → 2024 (every crate inherits it via
  `edition.workspace = true`).
- Apply `cargo fix --edition` mechanical rewrites plus hand fixes for
  what it cannot do (RPIT capture, `if let` rescoping, tail-expression
  temporaries, `unsafe extern`, `gen` keyword).
- Collapse every nested-`if` pyramid clippy's `collapsible_if` flags into
  let-chains, so `cargo clippy --workspace --all-targets -- -D warnings`
  is green on 2024.
- Also rewrite the let-chain candidates clippy doesn't flag: 8
  tuple-scrutinee `if let (Some(a), Some(b)) = (x, y)` and 15
  `if let Some(v) = e.filter(|…| cond)` sites whose closure only adds a
  condition.
- Reformat the workspace with rustfmt's 2024 style edition (follows the
  crate edition automatically; there is no `rustfmt.toml`).
- `AGENTS.md`: format line becomes "stock edition-2024".
- `.pi-lens.json`: `rust-2024-let-chain-candidate` stays disabled —
  clippy's `collapsible_if` now enforces the same thing in CI, so the
  pi-lens rule would only duplicate it.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

None. Tooling/language migration with no behavior change; the change
sets `skip_specs: true`.

## Impact

- Every Rust source file (reformat + let-chain collapse); no public API
  or runtime behavior change intended.
- `Cargo.toml` (workspace edition), `AGENTS.md`.
- `rust-version` stays 1.88 (the let-chain minimum on 2024).
- Dependencies unaffected (editions are per-crate).
- Closes #840.
