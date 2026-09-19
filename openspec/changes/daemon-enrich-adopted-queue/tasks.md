## 1. Daemon: async refresh on cold adoption

- [ ] 1.1 Add `DaemonEvent::QueueEnriched(Vec<EmbyItem>)` (or equivalent name)
      to the enum in `crates/mbv-core/src/daemon_core.rs`, mirroring
      `DaemonEvent::AudiobookshelfProgress`'s shape. Verify: `cargo check -p
      mbv-core` compiles once the new variant has a handler (task 1.3).
- [ ] 1.2 In `daemon_control.rs`'s `CtrlCmd::UnifiedAdoptQueue` handler
      (`crates/mbv-core/src/daemon_control.rs:126-180`), after installing the
      admitted items into `*queue`, spawn a background thread using the
      already-in-scope `client: &Arc<Mutex<EmbyClient>>` that calls
      `get_items_by_ids` for the adopted Emby item IDs and sends the result
      through `merged_tx` as the new `DaemonEvent`. Skip the fetch entirely
      when the adopted queue has no Emby items (mirror
      `spawn_enrich_queue_state`'s early return,
      `src/app/queue_actions_playlist_mutation.rs:314-319`). Verify: a unit
      test (mocked `EmbyClient`, no live network — see task 1.4) confirms the
      thread is spawned and sends the event; the ctrl handler itself keeps
      returning immediately (no blocking on the fetch).
- [ ] 1.3 Handle the new `DaemonEvent` in `daemon_run.rs`'s event loop: call
      `owner.core.queue.merge_refresh(items)` (reuse `PlaybackQueue::merge_refresh`,
      `crates/mbv-core/src/playback/queue.rs:543`, no new merge logic), then
      `broadcast_queue_state` on a non-empty merge result, mirroring the
      `DaemonEvent::AudiobookshelfProgress` handler's shape
      (`daemon_run.rs:597`). Verify: `cargo check -p mbv-core` compiles; a
      unit test (task 1.4) asserts the merged queue is broadcast.
- [ ] 1.4 Add a daemon-side unit test (mocked `EmbyClient`/`Player`, no live
      mpv or network — see `.claude/skills/writing-tests` conventions)
      proving: adopting a persisted snapshot with stale positions, then
      simulating the fetch's `DaemonEvent` arriving, updates
      `owner.core.queue`'s item positions and triggers a broadcast. Add a
      second case proving a slot already played (progress applied via the
      existing `apply_completion_progress` path) is not overwritten by a
      refresh result that arrives after it. Verify: `cargo nextest run -p
      mbv-core` passes both new tests.

## 2. Client: remove the now-redundant post-adoption enrichment

- [ ] 2.1 Remove the `spawn_enrich_queue_state` call from the
      daemon-adoption path in `src/app/daemon_restart.rs` (~line 102-103) and
      `src/app/construct.rs` (~line 611-612). Leave the plain-local
      (Bare-mode) restore path (`src/app/queue_actions_playlist_mutation.rs`'s
      `restore_queue_state`) untouched. Verify: `cargo check -p mbv` compiles
      and `cargo clippy --workspace --all-targets -- -D warnings` is clean
      (no now-unused `bootstrap.positions` field access left dangling — see
      task 2.2 for whether the field itself is still needed).
- [ ] 2.2 Decide whether `LocalDaemonBootstrap.positions`
      (`src/app/bootstrap.rs:24-29`) is now dead weight for the
      daemon-adoption path (its only consumer was the removed call in task
      2.1) or still needed for some other path; if dead, remove the field and
      its doc comment's claim that daemon-adoption enrichment uses it, update
      `bootstrap_local_daemon_queue`'s construction accordingly. Verify:
      `cargo check -p mbv` compiles with no unused-field warning.
- [ ] 2.3 Update or remove any test in `src/app/tests_daemon_bootstrap.rs`
      that asserts the now-removed post-adoption `spawn_enrich_queue_state`
      call happens. Verify: `cargo nextest run -p mbv tests_daemon_bootstrap`
      passes.

## 3. Documentation

- [ ] 3.1 Add a short cross-reference in
      `docs/invariants/06-queue-progress-application-sites.md` noting the new
      cold-adopt refresh as a fourth trigger that writes into the daemon's
      canonical queue, alongside the existing `TrackCompleted`/`Stopped`
      sites it already documents. Verify: the doc still reads as one
      coherent invariant, not a bolted-on addendum — reread it end to end
      after editing.

## 4. Verification and sync

- [ ] 4.1 Full workspace check: `cargo fmt --all`, `cargo clippy --workspace
      --all-targets -- -D warnings`, `cargo nextest run -p mbv-core`, `cargo
      nextest run -p mbv` — all clean, matching the standard already held for
      this queue-progress work (see commits 238ed4d8, f91120e2, f6dbb25c).
- [ ] 4.2 Manual check against the original report: restart the Local
      daemon with a persisted queue containing items with real Emby-side
      progress that were not played in the prior session, reconnect a
      client, and confirm the QueueList shows their progress without playing
      anything first (the exact repro from the screenshot that motivated this
      change).
- [ ] 4.3 Sync the `unified-playback-queue` delta spec into
      `openspec/specs/unified-playback-queue/spec.md` and archive this change
      once the above is verified (per this repo's `openspec-archive-change`
      workflow).
