# Proposal

## Why

A fresh coverage measurement on `dc8d8747` (clean tree, after #767) puts production
at **78.5%** (65,186/82,966) and the workspace at 83.3% — well above the 68.6% reported
by #697, whose audit was taken on a dirty tree. What is left at the cheap end is a
small, sharply-bounded set of files that sit at **zero** coverage even though the seam
needed to test them is already built and, in two cases, already documented as existing
for that purpose.

At the same time the shipped `mbv` binary carries test scaffolding, because
`mbv-core`'s `test-support` feature is declared on a **runtime** dependency
(`Cargo.toml:42`). A clean `cargo build --release -p mbv` links four test-support
symbols into `target/release/mbv`, including a spawned thread closure from a stub
helper. Nothing in production needs the feature: all 28 gate sites are
`#[cfg(any(test, feature = "test-support"))]`, and the only two production files that
touch the guard are themselves `#[cfg(test)]`.

This is tracked in #771, and it closes out the parts of #697 that are still true. #697's
two highest-priority actions are **not** among them: both target the libmpv surface,
which is out of test scope by maintainer decision — testing that surface asserts mpv
call correctness rather than mbv logic. Those items are withdrawn here rather than
deferred, so they are not re-planned.

## What Changes

- Cover `src/app/run_loop_drains.rs` (132 uncovered lines, 0%) by injecting into the
  existing `Option<Receiver>` fields and calling the drains directly. Tests reach the
  extracted loop steps without running the untested event loop; the Audiobookshelf
  fetchers those drains start fail fast and offline under the stub's default config.
- Cover `src/app/ws_event_actions.rs` (123, 0%): 15 of the 17 `WsEvent` variants are
  pure `PlayerCommand` dispatch, asserted through the existing `spy_on_commands()`
  seam. `Play` and `UserDataChanged` need the existing mock-Emby fixture.
- Cover `src/single_instance.rs` (36, 0%) — all three `Resolution` arms plus a
  `write_pid`/`read_pid` round-trip. `resolve(socket, lock)` already takes both paths as
  parameters, so no fixture is required.
- Cover the Audiobookshelf **book** playback-session path, which is entirely
  unexercised: `create_book_playback_session_bounded` and both private helpers are
  call-count 0, while the podcast equivalents are covered. Requires one new fixture —
  no book playback fixture exists today.
- Move `mbv-core`'s `test-support` feature from `[dependencies]` to
  `[dev-dependencies]`, so test scaffolding is compiled for test targets only.
- Delete `set_tv_cursor_for_test`, verified dead (one occurrence in the tree: its own
  definition).
- Record the #697 corrections in the change so the stale findings do not resurface.

Not changed: any product behavior, any spec requirement, the `#768` daemon-loop
refactor, and the `src/mpris.rs` / `visualizer_worker.rs` gaps (unsized and out of
scope here).

## Capabilities

### New Capabilities

None. This change adds tests and narrows a build-time feature; it does not introduce
behavior.

### Modified Capabilities

None. Every behavior under test is already specified — `local-daemon-single-instance`
already specifies the lock, the socket-connectability test, and the attach/refuse
outcomes; `audiobookshelf-book-playback` already specifies the book session. Adding
coverage to an existing requirement does not change the requirement, and the
`test-support` scoping is a build property with no existing capability. `.openspec.yaml`
therefore sets `skip_specs: true`, following
`2026-09-14-right-size-tui-presentation-tests` — the change that cleared the
test-spend debt this one builds on.

## Impact

- `crates/mbv-core/src/audiobookshelf_playback.rs` and its test module plus one new
  fixture under `crates/mbv-core/tests/fixtures/audiobookshelf/`.
- `src/app/run_loop_drains.rs`, `src/app/ws_event_actions.rs`, `src/single_instance.rs`
  (test modules; no production change).
- `src/app/render/test_helpers.rs` (one function deleted).
- `Cargo.toml` only — `crates/mbv-core/Cargo.toml` keeps declaring the feature, and
  `crates/mbvd` never used it.
- No new dependency, no API change, no migration. Public surface **shrinks**:
  `mbv_core::mock_http` and the stub helpers stop being part of a release build.
