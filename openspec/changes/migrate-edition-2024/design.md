# Design

## Context

See proposal.md - Why. Relevant current state:

- One edition pin: `[workspace.package] edition = "2021"` in the root
  `Cargo.toml`; all 26 crates and the root package inherit it.
- No `rustfmt.toml`, so rustfmt's style edition follows the crate
  edition — the bump reformats by itself.
- `rust-version = 1.88`; local toolchain 1.98.
- Edition-compat survey (`RUSTFLAGS=-W rust-2024-compatibility cargo
  check`, isolated target dir, 2026-09-28) on the crates it reached:
  - `tail_expr_drop_order`: 7 sites — `mbv-net/src/bounded.rs` (2),
    `mbv-audiobookshelf/src/socket.rs` (1), `mbv-ws/src/lib.rs` (2),
    `mbv-images/src/resize.rs` (2).
  - `deprecated_safe_2024`: 5 `std::env::set_var`/`remove_var` calls in
    `mbv-config/src/test_support.rs`.
  - The survey stopped at `mbv-images` (compile error unrelated to the
    edition), so crates above it (`mbv-render`, `mbv-components`, the
    TUI, daemon, player …) are unsurveyed. `cargo fix --edition` in task 1
    produces the full list.
- 22 `if let … .lock()…` scrutinees (e.g. `mbv-player/src/controller.rs`,
  `mbv-remote-player/src/connect.rs`) are subject to `if_let_rescope`.

## Goals / Non-Goals

**Goals:**
- Workspace on edition 2024 with fmt, clippy `-D warnings`, and the full
  test suite green.
- Every `collapsible_if` site collapsed to a let-chain.

**Non-Goals:**
- Raising `rust-version` or adopting non-edition features (async
  closures, etc.).
- Touching the pinned upstream rule text in `docs/standards/rules/`
  (its ```` ```rust,edition2021 ```` doc fences are upstream content).
- Any other refactor "while we're in there".

## Decisions

**Bump the whole workspace at once, not crate by crate.** The pin is one
workspace key and every crate inherits it; a staggered bump means
overriding `edition` per crate and running two style editions in CI for
the duration. Alternative (per-crate) rejected: more config churn, no
isolation gain since there's no behavior change.

**Let clippy define the let-chain scope.** "Collapse every
`collapsible_if` warning" is the exit criterion, not the ast-grep count
of ~225. `cargo clippy --fix` does the bulk; leftovers are hand-collapsed.
Alternative (a curated subset of pyramids) rejected: `-D warnings` makes
anything short of all of them a red CI. On top of that, the two
workaround idioms clippy can't see are rewritten by hand: tuple
scrutinees `(Some(a), Some(b)) = (x, y)` (8) and `.filter(|_| cond)`
folded into an `if let` scrutinee (15). Nested `if let { if c {…} else
{…} }` (~90) is excluded: the inner `else` cannot move onto a chain
without changing which branch runs when the outer pattern fails.

**Take rustfmt 2024 style in the same change.** One reformat commit, one
blame break. Alternative (pin `style_edition = "2021"` in a new
`rustfmt.toml`) rejected by the user.

**Commit layers separately inside the change:** (1) edition bump +
`cargo fix --edition` + hand fixes, (2) `cargo fmt` reformat,
(3) let-chain collapse. Reviewers can read (1) and (3) without the
reformat noise, and the reformat commit can go in
`.git-blame-ignore-revs` if one is ever added.

**Accept 2024 drop order for `tail_expr_drop_order` / `if_let_rescope`
sites; don't restore 2021 order by default.** Dropping temporaries
(mostly lock guards and I/O handles) earlier is the safer direction.
Each flagged site is read once; only a site where the earlier drop is
observably wrong gets an explicit binding to keep the old order.

**Test-only `set_var`/`remove_var` become `unsafe` blocks with a
`// SAFETY:` note** stating the precondition that makes each call sound
(single-threaded use, as the helper's existing callers guarantee). No
lint suppression.

## Risks / Trade-offs

- [`if_let_rescope` changes when a lock guard drops before an `else`] →
  In 2024 the guard drops earlier, which can only remove a deadlock, not
  add one; each flagged site is still read in task 1.
- [`clippy --fix` produces a let-chain rustfmt formats badly or that
  reads worse than the pyramid] → accept rustfmt output as-is (AGENTS.md:
  never revert reflow); readability judgment is out of scope.
- [Huge diff collides with concurrent branches] → the reformat is
  mechanical; anyone rebasing reruns `cargo fmt` after resolving.
- [RPIT lifetime-capture change breaks a `-> impl Trait` signature] →
  only 5 such returns; fix with `use<..>` at the compile error.

## Migration Plan

Single PR; the commits above are the sequence. Rollback = revert the
PR (no data, config-format, or protocol change).
