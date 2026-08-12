## 1. Carry addressable Feed progress

- [x] 1.1 Add serde-defaulted feed identity, position, and played fields to `FeedEntry`, and update existing FeedEntry fixtures and legacy deserialization coverage.
- [x] 1.2 Assign each fetched entry the normalized stored subscription URL as its `feed_id`, preserving it through per-subscription lists, the All group, queue persistence, and Feed-capable ctrl snapshots.
- [x] 1.3 Extend `QueueItem` progress accessors and playback-queue SlotProgress construction/application so Feed slots retain local position and played state without entering Emby sync state.

## 2. Share resume eligibility

- [x] 2.1 Extract an integer-safe `should_resume(position_ticks, runtime_ticks)` predicate and named 6% threshold, preserving positive unknown-runtime behavior and rejecting non-positive positions.
- [x] 2.2 Delegate `EmbyItem::should_resume` to the shared predicate and adapt the existing resume tests to the inclusive 6% boundary.
- [x] 2.3 Update every Feed load/queue transition path to use Feed progress and the same predicate instead of hardcoding a zero start position.

## 3. Hydrate Feed state at playback submission

- [x] 3.1 Before each explicit Feed play submission, read #492 state by `(feed_id, guid)` when supported and copy the returned position/played values into the submitted FeedEntry.
- [x] 3.2 Make missing identity, absent state, unsupported capability, disconnection, and read failure fall through to stateless playback without rejecting the play action or contacting Emby.

## 4. Persist event-driven Feed progress

- [x] 4.1 Add one App helper that resolves an addressable Feed queue slot, derives its lifecycle state, updates queue progress, and writes `FeedEntryState` through #492 without invoking Emby progress reporting.
- [x] 4.2 Call the helper for `Stopped` and `TrackCompleted` before consume/removal changes the queue, storing played with position zero for known-runtime EOF or stop at/above 95%.
- [x] 4.3 Persist the current Feed position on `PausedChanged(true)` and on one confirmed output restart gated by a pending seek marker; do not write on initial startup, buffering restarts, unpause, or time-position ticks.
- [x] 4.4 Keep unknown-runtime EOF unplayed, log state-write failures without stopping playback, and verify unavailable shared state remains a no-op.

## 5. Reconcile planning and verify

- [x] 5.1 Remove the superseded `openspec/changes/raise-playback-resume-threshold/` artifacts so this change is the sole active plan for #438's requirement.
- [x] 5.2 Update the narrow existing queue/player tests that protect Feed serialization compatibility, shared 6% resume behavior, and event-driven completion; avoid brittle UI-state assertions.
- [x] 5.3 Run `cargo fmt --all -- --check`, `cargo check -p mbv-core`, `cargo clippy --workspace --all-targets`, and `make check-code-file-lines`.
- [x] 5.4 Manually verify half-play/stop/replay resumes, EOF and stop-at-95% mark played, pause and seek write once, and playback remains stateless when the shared daemon is unavailable.
- [x] 5.5 After delivery, close #438 as absorbed by #493.
