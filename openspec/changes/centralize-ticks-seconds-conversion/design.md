# Design

## Context

44 `#[expect(clippy::cast_precision_loss)]` sites across `src/` and `crates/` (inventory in proposal). They fall into four families:

- **A — ticks→seconds** (~19 sites): `x as f64 / TICKS_PER_SECOND as f64` (player, daemon ws, ws/player events, api types, reporting, queue, audiobookshelf playback, render hero fraction).
- **B — seconds→ticks** (~12 sites), with three sub-semantics: saturating (via `saturating_i64_from_f64` in `player.rs:84` **and a duplicated private copy** in `daemon/audiobookshelf.rs:10`), plain truncating `as i64` (cast_status, playback_target, mouse_gestures), and rounding/trunc-to-u64 (browse, catalog).
- **C — mpris µs→seconds** (2 sites in `src/mpris.rs`).
- **D — one-off math** (~11 sites): mpv volume cube-root curve (4), chrome seek-fraction render (2), image f32 scale (2), cast volume f32, mbv-net backoff, mpris volume, mouse-gesture integer-second scaling.

The issue's original structural fix (narrow tick types) is a dead end: u32 ticks cap at ~7:15 of media, and mpv exposes `time-pos` as f64 regardless. See proposal.md — Why.

## Goals / Non-Goals

