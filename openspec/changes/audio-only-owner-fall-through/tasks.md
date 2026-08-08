Depends on `audio-only-mixed-queue-admission`. It must ship first; implementation
of this change may proceed against that change's specified admission contract.

## 1. Protocol capability

- [ ] 1.1 Add an audio-only capability constant beside the existing ctrl
      capability constants in `crates/mbv-core/src/ctrl.rs`. Do not change
      `CTRL_PROTOCOL_VERSION`.
- [ ] 1.2 Add `CtrlHello::current_daemon(audio_only: bool)` by delegating to
      `current()` and appending the capability only when true. Leave client hello
      construction unchanged.
- [ ] 1.3 Thread `audio_only` into daemon hello construction for both local and
      TCP ctrl listeners.
- [ ] 1.4 Preserve the peer's advertised audio-only fact in remote connection
      state; absence of the capability reads as false.
- [ ] 1.5 Extend an existing handshake/compatibility test to prove true and false
      advertisement and that an unknown additive capability does not prevent
      connection.
- [ ] 1.6 Verify `cargo check -p mbv-core` and `cargo test -p mbv-core`.

## 2. Player arrangement and responsibilities

- [ ] 2.1 Introduce a typed player-arrangement boundary that owns active and
      parked Player sessions, including each session's event receiver and local
      websocket resources. Represent local-only, owner-driven, and local
      fall-through arrangements without separate active-target or fall-through
      flags.
- [ ] 2.2 Expose semantic queries/accessors for eligible attachment, Transport
      owner, queue availability, queue-owning session, and active fall-through.
      Do not expose a replacement binary local/remote mode query.
- [ ] 2.3 Convert Direct remote control attachment and explicit non-local daemon
      startup to the arrangement boundary while preserving their distinct
      lifecycle/provenance state.
- [ ] 2.4 Keep Library routes outside fall-through eligibility. Do not alter
      Library-route selection, connection, or restoration behavior as part of
      this change.
- [ ] 2.5 When fall-through needs local playback and no reusable local Player
      exists, construct it through the normal local Player construction path.
      Treat failure as an ordinary playback-start error without ending the
      owner attachment.
- [ ] 2.6 Replace equality assertions between `player.is_remote()` and
      `player_endpoint` with arrangement invariants that validate attachment and
      session resource placement rather than weakening or deleting assertions.
- [ ] 2.7 Rebind MPRIS whenever the Transport owner changes.

## 3. Queue availability and command ownership

- [ ] 3.1 Change queue-scope resolution so Remote scope availability derives
      from an eligible attached owner's Bound queue, independently of the
      Transport owner.
- [ ] 3.2 Preserve the requested visible Queue scope across fall-through when
      that scope remains available.
- [ ] 3.3 Route transport commands to the Transport-owner session and queue
      replace/append/remove/move/jump commands to the session that owns the
      affected queue.
- [ ] 3.4 Audit callers of `has_direct_remote_queue()`, `player.is_remote()`,
      `playback_target_queue_scope()`, and `visible_queue_scope()`. Replace each
      use with the semantic responsibility it actually asks about; do not add
      fall-through conditionals to preserve an ambiguous helper.
- [ ] 3.5 Preserve independent Local and Remote queue models, metadata, cursors,
      and undo stacks while the local Player owns transport controls.

## 4. Origin-aware player events

- [ ] 4.1 Drain each owned Player session through the arrangement boundary and
      deliver every event to the application with stable Local or Attached-owner
      origin. Do not change `mbv_core::PlayerEvent` or the ctrl wire format.
- [ ] 4.2 Apply Local events only to Local queue/playback state and
      Attached-owner events only to owner queue/playback state.
- [ ] 4.3 During fall-through, make an Attached-owner `QueueUpdated` refresh only
      the Remote Bound queue; owner stop/completion must not consume, stop, or
      mutate the Local queue.
- [ ] 4.4 Make only a terminal Local event end fall-through and return transport
      ownership to an owner that remains attached.
- [ ] 4.5 If the attached owner disconnects or shuts down during fall-through,
      remove the attachment and Remote scope, report the loss, and allow local
      playback to continue.
- [ ] 4.6 Keep owner command rejections and authority notifications attributed to
      the owner without changing local transport ownership.

## 5. Explicit submission routing

- [ ] 5.1 Add one pure routing decision over relationship eligibility, advertised
      audio-only capability, explicit action kind, and selection item types. Use
      `MediaItem::is_audio`; do not add another audio predicate.
- [ ] 5.2 Invoke the decision before explicit play/enqueue paths mutate queues,
      Queue scope, focus, route state, or status presentation.
- [ ] 5.3 Gate eligibility to Sessions-panel Direct remote control and explicit
      non-local daemon attachment. An active Library route, Session watch, Local
      daemon, or peer without the capability retains current behavior.
- [ ] 5.4 For a mixed eligible selection, submit only audio items to the owner,
      report the dropped count, and do not stage dropped items locally.
- [ ] 5.5 For a wholly non-audio explicit enqueue, append to the client's own
      queue without starting playback, stopping the owner, or changing the
      Transport owner.
- [ ] 5.6 For a wholly non-audio explicit play, prepare or construct the local
      Player first; after preparation succeeds, stop the owner, transition to
      local fall-through, and start local playback.
- [ ] 5.7 If an owner-directed explicit play occurs during fall-through, stop
      local playback, return transport ownership to the owner, and submit there.
- [ ] 5.8 Apply the shared decision at the common play and enqueue boundaries so
      folder, artist-header, and single-item actions cannot bypass it; avoid
      duplicating routing policy at every UI entry point.

## 6. Remote queue projection

- [ ] 6.1 While fall-through is active, derive a pinned row from the local
      Player's current status when Remote Queue scope is visible.
- [ ] 6.2 Render it above the owner's items in selected-row styling with a clear
      client-playing marker and local progress. Do not insert it into the Remote
      queue model or projection slot mapping.
- [ ] 6.3 Keep the row outside cursor bounds and queue actions; remove it when
      local playback ends or the owner attachment is lost.
- [ ] 6.4 Verify the pinned-row interaction and appearance directly in the TUI;
      do not add brittle pixel/layout assertions.

## 7. Focused regression tests

- [ ] 7.1 Add one table-driven test for the routing decision covering eligible
      and ineligible relationships, capability presence, action kind, and
      wholly-audio/mixed/wholly-non-audio selections.
- [ ] 7.2 Add one arrangement transition test through public semantic outcomes:
      attachment remains, Local becomes Transport owner, Remote scope remains
      available, then a terminal Local event returns transport ownership.
- [ ] 7.3 Add one sourced-event regression test proving an owner queue update
      changes only the Remote queue and an owner stop cannot end or corrupt local
      fall-through playback.
- [ ] 7.4 Prefer extending existing queue-scope/event tests over parallel
      fixtures; remove or combine any superseded test that asserts the old
      binary local/remote invariant.

## 8. Verification and documentation

- [ ] 8.1 Run `cargo check -p mbv-core` and `cargo test -p mbv-core`.
- [ ] 8.2 Run `cargo clippy --workspace --all-targets` and the relevant root
      application tests.
- [ ] 8.3 Run `make check-code-file-lines`; keep every source file below 800
      lines by placing the arrangement and routing work in focused modules.
- [ ] 8.4 Confirm `CONTEXT.md`, ADR 0017, and the implemented symbols agree;
      update documentation in the same change if implementation naming evolves.
