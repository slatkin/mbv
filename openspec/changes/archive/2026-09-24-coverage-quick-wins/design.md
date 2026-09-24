# Design

## Context

See `proposal.md` for motivation and #771 for the origin. What shapes the approach:

- **The seams already exist and are documented as such.** `PlayerProxy::stub`'s doc
  comment reads "Test helper for root-crate integration tests that need a local player
  proxy without starting a real mpv session", and `spy_on_commands()`
  (`crates/mbv-core/src/player/proxy.rs:17,50`) installs a `cmd_tx` that
  `Player::send_command` writes to directly (`player/controller.rs:251`). `stub` sets
  `mpv_inhibited` so no player thread and no libmpv handle is ever created.
- **`make_app_stub()` already builds a full `App`** (`src/app/tests.rs:100`), and
  `App::build` installs a `TestStateDirGuard` under `#[cfg(test)]`
  (`src/app/construct.rs:74`), so any `save_prefs()` / `save_queue_state()` reached
  inside a handler lands in a thread-local tempdir rather than the developer's config.
- **`shell_run_tests.rs` already demonstrates the inject-then-call pattern** for a
  sibling loop step (`drain_card_image_completions`): send on the channel, call the
  step, assert the effect. The App-level drains never got the same treatment.
- **The four targets are at exactly zero, measured.** `run_loop_drains.rs` 0/132,
  `ws_event_actions.rs` 0/123, `single_instance.rs` 0/36; the Audiobookshelf book
  path's `create_book_playback_session_bounded` and both private helpers are
  call-count 0 while the podcast equivalents are covered.
- **The test policy is normative and recent.** `AGENTS.md` requires hermetic mocks:
  no live mpv handles, no real config or state directories, no live servers, no
  spawned product binaries. The immediate predecessor change
  (`2026-09-14-right-size-tui-presentation-tests`) added an audit for "real mpv
  handles, sockets/listeners, spawned product processes or binaries, writable
  config/state directories, live servers or Services".

## Goals / Non-Goals

**Goals:**

- Take the four zero-coverage targets off zero using only seams that already exist.
- Remove test scaffolding from the release build without inventing a test mechanism
  the repo's rules forbid.
- Leave a written record of the #697 findings that are now false, so they are not
  re-planned.

**Non-Goals:**

- The libmpv surface (`player/run/*`, `player/types.rs`, `player/runtime.rs`,
  `player/sources.rs`). Out of scope by maintainer decision; see Decisions.
- The `daemon_run.rs` loop split (#768) — separate, unstarted, and its own refactor.
- `src/mpris.rs` (238 uncovered) and `visualizer_worker.rs` (282). Unsized; adding
  them would make acceptance criteria provisional.
- Any coverage-percentage target. The predecessor change states the rule directly:
  "Do not add tests solely to preserve a coverage percentage." The four targets are
  chosen because each is a behavior claim nothing currently checks.

## Decisions

### Reach extracted loop steps directly, not through `Model::run`

`run_loop_drains.rs`'s functions are called only from `shell_run.rs`'s loop, which is
why coverage files them under "run loop". Their only caller being untested says nothing
about their testability: `drain_notif_actions`, `drain_session_events` and
`drain_audiobookshelf_events` are `App` methods, and the channels they read are
`App` fields (`notif_action_rx: mpsc::Receiver<String>`, `sessions_rx:
mpsc::Receiver<SessionEvent>`, and the `Option<…Receiver>` Audiobookshelf fields).

*Alternative rejected:* a "`Model::run`-once" harness. It drives the real terminal and
tick path (expensive, and the reason the loop is untested at all) to reach a function
that can simply be called. It also tests one pass, not the branch set.

### Let the drain's spawned fetchers fail fast, and assert the decision instead

`drain_audiobookshelf_events`' success branch calls `start_audiobookshelf_shows` /
`_shelves` / `_books`, each of which spawns a thread. Those threads are hermetic under
a stub config for a specific reason: `service_startup::audiobookshelf_client` returns
`Err(Protocol)` when `config.audiobookshelf_setup` is `None` (and `_key` returns `None`
when the secret is missing), so the thread performs **no** network I/O and sends a
single `Err` completion to `lib_tx` before exiting. `Config::default()` in
`make_app_stub` has no ABS setup.

So the tests assert the decidable parts: which receiver was drained and put back, the
generation gate, worker-disconnect handling, the authentication-rejected branch, and
the browse/progress reconciliation. Reading `lib_tx` additionally proves *which* fetch
was requested per library kind, which is the Podcast/Book dispatch decision — without
needing a server or a mock transport at the `service_startup` boundary.

*Alternative rejected:* extracting a "should fetch" predicate and testing only that.
It would leave the reconciliation logic (the bulk of the 132 lines) uncovered, which is
the interesting part.

### `single_instance`: bind a `UnixListener` on a temp path, deliberately

`resolve(socket, lock)` already takes both paths as parameters, so `Fresh` (nothing
held), `Refuse` (lock held, socket absent) and PID round-tripping are fixture-free.
`Attach` requires a *connectable* socket path, because the ADR 0006 rule the capability
specifies is that socket-file existence must never count — only a successful connect.
Reaching it means binding a listener.

This is a deliberate call, and it is the one place this change touches a line the
predecessor change listed for audit ("sockets/listeners"). The rationale is that the
policy targets **live externals**, and this is not one:

- It is an in-process peer on a path under the system temp dir, bound and closed by the
  test. Nothing is contacted; no process, service, port or binary is involved.
- It is deterministic. The repo already relies on the same category of hermetic socket
  fixture (`UnixStream::pair()` in `daemon_tests_ctrl_auth.rs`, `remote_player/tests.rs`).
- The alternative is leaving `Attach` uncovered — the arm that decides "join the
  running daemon" versus "refuse", which is the pair most worth asserting.

Recorded here so it is reviewed as a decision rather than discovered as a surprise.
`flock` semantics make the held-lock case reproducible in one process: locks attach to
the open file description, so a second `open()` on the same path conflicts.

*Alternatives rejected:* `UnixStream::pair()` cannot produce a connectable path, so it
cannot reach `Attach`. Injecting a connectability closure into `resolve` would add
production indirection to suit one test, when the existing path seam is sufficient.

### `test-support` moves to `[dev-dependencies]`, verified by probe

The fix is one manifest:

```toml
[dependencies]
mbv-core = { path = "crates/mbv-core" }

[dev-dependencies]
mbv-core = { path = "crates/mbv-core", features = ["test-support"] }
```

Cargo keeps dev-dependency features out of non-test builds, and `crates/mbvd`'s
`mbv-core` dependency never had the feature. Verified in a scratch workspace mirroring
this repo's shape (workspace root that is also a package, edition 2021, no `resolver`
field): the gated symbol is absent from the release binary, and an integration test
still sees the feature.

