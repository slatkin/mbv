## 1. Typed Settings Model And Baseline

- [ ] 1.1 Define the two-key daemon setting enum, typed override values, schema-versioned override document, revisioned record, source and apply-mode enums, resolved row, and snapshot with runtime generation.
- [ ] 1.2 Centralize validation and resolution for `broadcast_ms` and `audio_pipe_playout_delay_ms`, including explicit delay disablement and exact apply modes.
- [ ] 1.3 Add a daemon-specific configuration loader that returns parsed values plus explicit-presence provenance for the two allowlisted TOML keys, and use it from packaged and detached daemon entrypoints without changing ordinary client loading.

## 2. Durable Global Override Storage

- [ ] 2.1 Add a fixed-key daemon-settings record separate from per-user shared documents, preserving revision zero for absence and retaining a revisioned empty document after the last reset.
- [ ] 2.2 Extend the serialized storage worker with global read and atomic expected-revision set/reset mutations that validate before commit and return committed, stale, or failed outcomes.
- [ ] 2.3 Include the non-secret daemon-settings record in a distinct section of local administrative JSON export without changing per-user document keys or revisions.
- [ ] 2.4 On shared-host startup, open and validate the override record before constructing setting-dependent runtime state; fall back to host configuration/defaults and suppress settings management if the database or record is unavailable.

## 3. Runtime Settings State

- [ ] 3.1 Add a daemon runtime settings holder that tracks effective and active values plus a runtime generation, initialized from the resolved startup baseline and override document.
- [ ] 3.2 Feed active `broadcast_ms` into the broadcast loop so restart-required overrides take effect after daemon restart.
- [ ] 3.3 Promote effective playout delay at the next accepted playback boundary, capture that delay for the playback intent, and increment runtime generation without allowing later edits to alter an in-flight playback's timing.
- [ ] 3.4 Produce resolved snapshots from the baseline, current override record, and runtime holder, including accurate pending state after commits and after runtime promotion.

## 4. Additive Shared-Data Protocol

- [ ] 4.1 Add the daemon-settings capability string and typed snapshot, set, reset, committed, stale, notification, and request-error messages without changing shared-data or ctrl protocol versions.
- [ ] 4.2 Handle snapshot and mutation commands only after shared-data authentication, using the request's expected revision and returning the daemon-resolved current snapshot for both commits and stale writes.
- [ ] 4.3 Track sessions that request daemon settings and fan out global post-commit and runtime-activation snapshots only to those subscribers, independently of per-user shared-document notifications and playback authority.
- [ ] 4.4 Extend the shared client to detect capability support, request and order snapshots by document revision plus runtime generation, issue set/reset mutations, adopt stale winners without retry, and clear cached authority on disconnect.

## 5. F2 Scope And Read-Only Daemon View

- [ ] 5.1 Add independent local/daemon settings scope, cursor, scroll, snapshot, and editor state while preserving the current local settings defaults and delayed config save path.
- [ ] 5.2 Render the canonical `LOCAL` / `DAEMON` pill bar at the top of F2 with settings-specific hitboxes, `Tab`/`BackTab` navigation, and mouse selection.
- [ ] 5.3 Keep the existing sections and activation behavior unchanged in `LOCAL`; render only server-provided allowlisted rows, values, sources, and pending apply annotations in `DAEMON`.
- [ ] 5.4 Render explicit disconnected and unsupported states in `DAEMON`, prevent edits without a current authoritative snapshot, and never display a cached snapshot after connection loss.

## 6. Daemon Setting Editing

- [ ] 6.1 Implement boolean activation against the effective or existing override value and submit the mutation with the displayed snapshot revision.
- [ ] 6.2 Add a bounded numeric editor for broadcast interval and playout delay, including `off` for delay, server error display, commit acknowledgement, and cancellation without mutation.
- [ ] 6.3 Add `r` reset handling that removes an override, adopts the acknowledged inherited snapshot, and leaves already inherited settings unchanged.
- [ ] 6.4 Apply committed, stale, and notification snapshots to the daemon view; retain the last acknowledged view while a write is pending and show high-priority feedback when a stale mutation adopts another client's state.

## 7. Verification And Operator Guidance

- [ ] 7.1 Verify no override record preserves current behavior, explicit values equal to defaults report source `config`, precedence is default then config then override, and resetting the last field preserves revision history.
- [ ] 7.2 Verify invalid values, unknown setting identifiers, stale revisions, and commit failures do not mutate storage or active runtime values and return actionable responses.
- [ ] 7.3 Verify restart-required settings remain visibly pending until restart, playout delay activates only on the next playback, and active-state notifications with unchanged document revisions are accepted.
- [ ] 7.4 Verify authenticated users observe the same daemon-wide settings while per-user roaming isolation, playback authority, unsupported peers, and the existing F2 local settings behavior remain unchanged.
- [ ] 7.5 Document the two-setting allowlist, precedence and reset semantics, apply modes, shared-data dependency, LAN trust model, and rollback by disabling shared-data hosting.
- [ ] 7.6 Run formatting, linting, relevant workspace tests, and strict OpenSpec validation; resolve regressions attributable to this change.
