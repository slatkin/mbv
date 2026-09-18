# Tasks: fix-next-up-accept-and-mpv-script-source

Reference: `design.md` decisions A1-A3 and B1-B4; spec deltas in
`specs/ctrl-protocol/`, `specs/mpv-overlay-scripts/`, `specs/unified-playback-queue/`.

## 1. Unit A - owner-addressed slot jump (stops the panic)

- [x] 1.1 Add the two shared seams on `App`: `dispatch_jump(transition)`
  (out-of-process owner -> `player.queue_play_slot`; this process is the owner ->
  `send_command(transition.into_jump())`) and `request_slot_jump(slot_id)`
  (out-of-process owner -> `queue_play_slot` and report the refusal; this process is the
  owner -> sync snapshot, mint/accept the local transition, `dispatch_jump` the
  `DispatchDecision::DispatchNow` transition).
  (Verify: `cargo nextest run -p mbv` green; the fresh-jump seam is covered by 1.3/1.4.)
- [x] 1.2 Route `src/app/player_event.rs:18` (expire promoted a transition) and `:288`
  (settle promoted a transition) through `dispatch_jump` with the transition they
  already hold - do NOT re-mint, and do NOT re-accept.
  (Verify: a test asserts the promoted transition is the one sent (the expire/settle
  tests in `actions_tests_queue_state_reseat.rs` and
  `tests_tick_integration_queue_playback.rs` stay green). Both sites are reached only
  when this process is the owner - `:8` returns early, `:277` guards the block - so no
  out-of-process request is expected from them.)
- [x] 1.3 Route the Next-Up accept event (`src/app/player_event.rs:371`) through
  `request_slot_jump` and drop the client cursor write when the owner is
  out-of-process.
  (Verify: with a stub remote player and its command receiver
  (`make_remote_app_stub_with_cmd_rx`), an accept requests
  `CtrlCmd::UnifiedQueuePlaySlot` for the next-up slot, no `PlayerCommand::JumpTo` is
  constructed, the local cursor stays on the owner snapshot, and the client keeps
  running.)
- [x] 1.4 Route `src/app/action.rs:527` through `request_slot_jump` so explicit play and
  the accept cannot diverge again.
  (Verify: `cargo nextest run -p mbv` green, including
  `rejected_remote_queue_selection_keeps_observed_playhead` and
  `queue_play_cursor_keeps_observed_progress_until_player_ack`.)
- [x] 1.5 Audit every remaining client-side jump send (repository-wide search for
  `into_jump()`, `mint_local_transition`, `accept_local_transition`) and confirm each is
  either routed through a seam or owner-side only; record the audit result in this
  change's notes.
  (Verify: the only remaining `into_jump()` call sites are the owner-side dispatch sites
  in `crates/mbv-core/src/daemon_core.rs` (`:378`, `:430`, `:476`) and the daemon-side
  `dispatch_slot_jump`; no client-side site constructs a local jump command outside the
  two seams.)
  - Audit note (unit A, HEAD of this change): repository-wide search for
    `into_jump()` / `mint_local_transition` / `accept_local_transition` after routing —
    client-side jump sends now exist only inside the two seams
    (`src/app/action.rs` `dispatch_jump` / `request_slot_jump`). Remaining
    `into_jump()` sites are owner-side only: `crates/mbv-core/src/daemon_core.rs:378`
    (`dispatch_slot_jump`'s DispatchNow arm, fed from `daemon_control.rs:273/:517`),
    `:430` (`settle_and_redispatch`), `:476` (`expire_and_redispatch`) — the daemon's
    `player` there is the in-process Playback run, not a ctrl client. Remaining
    `mint_local_transition`/`accept_local_transition` uses are the seams plus
    owner-state unit/integration tests that seed owner state directly
    (`owner_state.rs` tests, `tests_tick_integration_queue_playback.rs`,
    `actions_tests_queue_state_reseat.rs`, `tests_next_up_accept_dispatch.rs`).
- [x] 1.6 Replace the local-only `unreachable!()` arm in
  `crates/mbv-core/src/ctrl.rs:456-465` with a fallible conversion and surface the
  refusal to the caller.
  (Verify: a ctrl-level test asserts the attempt is refused, no command is delivered,
  and the caller's process survives; migrate the call sites that must follow the
  fallible form - `crates/mbv-core/src/remote_player/mod.rs:188`,
  `crates/mbv-core/src/ctrl_tests.rs:271`, `crates/mbv-core/src/daemon_tests.rs:266`.)

## 2. Unit B - one runtime source for the mpv overlay scripts

- [x] 2.1 Make script resolution a pure function over injected candidate directories (checkout, package, legacy installer path) returning the chosen path plus any unused legacy copy found; verify with hermetic unit tests covering checkout present, package only, and legacy copy present-but-ignored (no real HOME/config/state directory is read).
- [x] 2.2 Remove the user-data-directory branch from script resolution and correct the
  dead checkout fallback in `crates/mbv-core/src/config_paths.rs:53` to the checkout's
  real `scripts/` path (from `env!("CARGO_MANIFEST_DIR")` = `<checkout>/crates/mbv-core`,
  the entry script is `<checkout>/scripts/mbv.lua`).
  (Verify: the 2.1 tests pass and a checkout run resolves the checkout's entry script.)
- [x] 2.3 Apply the same resolution rule to the overlay font directory
  (`crates/mbv-core/src/config_paths.rs:63`; checkout `<checkout>/fonts`).
  (Verify: a font-resolution test mirrors the script cases.)
- [x] 2.4 Log the resolved script path where the script is handed to mpv
  (`crates/mbv-core/src/player/runtime.rs:239`) and warn, naming the path, when an
  unused legacy copy exists - emitted only on the gate that actually hands a script set
  to mpv.
  (Verify: a test asserts the resolution result the log decision is derived from (the
  pure result from 2.1; there is no log-capture seam to assert the log line itself), and
  one manual run on this machine shows the resolved path in the startup log.)
- [x] 2.5 Confirm packaging metadata still installs the whole fragment set (`Cargo.toml` script mapping, `PKGBUILD`) and that the entry script resolves its siblings from its own directory; verify the mapping lists every fragment and `openspec validate --all` passes.
- [ ] 2.6 Manual check: start Local-daemon-owned playback and confirm the on-screen accept completes the jump; then remove the legacy `~/.local/share/mbv/scripts/mbv.lua` and repeat, confirming the accept still completes the jump after the copy is gone. (manual terminal check, not an automated test)

## 3. Gates and acceptance

- [ ] 3.1 `cargo fmt --all -- --check` reports no diff.
- [ ] 3.2 `cargo clippy --workspace --all-targets -- -D warnings` is clean.
- [ ] 3.3 `cargo nextest run -p mbv -p mbv-core` passes (report any teardown flake with a clean rerun).
- [ ] 3.4 `openspec validate --all` passes.
- [ ] 3.5 Add the change's domain terms to `CONTEXT.md` (the mpv script set and its
  resolved path; the out-of-process owner) and use them consistently in the code and
  artifacts.
  (Verify: terms present; no collision with the existing `Player owner` / `Client` /
  `Stay-alive` entries or `player-target-locality`'s on-this-machine classification.)
- [ ] 3.6 Manual acceptance: reproduce the original report end-to-end (a Local daemon
  owns playback, episode near its end, press the on-screen accept) and confirm the next
  episode starts immediately with the TUI still running, and that volume scaling now
  applies once (the proposal's breaking note).
