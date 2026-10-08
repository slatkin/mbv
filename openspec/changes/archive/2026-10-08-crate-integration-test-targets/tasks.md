# Tasks

Procedure for every "move" task (design.md Decisions 1–4): create `crates/<crate>/tests/<short>.rs`
with `mod` lines for the moved files under `crates/<crate>/tests/<short>/`. Move the files with
`git mv`. Rewrite `use super::*` / `use crate::…` to `use <crate_name>::…`. Remove the `mod` lines
from `src/tests.rs`. Build with `cargo nextest run -p <crate>`. If a test fails to compile because
it needs a private or `cfg(test)`-only item, move that test (and its helpers) back into
`src/tests/` unchanged. Never change visibility, except in task 4.1. Test bodies are not edited.

Each group's gate: `cargo nextest run -p <crate>` passes with the same total test count as before
the group (note the count first), and `cargo clippy -p <crate> --all-targets -- -D warnings` is
clean. Then commit the group.

## 1. mbv-queue

- [x] 1.1 Move `crates/mbv-queue/src/tests.rs` (its helpers and test bodies) and all of
  `src/tests/*.rs` into `tests/queue.rs` + `tests/queue/`. Leave the inline `mod tests` in
  `src/execution_sequence.rs`. If nothing is left in `src/tests`, delete `src/tests.rs` and the
  `#[cfg(test)] mod tests;` in `src/lib.rs`. Verify: group gate.

## 2. mbv-ctrl

- [x] 2.1 Move `crates/mbv-ctrl/src/tests.rs` and `src/tests/tests_{handshake,queue,wire}.rs` into
  `tests/ctrl.rs` + `tests/ctrl/` (drop the `tests_` filename prefix). Delete the
  `#[cfg(test)] use crate::player::{PlayerCommand, PlayerStatus};` import and
  `#[cfg(test)] mod tests;` in `src/lib.rs` if they are orphaned. Verify: group gate.

## 3. mbv-emby

- [x] 3.1 Move `src/tests/parsing.rs` and `src/tests/failure.rs` into `tests/emby.rs` +
  `tests/emby/`. Verify: group gate.
- [x] 3.2 Move `src/tests/client.rs` into `tests/emby/client.rs`. Keep in `src/tests/client.rs`
  only the tests that need `auth_header` or `save_cached_token` (for example
  `auth_header_contains_device_name_and_id` and the `authenticate_*` tests that use
  `seed_cached_token`). If `save_cached_token` is still used by a unit test, keep its
  `#[cfg(test)]` re-exports in `src/lib.rs` and `src/types.rs`. Otherwise delete them. Verify:
  group gate.

## 4. mbv-audiobookshelf

- [x] 4.1 In `crates/mbv-audiobookshelf/src/lib.rs`, make `AudiobookshelfClient::with_test_agent`
  `pub` and remove its `#[cfg(test)]`. Add `#[must_use]`, as on `EmbyClient::with_test_agent`.
  Verify: `cargo clippy -p mbv-audiobookshelf --all-targets -- -D warnings` is clean.
- [x] 4.2 Move the test bodies in `src/tests.rs`, `src/tests/failure.rs` and
  `src/tests/playback.rs` into `tests/audiobookshelf.rs` + `tests/audiobookshelf/`. Leave
  `src/tests/catalog.rs` (wire-type tests) and the inline `mod tests` in `src/socket.rs` in place.
  Keep the `#[cfg(test)] pub(crate) use catalog::{…}` re-export as long as `catalog.rs` uses it.
  Fixture reads use `env!("CARGO_MANIFEST_DIR")`. Verify: group gate.

## 5. mbv-config

- [x] 5.1 Move `src/tests/emby_admin.rs`, `src/tests/paths_migration.rs` and
  `src/tests/ui_state_reset.rs` into `tests/config.rs` + `tests/config/`. Then try each remaining
  `src/tests/*.rs` file the same way. Keep it moved only if it compiles without a private item,
  `SYS_ENV_LOCK`, `TEST_DEFAULT_STATE_DIR` or `test_support` (design.md Decision 5). Delete any
  `#[cfg(test)] pub(crate) use` in `src/lib.rs` that becomes orphaned. Verify: group gate.

## 6. Placement rule (drift prevention)

- [x] 6.1 Amend clause 2 of `docs/invariants/14-test-ownership.md`. For a library crate, a test
  whose contract is the crate's public API is owned by the crate's `tests/` integration binary.
  "Narrowest layer" means the narrowest layer that can observe the contract. Adding
  `#[cfg(test)] pub(crate) use` (or other visibility widening) so that a public-contract test can
  stay in `src/` violates the invariant. Cite #889 and `M-INTEGRATION-TESTS`. Add a "where it
  still fails" note: `mbv-daemon` has no public-contract test target yet. Verify: the clause
  reads consistently with clauses 1 and 3.
- [x] 6.2 Add one bullet to the `## Tooling` section of `AGENTS.md`, next to the "A test owns a
  contract" bullet. It says: public-API tests of a library crate go in that crate's `tests/<name>.rs`
  binary (one binary per crate, `<name>/` submodules); `src/` tests are for private items; never
  widen visibility to keep a test in `src/`. It references
  `docs/standards/rules/M-INTEGRATION-TESTS.md`. Verify: `CLAUDE.md` still points at `AGENTS.md`
  and needs no edit.

## 7. Workspace gate

- [x] 7.1 Run `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`
  and `cargo nextest run --workspace`. All pass. The workspace test total equals the total
  before group 1.

## Workflow follow-up

- File a separate issue for an `mbv-daemon` public-contract test (through `run_with_options` or
  the ctrl socket). Reference #889.
- Comment on #889 with what moved and what stayed, and why.
