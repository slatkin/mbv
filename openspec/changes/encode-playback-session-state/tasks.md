## 1. State enums

- [ ] 1.1 Create `crates/mbv-core/src/player_session_state.rs` with
      `LoadState { Ready, Pending(NonZeroU8) }` and its `begin_replace`,
      `begin_single`, and `drain` (returns whether the count hit zero)
      methods.
- [ ] 1.2 Add `StopReport { NotSent, Sent, Accepted }` to the same file.
- [ ] 1.3 Add `NextUp { Idle, Armed, Fired }` to the same file (used for both
      `next_up_*` and `queue_next_up_*` field pairs).
- [ ] 1.4 Add `IntroState { Pending, Shown, Dismissed }` to the same file.
- [ ] 1.5 Add `StartupPause { None, Holding { events_to_skip: u8 } }` to the
      same file.

## 2. Migrate `PlaybackSession`

- [ ] 2.1 Replace the 15 flag fields in `player_session_types.rs`'s
      `PlaybackSession` struct with the five enum types from section 1.
- [ ] 2.2 Fix every resulting compile error in `player_session_commands.rs`
      by replacing raw flag reads/writes with the corresponding enum
      transition or match.
- [ ] 2.3 Do the same in `player_session_events.rs`, including the
      `pending_load == 0` → stop-report-reset coupling (`LoadState::drain`'s
      return value replaces the manual zero-check).
- [ ] 2.4 Do the same in `player_session_queue.rs`, including the
      `intro_show`/`intro_hide` and `startup_pause_*` construction sites.
- [ ] 2.5 Do the same in `player_session_run.rs`.
- [ ] 2.6 Update `player_proxy.rs`'s `quit_timeout_stop_flags` (~line 408)
      and `player_runtime.rs`'s `handle_intro` (~line 592) to the new
      parameter types; grep for any other callers of either function not
      already listed in proposal.md's Impact section.

## 3. Collapse duplicated reset sites

- [ ] 3.1 Add `begin_item_lifecycle()` to `PlaybackSession` covering the
      reset performed identically at `player_session_commands.rs`'s three
      sites (~192-194, ~253-255, ~339-345).
- [ ] 3.2 Replace all three sites with calls to `begin_item_lifecycle()`.

## 4. Verify enum work

- [ ] 4.1 Replace `player_tests_session.rs:45`'s direct
      `session.pending_load = 1` with the `LoadState::begin_single()`
      constructor; grep the crate for any other direct writes to the fields
      listed in proposal.md and fix any found.
- [ ] 4.2 `cargo check -p mbv-core` clean.
- [ ] 4.3 `cargo test -p mbv-core` passes unchanged (same test count/names
      as before this change).
- [ ] 4.4 `cargo clippy --workspace --all-targets` clean.
- [ ] 4.5 `make check-code-file-lines` clean.
- [ ] 4.6 Commit the enum work (state extraction + `begin_item_lifecycle`),
      separate from the rename in section 5.

## 5. Rename `PlaybackSession` → `PlaybackRun`

- [ ] 5.1 Rename the type `PlaybackSession` → `PlaybackRun` and
      `MpvSessionConfig` → `MpvRunConfig` (fields unchanged from section 2).
- [ ] 5.2 Rename `player_session_{types,commands,queue,run,events}.rs` and
      `player_session_state.rs` to `player_run_{types,commands,queue,run,
      events,state}.rs`; update `mod` declarations and imports.
- [ ] 5.3 Confirm `SessionReporter` (`player_runtime.rs:195`) is left
      unrenamed.
- [ ] 5.4 Add the **Playback run** glossary entry to `CONTEXT.md` (local
      mpv playback loop, one per mpv invocation, distinct from Session;
      `_Avoid_: session, playback session`).
- [ ] 5.5 `cargo check -p mbv-core`, `cargo test -p mbv-core`, `cargo
      clippy --workspace --all-targets` all clean, confirming the rename
      was purely mechanical.
- [ ] 5.6 Commit the rename, separate from section 4's commit.

## 6. Manual verification

- [ ] 6.1 Play a queue, skip mid-item, stop near the end; confirm Emby
      marks the item watched (the `stopped_near_end` → played/consume path
      has no automated test).
