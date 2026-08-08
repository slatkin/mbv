## 1. Protocol: advertise the capability

- [ ] 1.1 Add an audio-only capability constant to `crates/mbv-core/src/ctrl.rs`
      alongside `CTRL_CAP_QUEUE_STATE` / `CTRL_CAP_START_INDEX` /
      `CTRL_CAP_STATUS_ONLY` / `CTRL_CAP_LIFECYCLE_SHUTDOWN`. Do NOT change
      `CTRL_PROTOCOL_VERSION` — the rule sits directly above that constant.
- [ ] 1.2 Add `CtrlHello::current_daemon(audio_only: bool)` next to
      `CtrlHello::current()`. It delegates to `current()` and appends the new
      capability only when `audio_only` is true. Leave `current()` and
      `current_client()` unchanged — `current()` is also the client hello base.
- [ ] 1.3 Thread `audio_only: bool` into `spawn_ctrl_client`
      (`daemon_core.rs:573`) from its caller in `daemon_run.rs`, and switch the
      hello at `daemon_core.rs:589` from `CtrlHello::current()` to
      `CtrlHello::current_daemon(audio_only)`.
- [ ] 1.4 Verify: `cargo check -p mbv-core` clean.

## 2. Daemon: admit instead of reject

- [ ] 2.1 Replace `audio_only_rejection` in
      `crates/mbv-core/src/daemon_core.rs:565` with an admission filter over
      `&[MediaItem]` returning the admitted items and a discard count. Keep it a
      pure function with no `Player`/`EmbyClient` argument, for the same
      testability reason the doc comment above the current function gives. Reuse
      the existing `all_audio` helper's item-type test (`daemon_ws.rs:172`).
- [ ] 2.2 Add a start-index remap helper: given the original index and which
      positions were admitted, return the index of the first admitted item at or
      after the original position, else the last admitted item. This is NOT the
      `start_idx.min(len - 1)` clamp already in `PlayItems` — that clamp stays
      for its own purpose but cannot substitute for the remap.
- [ ] 2.3 Apply the filter and remap in `daemon_control.rs` `CtrlCmd::PlayItems`
      (currently rejecting at `:361`), before `*items` is assigned and before
      `play_queue`/`play` is called. When nothing is admitted, leave the existing
      queue and cursor untouched and start no playback.
- [ ] 2.4 Apply the filter in the playback-intent path
      (`daemon_run.rs:559`), preserving the existing intent
      accept/reject/coalesce sequencing around it.
- [ ] 2.5 Apply the filter in the ws path (`daemon_ws.rs`) so Emby-started
      playback is admitted on the same terms.
- [ ] 2.6 Log every discard with its count on all three paths. Do NOT send a
      ctrl notification — reporting discards over ctrl is explicitly out of
      scope (design.md, Non-Goals).
- [ ] 2.7 Keep the `AudioOnly` rejection reachable as a backstop. It must not be
      the path a normal mixed submission now takes.
- [ ] 2.8 Verify: `cargo test -p mbv-core` passes, including the existing
      `daemon_tests.rs` cases that reference `audio_only_rejection` and
      `all_audio` (update them to the new shape).

## 3. Client: read the capability

- [ ] 3.1 Capture the daemon's advertised capabilities from the hello when a
      ctrl connection is established (`remote_player_connect.rs` handles
      `CtrlEvent::Hello`), and expose "this owner is audio-only" as a queryable
      fact on the connection.
- [ ] 3.2 Ensure the fact is available to `App` for a connection made by any
      path: `switch_to_direct_remote`, `switch_to_library_route`, and the
      startup-attached path.
- [ ] 3.3 Verify: an owner that does not advertise the capability reads as not
      audio-only, and nothing downstream changes behavior.

## 4. Client: separate the playback target from the connection

- [ ] 4.1 Add an explicit "active playback target" value on `App` (local vs the
      attached owner), set wherever a target is chosen today.
- [ ] 4.2 Audit all 27 non-test `is_remote()` call sites. For each, record which
      question it asks — "is there a remote connection" (keeps `is_remote()`) or
      "where does playback go" (reads the new value). Expect the target cluster
      in `playback_target_local.rs`, `queue_actions.rs`, `remote_slot_state.rs`,
      `consume_quit_actions.rs`; expect the connection cluster in
      `session_connect.rs`, `run_loop_events_teardown.rs`, `library_route.rs`.
