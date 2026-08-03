## 1. Prerequisite And Daemon Role

- [ ] 1.1 Confirm #441 has landed so packaged and hidden daemons no longer consume `always_play_next`, `always_skip_intro`, `subtitle_mode`, `subtitle_lang`, or `audio_lang` as daemon-host preferences.
- [ ] 1.2 Add an explicit packaged-versus-hidden daemon role to common daemon runtime options and pass the correct role from `mbvd` and `mbv --__local-daemon` entrypoints.
- [ ] 1.3 Gate override loading, capability advertisement, settings commands, and subscriptions on the packaged daemon role.

## 2. Typed Settings Model

- [ ] 2.1 Define the eight-key daemon setting enum, typed values, schema-versioned override document, revisioned record, inherited/override source, application boundary, resolved row, and snapshot with runtime generation.
- [ ] 2.2 Centralize labels, conversions, and server validation for booleans, nonempty audio-pipe paths, positive sample rates, bit depths 16/24/32, positive progress intervals, and disabled or safely representable playout delays.
- [ ] 2.3 Resolve each effective value from a stored override or the existing parsed `Config` value and construct snapshots without exposing arbitrary configuration fields.

## 3. Durable Global Override Storage

- [ ] 3.1 Add a fixed-key daemon-settings record separate from per-user shared documents, preserving revision zero for absence and retaining a revisioned empty document after the final override is reset.
- [ ] 3.2 Extend the serialized storage worker with global read and atomic expected-revision set/reset mutations that check revision first, validate before commit, and return committed, stale, no-op, or failed outcomes.
- [ ] 3.3 Make current-revision no-ops acknowledge without storage writes, revision increments, or notifications while stale no-ops still return the current winner.
- [ ] 3.4 Strictly validate schema version and stored field values; preserve invalid records, fall back to inherited runtime values, and disable only daemon-settings management for that run.
- [ ] 3.5 Keep per-user storage operations and `mbvd --export-shared-data` output unchanged.

## 4. Runtime Settings Application

- [ ] 4.1 Add packaged-daemon runtime settings state that tracks effective and active values, override revision, and runtime generation without mutating inherited `Config`.
- [ ] 4.2 Snapshot `use_mpv_config`, `no_scripts`, `audio_pipe_enabled`, `audio_pipe_path`, `audio_pipe_samplerate`, `audio_pipe_bitdepth`, and `progress_interval_secs` into each new playback session and promote their active values at that boundary.
- [ ] 4.3 Capture effective `audio_pipe_playout_delay_ms` when accepting each new pipe playback intent, use checked timing arithmetic, and retain the captured delay through that intent's output-start settlement.
- [ ] 4.4 Increment runtime generation when a pending effective value becomes active and produce an updated resolved snapshot without changing the document revision.
- [ ] 4.5 Load and validate packaged-daemon overrides before accepting playback commands; keep playback operational from inherited settings when management initialization fails.

## 5. Additive Shared-Data Protocol

- [ ] 5.1 Add the packaged-only daemon-settings capability string and typed snapshot, set, reset, committed, stale, no-op, notification, and request-error messages without changing shared-data or ctrl protocol versions.
- [ ] 5.2 Handle snapshot and mutation commands only after shared-data authentication, using the request's expected revision and returning the daemon-resolved current snapshot for every outcome.
- [ ] 5.3 Treat a successful snapshot request as subscription and fan out global post-commit or runtime-activation snapshots only to subscribed sessions, independently of per-user notifications and playback authority.
- [ ] 5.4 Ensure older clients receive no unsolicited daemon-settings messages and hidden local daemons never advertise or handle the capability.

## 6. Shared Client State And Mutation Queue

- [ ] 6.1 Extend the shared client to detect capability support, request snapshots, track document revision plus runtime generation, and discard authoritative state on disconnect.
- [ ] 6.2 Add a typed mutation queue with at most one request in flight; send each later intent using the revision acknowledged by the preceding response.
- [ ] 6.3 Complete pending requests by correlation even when their embedded snapshot is equal to or older than one already received, while applying only authoritative snapshot state.
- [ ] 6.4 On stale response, adopt the current winner, do not retry the rejected intent, and preserve later queued intents against the adopted revision.
- [ ] 6.5 On disconnect, clear the pending request and unsent queue with visible feedback; after reconnection request a fresh snapshot to resubscribe before enabling edits.

## 7. F2 Scope And Daemon View

- [ ] 7.1 Add independent local/daemon settings scope, cursor, scroll, snapshot, queue, pending-request, and editor state while preserving existing LOCAL defaults and delayed config saving.
- [ ] 7.2 Render the canonical `LOCAL` / `DAEMON` pill bar at the top of F2 with settings-specific hitboxes, `Tab`/`BackTab` navigation, and mouse selection.
- [ ] 7.3 Keep existing sections and activation behavior unchanged in `LOCAL`; render only server-provided allowlisted rows, effective values, inherited/override sources, and pending application boundaries in `DAEMON`.
- [ ] 7.4 Render explicit disconnected and unsupported states in `DAEMON`, prevent edits without a current snapshot, and never display a cached snapshot as authoritative after connection loss.

## 8. Daemon Setting Editing

- [ ] 8.1 Implement boolean editing for `use_mpv_config`, `no_scripts`, and `audio_pipe_enabled` by queuing explicit typed values.
- [ ] 8.2 Add a nonempty path editor for `audio_pipe_path` with cancellation and server rejection feedback.
- [ ] 8.3 Add typed numeric/choice editors for sample rate, bit depth, progress interval, and playout delay, including `off` for delay and exactly 16/24/32 for bit depth.
- [ ] 8.4 Add `r` reset handling that queues override removal and adopts the acknowledged inherited value.
- [ ] 8.5 Apply committed, no-op, stale, and notification snapshots correctly; retain the last authoritative display while writes are pending and show conflict feedback for stale mutations.

## 9. Verification And Operator Guidance

- [ ] 9.1 Verify packaged `mbvd` exposes exactly the eight-setting capability while `mbv --__local-daemon` and unsupported peers retain existing behavior.
- [ ] 9.2 Verify inherited values, overrides, resets, final-field reset, current-revision no-ops, stale no-ops, monotonic revisions, and unchanged shared-data export.
- [ ] 9.3 Verify every playback-session setting affects the next session without altering the active session or requiring daemon restart.
- [ ] 9.4 Verify playout delay is captured per accepted pipe intent, later edits do not alter an in-flight intent, and unsafe timing values are rejected without panic.
- [ ] 9.5 Verify concurrent clients produce one committed winner, correlated stale responses complete pending edits, later queued intents continue, and disconnect clears unsaved edits before resubscription.
- [ ] 9.6 Verify malformed or unsupported stored documents disable only daemon-settings management, preserve the record, and leave playback and per-user shared state operational.
- [ ] 9.7 Verify all authenticated shared-data users observe and can mutate the same daemon-wide settings while unauthenticated requests fail and playback authority remains unchanged.
- [ ] 9.8 Verify the F2 LOCAL panel behaves unchanged and the DAEMON panel handles keyboard, mouse, typed editing, reset, pending boundaries, unsupported service, and reconnection states.
- [ ] 9.9 Document the eight-setting allowlist, inheritance/reset semantics, application boundaries, shared-data dependency, authenticated-LAN trust model, #442 caveat, and rollback by disabling shared-data hosting.
- [ ] 9.10 Run formatting, linting, relevant workspace tests, and strict OpenSpec validation; resolve regressions attributable to this change.
