# Proposal

## Why

Issue #811 (downscoped after evaluation): the codebase carries 44 `#[expect(clippy::cast_precision_loss)]` sites, each re-deriving the same facts inline — ticks are 100 ns units, i64→f64 is exact below 2^53 (≈28,500 years of media), and the mpv/cast side is inherently f64. The lint is not a latent bug, but 44 duplicated expect+reason blocks are a real smell: the seconds↔ticks conversion API does not exist, so every site hand-rolls `x as f64 / TICKS_PER_SECOND as f64` and re-justifies it. The original issue's structural fix (narrow tick types) is a dead end — u32 ticks cap at ~7:15 of media, and the mpv boundary is f64 regardless — so the fix is centralization, not type migration.

## What Changes

- Add a conversion API in `mbv-core` next to `TICKS_PER_SECOND` (`crates/mbv-core/src/api/types.rs`):
  - `ticks_to_seconds(ticks: i64) -> f64` — carries the single `cast_precision_loss` expect and the 2^53-exactness domain doc once.
  - `seconds_to_ticks(seconds: f64) -> i64` — promote the existing `saturating_i64_from_f64` (`crates/mbv-core/src/player.rs:84`) into this API and route seconds→ticks sites through it.
- Migrate Family A (ticks→seconds, ~19 sites) and Family B (seconds→ticks, ~12 sites) to the helpers, deleting their per-site expects.
- Migrate the mpris µs→seconds sites (2) to a small local helper with one expect.
- **Eliminate, not allow, the residual lints.** Family D sites (mpv volume curve, chrome seek-fraction render, image f32 scale, cast/mpris volume, reconnect backoff, mouse-gesture scaling) each get a genuine fix: narrow the value through `u32::try_from`/`i32::try_from` (the domain bound, expressed in types) and convert via `f64::from`/`f32::from`, or reuse a helper. No residual `cast_precision_loss` expect survives at a call site.
- End state: exactly 3 `cast_precision_loss` expects remain in the workspace — one inside `ticks_to_seconds`, one inside mpris `us_to_seconds`, and one inside `int_ratio` — each a sanctioned conversion kernel carrying its domain rationale. (A quotient/remainder exact-construction variant that removes even these is documented in design as rejected: it obscures the math to dodge a linter.)
- No behavior change: helpers and per-site fixes preserve the arithmetic each site performs today (including `.round()`/truncation differences — see design).

## Capabilities

### New Capabilities

(none — pure internal refactor; `skip_specs: true` per repo precedent for refactor changes)

### Modified Capabilities

(none — no externally observable behavior changes; existing specs in `openspec/specs/` are untouched)

## Impact

- `crates/mbv-core/src/api/types.rs` (new helpers), `crates/mbv-core/src/player.rs` (`saturating_i64_from_f64` moves/promotes), and ~44 expect sites across `src/` and `crates/` (mpris, mbv-net, dispatch, components, render, player, daemon, playback).
- Net diff expected negative: ~42 expect blocks deleted, 2 added (kernel-internal), plus small per-site type-bound code.
- Zero `cast_precision_loss` findings outside the three sanctioned kernels (ticks, mpris µs, and `int_ratio`); verified by `rg 'clippy::cast_precision_loss'` (target count: 3) and a clean `cargo clippy --workspace --all-targets -- -D warnings`.
- No API/protocol/wire changes; golden wire-format tests unaffected. Full gates apply: `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt`, `cargo nextest run -p mbv -p mbv-core`.