*Alternative rejected:* `skip_specs`-style "just leave it" — the feature being on for
every release build is what makes `#[cfg(any(test, feature = "test-support"))]` code
codegen'd into a shipped artifact.

### Record the mpv exclusion as a scope decision, not a deferral

#697's actions 1 and 2 are withdrawn, not postponed. `PlaybackRun::handle_command`
takes `&Mpv`, so a test of its body asserts that mbv calls libmpv correctly — QA for
the embedded dependency, not a check of mbv's own logic. The same reasoning excludes
`player/run/*` (~1428 uncovered lines, the largest cluster in the workspace) and
`player/{types,runtime,sources}.rs`. This will be restated wherever #771 is read, so it
is not rediscovered as "the biggest untouched gap".

### Prefer `#[rstest]` tables for the `WsEvent` command variants

15 of 17 `WsEvent` variants differ only in fixture and expected `PlayerCommand`, which
is the repo's stated case for a named `#[case]` table. Two do not fit: `SetSub` derives
its target stream from player status and can legitimately send nothing, and `Play` /
`UserDataChanged` need the mock-Emby fixture, so they get their own tests.

## Risks / Trade-offs

- **The `UnixListener` call is a judgment against a listed audit item** → Recorded
  above with rationale and the rejected alternatives; it is one test, easily removed
  if the maintainer disagrees.
- **`drain_audiobookshelf_events` tests spawn short-lived threads** → They cannot
  perform I/O under the stub config, and the tests assert on channel traffic rather
  than timing, so there is no sleep and no flake surface.
- **A release build is not covered by the test suite**, so nothing enforces the
  `test-support` scoping after this change; a future dependency edit could silently
  reintroduce it → The verification is a recorded build-time check in `tasks.md`. A
  script or CI job to assert it would violate the repo's prohibition on bespoke
  scripting as a proof mechanism, so no such guard is added.
- **`set_tv_cursor_for_test` deletion could strand its helper** → Checked:
  `tv_owner_key` is still used by `test_helpers_mounted.rs:154`, so removing the
  function leaves no dead code behind.
- **The ABS book fixture is new** → Modeled on the existing podcast fixtures
  (`play-direct.json`, `play-transcode.json`) with `media_type: "book"` and a valid
  `audio_tracks` array; `decode_book_playback_session` rejects a wrong `media_type`,
  an empty track list, a mismatched `library_item_id`, and non-finite or negative
  timings, so the fixture must satisfy all four.

## Open Questions

None. The two decisions that could have been deferred — the socket-listener judgment
and the spec-skip — are resolved above and in `.openspec.yaml`.
