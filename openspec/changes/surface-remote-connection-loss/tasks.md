# Tasks

- [ ] 1.1 Writer-side death detection: on write error the ctrl writer thread
  logs a warning and sets the shared `disconnected` flag before exiting.
  Unit test: a peer that closes its read side makes the first post-close
  send flip `is_disconnected()` (hermetic in-memory socket pair).
- [ ] 1.2 `send_ctrl_cmd` on a disconnected remote returns `false` instead of
  queueing into the dead writer; covered by the same test family.
- [ ] 2.1 Queue/play submission paths surface the dead connection: enqueue
  rolls back and flashes a connection-lost toast (respecting
  `enqueue-feedback` suppression rules where they apply); play flashes
  connection-lost instead of "Requesting playback…"; slot jumps report
  rejection through the existing `CommandRejected` event path.
- [ ] 2.2 An unexpected remote disconnect (reader-side drop without an
  announced reason) flashes a connection-lost toast and performs the
  `remote-queue-disconnect` return-to-local presentation (local daemon
  re-adopt per that spec's reconnect requirement); daemon-announced shutdown
  keeps its existing dedicated handling.
- [ ] 3.1 Tick-integration coverage (`tests_tick_integration*`) for the
  disconnect event path through the shell sync pass: adopted remote queue
  visible before, local presentation after, toast shown once.
- [ ] 4.1 Spec sync: fold the deltas into `openspec/specs/`, resolve the
  stale "Remote-daemon disconnects are unaffected" wording in
  `daemon-disconnect-handling`, `openspec validate --strict`.
- [ ] 4.2 fmt / check / nextest / clippy gate; commit planning + code
  together.