**Goals:**
- One home for the seconds↔ticks conversion API and its 2^53-exactness domain doc.
- Collapse the 44 expects to exactly **2** — the unit-conversion kernels — by genuinely fixing every other site, not by allowing the lint (user direction on #811).
- Delete the duplicated saturating-conversion copy in `daemon/audiobookshelf.rs`.
- Zero behavior change: every migrated site computes the same value it does today.

**Non-Goals:**
- Narrowing tick storage types (u32/newtype) — dead end per the issue's own caveat.
- Touching Family D one-off math or its expects.
- Any wire-format, protocol, or spec-level change (`skip_specs: true`).

## Decisions

1. **Helpers live in `crates/mbv-core/src/api/types.rs`, next to `TICKS_PER_SECOND`.**
   `src/` already imports `mbv_core::api::TICKS_PER_SECOND`, so the dependency direction is already established. Alternatives: a new `ticks.rs` module (more files for two functions) or leaving them in `player.rs` (wrong layer — api types are the shared home of the unit).

2. **`TICKS_PER_SECOND_F64: f64 = 10_000_000.0` const eliminates the lint at call sites.**
   Most Family B expects exist only because of `TICKS_PER_SECOND as f64` in a call-site expression. A pre-converted f64 const removes the i64→f64 cast entirely: `(seconds * TICKS_PER_SECOND_F64)` needs no expect. The const is defined once beside the i64 one (drift risk noted below). Alternative: route every multiply through the helper — but rounding/trunc-to-u64 sites then need helper variants per rounding mode.

3. **Helper set: one cast kernel, two wrappers.**
   - `ticks_to_seconds(ticks: i64) -> f64` — `ticks as f64 / TICKS_PER_SECOND_F64`, carries the single Family-A expect plus the domain doc ("exact for |ticks| < 2^53 ≈ 28,500 years of media").
   - `i64_ticks_saturating(value: f64) -> i64` — the promoted `saturating_i64_from_f64` (NaN→0, clamp at i64 bounds); no expect needed (f64→i64 is a different lint family).
   - `seconds_to_ticks(seconds: f64) -> i64` — `i64_ticks_saturating(seconds * TICKS_PER_SECOND_F64)`; truncating. Drop-in for Family B's saturating **and** plain-truncating sites (saturating differs from bare `as i64` only outside i64 range, where bare `as` is UB-adjacent saturation anyway — same value, defined).
   - Rounding/trunc-to-u64 sites (browse `u64`, catalog `u64`) keep `.round()`/`.trunc()` at the call site and call `i64_ticks_saturating((seconds * TICKS_PER_SECOND_F64).round())` — semantics preserved exactly, no expect needed.

4. **mpris gets a file-local `us_to_seconds(i64) -> f64`** with one expect, not a place in the shared API — microseconds are an mpris/dbus concern, not a media-unit.

5. **Residual Family-D lints are fixed by narrowing through `try_from`, not expected.**
   Clippy's cast lints fire only on `as` expressions, and `f64::from`/`f32::from` exist for u8/u16/u32/i32 — all exact. The recipe `f64::from(u32::try_from(v).unwrap_or(u32::MAX))` expresses the value's domain bound **in types** at the boundary, which is exactly the #810 spirit this issue sits under. Applied per site:
   - mpv volume curve (`runtime.rs:467,472`, `decisions.rs:87`, `commands.rs:139`): values are clamped ≤ volume-max (≈169); `u32::try_from` before `f64::from`.
   - chrome seek-fraction (`chrome_player.rs:180`): route through a shared `int_ratio(numer: i64, denom: i64) -> f64` helper (≥2 users: chrome_player and `hero.rs:417`'s position/duration percent) carrying one expect; `chrome_player.rs:191` `width as f64` becomes `f64::from(area.width)` (u16, exact).
   - image f32 scale (`images.rs:201,207`): `u16::try_from` on base dimensions (no real image exceeds 65,535 px) before `f32::from`; if the u32 source resists, fall back to one reasoned expect decided during implementation.
   - cast volume (`cast.rs:84`): compute the clamped 0..=100 volume as u8 (the attachment already stores u8) and `f32::from` it.
   - mpris volume (`mpris.rs:401`) and mbv-net backoff (`lib.rs:33`): `u32::try_from` route, or narrow the stored field type if its only producer already guarantees the range.
   - mouse-gesture integer-second scale (`mouse_gestures.rs:31`): `u32::try_from(runtime_s)` route.
   Any site that resists both narrowing and helper routing during implementation reports back with its specific domain argument rather than silently re-adding an expect.

6. **The two kernels keep their expects; a zero-expect exact-construction variant was considered and rejected.**
   `ticks_to_seconds` could avoid `as f64` via a quotient/remainder split (`i32::try_from(ticks / TPS)` + `(ticks % TPS)` parts), exact for all |ticks| < 2^53. Rejected: it adds clever arithmetic whose only purpose is dodging a linter, and the expect + reason string is precisely the domain documentation #811 asks for. Same for mpris `us_to_seconds` (2^53 µs ≈ 285 years). Two audited, documented kernels beat one clever formula.

## Risks / Trade-offs

- [Const drift: `TICKS_PER_SECOND` (i64) and `TICKS_PER_SECOND_F64` could diverge] → define `TICKS_PER_SECOND_F64` immediately below the i64 const with a comment binding them; optionally a one-line unit test asserting `TICKS_PER_SECOND_F64 == TICKS_PER_SECOND as f64` (that one cast lives in a test, where the workspace's test-code lint scope keeps it).
- [Saturating helper changes truncation-site behavior at extremes] → only reachable for |seconds| beyond i64 ticks (≈2920 years); documented as strictly-safer, not behavior-changing for any real input.
- [Missed sites when migrating] → after each family migration, `rg 'clippy::cast_precision_loss'` count is the acceptance check; final target count: 2.
- [try_from fallbacks silently saturating where callers assumed range] → fallback values chosen to match existing saturation behavior (u32::MAX/0); volume and backoff paths are clamped upstream, and `nextest -p mbv -p mbv-core` covers the curve math.
- [Clippy `cast_possible_truncation`/`cast_sign_loss` newly firing inside helpers] → the kernel already handles range; if clippy flags the f64→i64 path, route through the existing `as`-based body unchanged (it is the current `saturating_i64_from_f64` body, already compiling clean).

## Migration Plan

Per-family commits (A, B, C, then stragglers), each: add/move helper → migrate sites → `cargo clippy --workspace --all-targets -- -D warnings` + `cargo nextest run -p mbv -p mbv-core`. Rollback is per-commit revert; no data or wire-format impact.

## Open Questions

None.
