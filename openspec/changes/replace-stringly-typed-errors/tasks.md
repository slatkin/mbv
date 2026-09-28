# Tasks

Units run leaf-first; each owns a disjoint file set (except unit 7, which
takes all remaining `src/` files). One commit per unit. Per-unit gates:
`cargo clippy --workspace --all-targets -- -D warnings`,
`cargo nextest run -p <touched packages>`, `cargo fmt`. Contracts name what
each unit's tests own, per the test-ownership rule. Every introduced or
extended error type includes `kind_name()` (design: Decisions).

## 1. Tiny crates (feed, ctrl, player, visualizer, components)

- [x] 1.1 Introduce `FeedError`, `CtrlError`, `PlayerError`,
  `VisualizerError`, `ComponentsError` (struct + private kind enum +
  `is_*()` predicates + `kind_name()` + `Display` + `Error` + `From`), convert the 18
  `Result<_, String>` sites (`mbv-feed` 5, `mbv-ctrl` 5, `mbv-player` 5,
  `mbv-visualizer` 2, `mbv-components` 1), verify per-package nextest passes
  and `rg 'Result<.*, String>'` over the five crates returns zero

## 2. Config crate (largest unit)

- [x] 2.1 Introduce `ConfigError` covering lifecycle/admin/credentials/
  state/save/parse/launch paths, convert all 74 sites in `mbv-config`,
  keeping every user-visible message byte-identical in `Display`, verify
  `cargo nextest run -p mbv-config` passes and crate `rg` audit returns zero
- [x] 2.2 Extend the converted-path tests to assert failure kinds via
  `is_*()` predicates (contract: config failures are kind-distinguishable),
  verify new predicate tests pass under nextest

## 3. Emby crate

- [x] 3.1 Introduce `EmbyError`, convert all 42 sites in `mbv-emby`
  (`client_library`, `client_sessions`, `client_playlists`, `client_auth`),
  verify `cargo nextest run -p mbv-emby` passes and crate `rg` audit zero

## 4. Cast crate plus its src consumers

- [x] 4.1 Introduce `CastError`, convert the 22 `mbv-cast` sites, then
  convert the cast-domain `src/` consumers to it
  (`src/app/state/types/cast.rs` 39 sites, `src/app/dispatch/cast.rs`
  9 sites), verify touched-package nextest passes and both areas audit zero

## 5. Daemon binary (kind-driven exit codes)

- [ ] 5.1 Introduce `DaemonError` (or per-module errors if the 21 `mbvd`
  sites span disjoint failure domains), convert all sites, replace
  `exit_code_for_error`'s `contains` matching with `is_restart_required()` /
  `is_usage_error()` predicates preserving the 3/2/1 mapping and message
  text, verify `cargo nextest run -p mbvd` passes
- [ ] 5.2 Rewrite the `error.contains(...)` exit-path tests to construct
  each kind and assert its exit code (contract: exit codes derive from
  kinds, never message text), verify the rewritten tests pass

## 6. Mid crates (remote-player, daemon, ui-model, core, audiobookshelf)

- [ ] 6.1 Introduce `RemotePlayerError` (16 sites), `DaemonLibError`
  (10 sites in `mbv-daemon`), `UiModelError` (7 sites), convert the
  `mbv-core` contract-probe example (17 sites), verify per-package nextest
  passes and each area audits zero
- [ ] 6.2 Extend `AudiobookshelfError` with `kind_name()` and real kinds for its 4 remaining
  `Result<_, String>` sites, delete the lossy `From<String>` impl
  (collapses to `connectivity`), update its in-crate callers to classify
  properly (contract: no audiobookshelf failure is misclassified as
  connectivity), verify `cargo nextest run -p mbv-audiobookshelf` passes

## 7. Remaining src integration

- [ ] 7.1 Convert all remaining `src/` `Result<_, String>` sites (~59:
  library browse dispatch, session connect/startup/switch, state events,
  `config.rs`, overlays, feeds) to the unit 1–6 domain types, formatting via
  `Display` at UI boundaries with no new `String` errors, verify
  workspace `rg 'Result<.*, String>' -g '*.rs' crates/ src/` returns zero
  and `rg 'map_err'` shows only context-adding foreign conversions

## 8. Final verification

- [ ] 8.1 Run the end-of-change gates over the whole workspace
  (`cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo nextest run --release --test-threads=4`, `cargo fmt --all -- --check`)
  plus the zero-`String`-error `rg` audit, verify all green
