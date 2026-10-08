# Design

## Context

See proposal.md, Why. Findings from the code (2026-10-08):

| Crate | `src/tests` tree | Private / `cfg(test)`-only items used | Expected to move |
|---|---|---|---|
| `mbv-queue` | 7 files, 78 tests | none found | all of `src/tests*` |
| `mbv-ctrl` | 4 files, 48 tests | none (`lib.rs` has a `#[cfg(test)]` import of `PlayerCommand`/`PlayerStatus` only for the tests) | all |
| `mbv-emby` | 4 files, 67 tests | `auth_header` (`pub(super)`), `save_cached_token` (`#[cfg(test)] pub use`) | `parsing.rs`, `failure.rs`, most of `client.rs` |
| `mbv-audiobookshelf` | 4 files, 69 tests | `with_test_agent` (`#[cfg(test)] pub(crate)`); catalog wire types (`ShelfWire`, `ItemsResponse`, …, `#[cfg(test)] pub(crate) use`) | `tests.rs` root tests, `failure.rs`, `playback.rs`; `catalog.rs` stays |
| `mbv-config` | 12 files, 139 tests | `*_at` path-injected functions, `parse_config`, `checkout_*`, `SYS_ENV_LOCK`, `TEST_DEFAULT_STATE_DIR` | seven files split (public-API fragments moved, `cfg(test)`-only remainder stayed); `paths_migration.rs`, `ui_state_reset.rs`, `paths.rs`, `credentials.rs` stayed whole |

Inline `mod tests {}` blocks inside production files (`execution_sequence.rs`, `client_library.rs`,
`socket.rs`, …) test private helpers. They stay.

## Goals / Non-Goals

**Goals:**
- Each crate in scope has one integration test binary that owns its public-contract tests.
- The placement rule is written down where agents read it when they write tests.

**Non-Goals:**
- `mbv-daemon`, the TUI crate and the UI crates.
- Adding, rewriting or pruning tests. Bodies move without change, except for `use` paths.
- Widening visibility to make a test movable. The single exception is ABS `with_test_agent`
  (Decision 3).
- A mechanical gate (CI check, script). AGENTS.md forbids bespoke checkers, and no clippy lint
  covers placement.

## Decisions

1. **One binary per crate: `tests/<short>.rs` + `tests/<short>/*.rs`.** Every top-level file in
   `tests/` is its own linked binary. One binary per crate keeps link time flat. A crate-root
   test binary resolves `mod` children against `tests/`, not `tests/<short>/`, so the whole
   file content (helpers, root tests, `mod` lines) is wrapped in an inline `mod <short> { ... }`
   and the children resolve to `tests/<short>/*.rs` (precedent: `crates/mbv-queue/tests/queue.rs`).
   Child files keep `use super::*` for the shared helpers. Names: `tests/queue.rs`, `tests/ctrl.rs`, `tests/emby.rs`,
   `tests/audiobookshelf.rs`, `tests/config.rs`. The existing `tests/fixtures/` directories in
   `mbv-audiobookshelf` and `mbv-config` stay where they are. Use `env!("CARGO_MANIFEST_DIR")`
   paths when a moved test reads fixtures.
   *Alternative:* one file per former submodule. Rejected because of link cost.

2. **The compiler decides what moves.** Move a file, rewrite `use super::*` / `use crate::` to
   `use <crate_name>::…`, and build. If a test needs an item that is not public, move that test
   back into `src/tests/` unchanged. Do not change visibility. Splitting a file this way is
   expected (for example, `emby/client.rs` keeps the `auth_header` and cached-token tests in `src/`).
   *Alternative:* classify by hand first. Rejected because the compiler is exact and free.

3. **ABS `with_test_agent` becomes `pub` without `cfg(test)`**, the same as
   `EmbyClient::with_test_agent` (`crates/mbv-emby/src/client_auth.rs`). This is ABS/Emby
   parity, not a new widening. It unlocks the ABS `failure.rs` and `playback.rs` client tests.
   No other seam changes. `save_cached_token`, `auth_header`, the `mbv-config` `*_at`
   functions and the ABS wire types stay private, so their tests stay unit tests.

4. **Shared test helpers move with their tests.** A helper used both by tests that move and by
   tests that stay gets copied into both places. Do not make it public. These helpers are small
   fixture builders (for example `item(id)` in queue).

5. **Test-support features.** Moved Emby tests reach `mbv_config::TestStateDirGuard` and
   `mbv_net::mock_http` through the existing `features = ["test"]` dev-dependencies. ABS has
   the same dev-dependencies. No crate adds a self dev-dependency. `mbv_config::TestStateDirGuard`,
   `TestTempDir` and `SYS_ENV_LOCK` are `cfg(any(test, feature = "test"))`-gated and unreachable
   from the crate's own integration binary, so an `mbv-config` test that needs them stays in
   `src/`. Actual outcome: seven files had public-API fragments split out into `tests/config/`
   (55 tests); `paths_migration`, `ui_state_reset`, `paths` and `credentials` stayed whole.

6. **Drift prevention is documentation in the two places agents read before they write a test,
   plus copyable examples.**
   - `docs/invariants/14-test-ownership.md` clause 2: for a library crate, a test whose contract
     is the crate's public API is owned at the crate's `tests/` layer. "Narrowest layer" means
     the narrowest layer *that can see the contract*, not "always `src/`". Adding
     `#[cfg(test)] pub(crate) use` to reach a public-contract test is a violation.
   - `AGENTS.md` Tooling: one bullet with the same rule and a pointer to `M-INTEGRATION-TESTS`.
   - The new `tests/` binaries are the copyable pattern. Their absence was the main cause of
     the drift.
   *Alternative:* a unit test or script that flags `src/tests` files without private access.
   Rejected because AGENTS.md forbids bespoke checkers and the signal is heuristic.

## Risks / Trade-offs

- [`cfg(test)`-gated items are invisible to `tests/`] → Decision 2. Such tests stay in `src/`.
- [Integration binaries compile the crate without `cfg(test)`, so `#[cfg(test)]` imports in
  `lib.rs` can go unused after the move and fail `-D warnings`] → delete the orphaned ones
  (`mbv-ctrl` `lib.rs` `PlayerCommand`/`PlayerStatus` import, the emby `save_cached_token`
  re-export if no unit test still uses it, ABS wire-type re-exports only if `catalog.rs` no
  longer uses them).
- [Guidance can still be ignored] → accepted. The `code-review` Standards axis already checks
  `docs/standards`. The user-global `writing-tests` skill is outside this repo and is not part of
  this change.