- [ ] 4.3 Replace the three `debug_assert_eq!(self.player.is_remote(),
      self.player_endpoint.is_some())` pairings (`session_connect.rs:290`,
      `:380`, and the one inside `restore_local_mode`) with assertions against
      the new value. Do not simply delete them.
- [ ] 4.4 Verify: `cargo clippy --workspace --all-targets` clean and the full
      test suite passes with no behavior change yet — this group is a refactor.

## 5. Client: fall-through

- [ ] 5.1 Add a routing check at the explicit play sites that call
      `apply_route_for_playback` (`actions.rs:186`, `:231`): if the attached
      owner is audio-only and the selection is wholly non-audio, target local.
      This is NOT `resolve_route_for_play`'s library lookup — it is a separate
      capability test that runs alongside it.
- [ ] 5.2 Add a path that installs `suspended_local` as the active player and
      rebinds MPRIS, WITHOUT calling `disconnect_remote()`, without clearing
      `active_route`, and without touching `player_endpoint`. Do not reuse
      `restore_local_mode` — it does all three and stays the disconnect path.
- [ ] 5.3 Stop the owner explicitly before local playback begins. Stop, not
      pause.
- [ ] 5.4 Add the reverse path: when local playback ends by any means
      (completion, user stop, failure), re-suspend the local Player and restore
      the owner as the active target.
- [ ] 5.5 Gate fall-through to explicit user play/enqueue only. Auto-advance,
      resume, and owner-initiated events must not reach the routing check.
- [ ] 5.6 Handle the startup-attached case: `bootstrap.rs:26` builds
      `player_tab` from the remote items, so that path has no separate local
      queue for a fallen-through item. Decide and implement its behavior rather
      than assuming the two-tab arrangement.

## 6. Client: strip and stage

- [ ] 6.1 Strip non-audio items from a mixed selection before submitting to an
      audio-only owner, and report the dropped count to the user via the status
      line.
- [ ] 6.2 Do NOT add stripped items to the client's own queue. A mixed batch has
      one destination; only a wholly non-audio selection falls through.
- [ ] 6.3 Route an explicit enqueue of a wholly non-audio selection to the
      client's own queue without starting playback and without disturbing the
      owner, reporting where it went.
- [ ] 6.4 Confirm disconnecting with a staged local queue leaves it intact and
      does not start playback.

## 7. UI: the pinned row

- [ ] 7.1 Render a pinned row above the owner's items in the remote queue view
      while a fallen-through item plays, in selected-row styling, carrying a
      marker identifying it as playing on the client, fed from the local
      player's status.
- [ ] 7.2 Derive the row at render time. Do NOT insert it into
      `remote_player_tab.items` — cursor bounds, queue mutations, undo, and the
      projection slot mapping must not see a member that is not there.
- [ ] 7.3 Make the row non-selectable: cursor navigation skips it and queue
      actions cannot target it.
- [ ] 7.4 Remove the row when local playback ends.
- [ ] 7.5 Switch the visible queue scope to Local when fall-through starts and
      back when it ends.

## 8. Tests

- [ ] 8.1 Unit-test the admission filter: wholly audio, mixed, wholly non-audio,
      empty.
- [ ] 8.2 Unit-test the start-index remap: index on an admitted item, index on a
      discarded item with admitted items after, index on a discarded item with
      none after, all discarded.
- [ ] 8.3 Unit-test the capability advertisement: `current_daemon(true)`
      includes it, `current_daemon(false)` does not, `current()` is unchanged.
- [ ] 8.4 Test the routing decision as a pure predicate over (owner is
      audio-only, selection item types, explicit vs advance). Do NOT write tests
      that stand up a daemon, a socket, or a full `App` run loop.

## 9. Close out

- [ ] 9.1 Verify: `cargo clippy --workspace --all-targets` clean.
- [ ] 9.2 Verify: `make check-code-file-lines` passes. `daemon_control.rs` and
      `session_connect.rs` are the likely files to cross the 800-line cap; split
      in this change if they do.
- [ ] 9.3 Update `AGENTS.md` or `CONTEXT.md` only if the code ended up
      disagreeing with what they say. ADR 0017 already records the model.
