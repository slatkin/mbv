# Tasks

## 1. Conversion API (mbv-core)

- [x] 1.1 In `crates/mbv-core/src/api/types.rs`, add beside `TICKS_PER_SECOND`: the `TICKS_PER_SECOND_F64` const, `ticks_to_seconds(i64) -> f64` (the family's single `#[expect(clippy::cast_precision_loss)]` + 2^53-exactness domain doc), `i64_ticks_saturating(f64) -> i64` (promote `saturating_i64_from_f64` from `crates/mbv-core/src/player.rs:84`), and `seconds_to_ticks(f64) -> i64`. Delete the private copy in `player.rs` and the duplicated private copy in `crates/mbv-core/src/daemon/audiobookshelf.rs:10`, repointing in-crate callers. Verify: `cargo check -p mbv-core`, `cargo clippy -p mbv-core --all-targets -- -D warnings`, `cargo nextest run -p mbv-core`.
- [x] 1.2 Unit test in `api/types.rs` owning the round-trip/exactness contract: `TICKS_PER_SECOND_F64 == TICKS_PER_SECOND as f64`; `ticks_to_seconds`→`seconds_to_ticks` identity for representative values (0, 1 s, 24 h); saturation at f64 extremes (NaN→0, ±huge→clamped). Verify: `cargo nextest run -p mbv-core`.

## 2. Family A — ticks→seconds (~19 sites)

- [x] 2.1 Migrate mbv-core sites to `ticks_to_seconds` (or `TICKS_PER_SECOND_F64` where the product stays f64): `player.rs:43`, `daemon/ws.rs:147`, `player/reporting.rs:8,224`, `player/sources.rs:158`, `player/run/queue.rs:422`, `player/run/events/restart.rs:82`, `playback/queue/audiobookshelf.rs:54,108`, `api/types.rs:391,405`. Delete each expect. Verify: `cargo clippy -p mbv-core --all-targets -- -D warnings`, `cargo nextest run -p mbv-core`, `rg -c cast_precision_loss crates/mbv-core` drops accordingly.
- [x] 2.2 Migrate app-side sites: `dispatch/session/player_event.rs:115,233`, `dispatch/session/ws_event.rs:35`, `dispatch/library/event_reconcile.rs:85`, `dispatch/mouse_gestures.rs:47`, and `components/library_panel/hero.rs:417` via a new `int_ratio(numer, denom) -> f64` helper (design Decision 5) placed beside its other user in `render/components/`. Delete each expect. Verify: `cargo clippy -p mbv --all-targets -- -D warnings`, `cargo nextest run -p mbv`.

## 3. Family B — seconds→ticks (~12 sites)

- [x] 3.1 Migrate mbv-core sites to `seconds_to_ticks`/`i64_ticks_saturating`/`TICKS_PER_SECOND_F64` (rounding sites keep their `.round()`/`.trunc()` before the kernel): `player.rs:67,234`, `player/run/events/track_progress.rs:82`, `player/sources.rs:158` (f64-ticks product), `daemon/audiobookshelf.rs:13`, `audiobookshelf/catalog.rs:233`. Delete each expect. Verify: `cargo clippy -p mbv-core --all-targets -- -D warnings`, `cargo nextest run -p mbv-core`.
- [x] 3.2 Migrate app-side sites: `dispatch/cast_status.rs:101,171`, `state/playback_target.rs:162`, `dispatch/mouse_gestures.rs:24`, `dispatch/audiobookshelf/browse.rs:505`, `components/podcast_content.rs:179,463`. Delete each expect. Verify: `cargo clippy -p mbv --all-targets -- -D warnings`, `cargo nextest run -p mbv`.

## 4. mpris µs→seconds

- [x] 4.1 In `src/mpris.rs`, add file-local `us_to_seconds(i64) -> f64` carrying one expect with the 2^53-µs domain doc; migrate `mpris.rs:309,338`; delete their expects. Verify: `cargo clippy --workspace --all-targets -- -D warnings`, `cargo nextest run -p mbv`.

## 5. Family D — genuine per-site fixes (no expects added)

- [ ] 5.1 mpv volume curve: `player/runtime.rs:467,472`, `player/run/decisions.rs:87`, `player/run/commands.rs:139` — narrow via `u32::try_from` + `f64::from` (design Decision 5). Verify: `cargo nextest run -p mbv-core` (volume-decision coverage), clippy `-p mbv-core`.
- [ ] 5.2 Render/images: `chrome_player.rs:180` via `int_ratio`, `chrome_player.rs:191` via `f64::from(area.width)`, `infra/images.rs:201,207` via `u16::try_from` + `f32::from`. Verify: `cargo clippy -p mbv --all-targets -- -D warnings`, `cargo nextest run -p mbv` (seekbar render test).
- [ ] 5.3 Remaining one-offs: `state/playback_target/cast.rs:84` (u8 route), `mpris.rs:401` (u32 route), `crates/mbv-net/src/lib.rs:33` (u32 route), `mouse_gestures.rs:31` (u32 route). Verify: `cargo clippy --workspace --all-targets -- -D warnings`.
- [ ] 5.4 Sweep: `rg -n 'clippy::cast_precision_loss' src crates` returns exactly 2 hits (the two kernels). Any site that resisted its fix reports back with its specific domain argument instead of re-adding an expect. Verify: the count is 2.

## 6. Final gates

- [ ] 6.1 Full gates: `cargo fmt` (then `-- --check`), `cargo clippy --workspace --all-targets -- -D warnings`, `cargo nextest run -p mbv -p mbv-core`, `make check-code-file-lines`. Verify: all green; commit per repo git-workflow.
