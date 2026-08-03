## Context

See `proposal.md` for motivation and `specs/daemon-settings-management/spec.md` for observable behavior. This change follows #441, which removes client-owned playback preferences from daemon-host configuration. The remaining managed fields configure packaged `mbvd`'s mpv session, audio-pipe output, progress reporting, and pipe-intent acknowledgement.

The shared-data service already provides an authenticated full-duplex connection, additive capability negotiation, one serialized `redb` worker, durable acknowledgements, and notifications. Its current records are deliberately per Emby user. Packaged `mbvd` and hidden `mbv --__local-daemon` currently share `run_with_options`, so management support needs an explicit role boundary rather than being inferred from transport or filesystem paths.

The F2 panel currently renders one fixed client-side settings collection and saves changes to the client's `config.toml`. The daemon currently loads one `Config` at startup, but the allowlisted values are naturally consumed when creating a playback session or accepting a pipe playback intent and therefore do not require process restart if supplied through runtime state.

## Goals / Non-Goals

**Goals:**

- Reuse the shared-data connection and durable worker without making daemon settings a per-user roaming document.
- Restrict management to packaged `mbvd` and keep hidden local-daemon behavior unchanged.
- Keep the remote surface typed and limited to eight daemon-owned runtime settings.
- Preserve the existing F2 local settings behavior behind an explicit scope boundary.
- Apply every managed setting at a playback boundary without restarting the daemon.
- Make mutations durable, conflict-detecting, serialized, and visibly acknowledged.

**Non-Goals:**

- Remotely reading or patching arbitrary TOML or serializing the full `Config` structure.
- Managing credentials, endpoints, listeners, TLS, shared-data enablement, or any restart-required setting.
- Managing client-owned playback preferences removed from daemon behavior by #441.
- Adding administrator roles beyond successful shared-data authentication.
- Changing shared-data export.
- Resolving #442's separate question of whether playout-delay UX should continue to exist.

## Decisions

### Keep a separate daemon-wide override record

Add a dedicated daemon-settings table or fixed-key record in the existing database rather than adding a document kind to the per-user shared-state model. The record envelope contains an independent revision and a typed document:

```text
DaemonSettingsRecord
  revision: u64
  document:
    schema_version: 1
    use_mpv_config: Option<bool>
    no_scripts: Option<bool>
    audio_pipe_enabled: Option<bool>
    audio_pipe_path: Option<String>
    audio_pipe_samplerate: Option<u32>
    audio_pipe_bitdepth: Option<16 | 24 | 32>
    audio_pipe_playout_delay: Option<Disabled | Milliseconds(u64)>
    progress_interval_secs: Option<u64>
```

An absent field means inherit from the daemon's ordinary parsed configuration/default. The explicit delay enum distinguishes removing the override from overriding a configured delay with disabled behavior. Revision zero represents no record; the first changing mutation writes revision one. Resetting the last field retains a revisioned empty document so concurrent clients cannot recreate from revision zero.

The existing storage worker gains global read and compare-and-swap set/reset requests. It checks the expected revision first, validates the typed mutation, derives the replacement document, detects no-ops, and commits a changed document before returning. A current-revision no-op returns the unchanged record without writing, incrementing, or notifying. A stale mutation remains stale even if it would be a no-op against current state.

Per-user records and `mbvd --export-shared-data` remain unchanged. Using a synthetic user ID was rejected because it weakens user-isolation invariants. Storing one record per field was rejected because the F2 surface needs one coherent revision and ordered edit stream.

### Distinguish packaged and hidden daemon roles explicitly

Extend daemon runtime options with a role such as `Packaged` or `HiddenLocal`. The packaged `mbvd` entrypoint passes `Packaged`; `mbv --__local-daemon` passes `HiddenLocal`. Only `Packaged` loads the global override record, advertises the daemon-settings capability, handles its commands, or creates subscribers.

Inferring role from Unix versus TCP transport was rejected because packaged services can expose Unix sockets and transport does not express process purpose. Inferring from system paths or environment variables was rejected because an explicit entrypoint decision is easier to audit and test.

### Resolve only inherited versus override

The daemon resolves each effective value as `stored override` when present and `Config`'s already-resolved value otherwise. Snapshots label those states `override` and `inherited`; they do not distinguish explicit TOML from compiled defaults.

This avoids adding source-provenance metadata to configuration parsing solely for display. The client receives effective and active values from the daemon and does not import defaults, parse host configuration, or infer application state.

### Validate through one static setting registry

Centralize key names, typed values, validation, labels, and application boundaries in one fixed registry:

| Setting | Value | Application boundary |
|---|---|---|
| `use_mpv_config` | boolean | next playback session |
| `no_scripts` | boolean | next playback session |
| `audio_pipe_enabled` | boolean | next playback session |
| `audio_pipe_path` | nonempty path | next playback session |
| `audio_pipe_samplerate` | positive runtime-representable integer | next playback session |
| `audio_pipe_bitdepth` | 16, 24, or 32 | next playback session |
| `audio_pipe_playout_delay_ms` | disabled or safely representable nonnegative milliseconds | next pipe playback intent |
| `progress_interval_secs` | positive integer | next playback session |

The server is authoritative for validation. The UI uses typed editors to prevent obvious invalid input but still displays server rejection. Playout delay uses checked duration/deadline construction rather than unchecked `Instant` addition. Unknown setting identifiers are rejected; the protocol does not accept caller-supplied whole documents or arbitrary config paths.

### Load and validate overrides before accepting playback commands

For packaged `mbvd` with shared-data hosting enabled, open the existing database and read the global record before binding playback control listeners. Resolve a valid record over the loaded `Config` and initialize the runtime settings holder. Start the shared-data listener later through the existing optional-feature path.

If the settings record has an unsupported schema version or fails strict typed validation, log the error, initialize runtime values entirely from inherited configuration, and disable only daemon-settings management for that run. Preserve the record without deletion, rewrite, or partial recovery. Playback and per-user shared documents continue when their existing storage paths remain usable.

If shared-data hosting is disabled, do not load or apply overrides. The database remains intact. Failing packaged-daemon startup because management is unavailable was rejected because playback is the primary responsibility.

### Capture settings at playback boundaries

Keep a shared runtime settings holder containing effective values, active values, the document revision, and a runtime generation. A successful mutation updates effective values only. The current playback session retains its captured values.

When a new playback session is constructed, snapshot `use_mpv_config`, `no_scripts`, all audio-pipe setup values, and `progress_interval_secs` into that session. Promote those active values and increment runtime generation if effective and active state differed.

When a pipe playback intent is accepted, capture the current effective playout delay into that intent. Promote the active delay and increment runtime generation if needed. `OutputStarted` settlement reads the intent's captured delay rather than the mutable `Config` or current effective value, so later edits cannot alter an in-flight intent.

After an activation changes active state, publish a refreshed snapshot to subscribers using the same document revision and the higher runtime generation. Clients order snapshots by document revision and then runtime generation. Only the document revision is used for compare-and-swap mutations.

Mutating `Config` in place was rejected because it mixes inherited startup configuration with persisted runtime authority and makes per-session capture difficult to reason about.

### Extend the shared-data protocol additively

Advertise a capability such as `daemon-settings-management-v1` only from packaged `mbvd`, without changing either protocol version. Add commands for requesting a snapshot and mutating one typed setting with an expected document revision. Mutations are `set` or `reset`; the daemon constructs the replacement document.

Responses are snapshot, committed snapshot, stale snapshot, no-op acknowledgement, and request error. Requesting the snapshot marks that authenticated connection as subscribed. Post-commit and runtime-activation snapshots fan out to all other subscribed sessions regardless of Emby user ID because the settings are daemon-wide. Existing per-user document notifications retain current user filtering.

Any authenticated shared-data user is intentionally trusted to read and mutate packaged-daemon settings. Older clients never request a snapshot and therefore receive no unknown daemon-settings events.

Adding settings commands to playback ctrl was rejected because management uses the stable shared-data endpoint rather than the currently selected playback target. A separate listener was rejected because the shared-data service already provides the needed LAN transport, authentication, and durable worker.

### Serialize client mutations through an intent queue

The client keeps a queue of typed operations such as `set audio_pipe_bitdepth 24` or `reset no_scripts`, with at most one request in flight. It does not prebuild replacement documents. A committed or no-op response completes the pending operation and lets the next queued intent use the acknowledged revision.

A stale response always completes its correlated request and raises conflict feedback even if an equal or newer notification arrived first. The rejected operation is not retried. Later queued intents are preserved and submitted against the adopted current revision. Correlated response handling is separate from snapshot freshness checks so equal snapshots cannot leave an operation pending forever.

On disconnect, clear the authoritative snapshot, pending request, and unsent queue, then report discarded edits. Do not replay offline mutations. After authenticated reconnection, request a fresh snapshot to resubscribe and keep editing disabled until it succeeds.

### Give F2 separate local and daemon view state

Add `SettingsScope::Local | Daemon` plus independent daemon cursor, scroll, snapshot, queue, pending request, and editor state. Opening F2 defaults to `LOCAL`. A canonical two-item pill bar occupies the first content row. `Tab` and `BackTab` switch scopes, and the pills use settings-specific mouse hitboxes rather than the library selector hit map.

The local branch keeps the existing sections, activation behavior, delayed TOML save, cursor, and scroll handling. The daemon branch renders only server-provided allowlisted rows, including effective value, `inherited` or `override`, and a pending boundary when active differs from effective. It never calls the local config save path.

Boolean activation submits an explicit opposite value. Path and numeric activation use a small typed editor seeded with the effective value. Bit depth accepts only the three supported choices; playout delay also accepts `off`. Pressing `r` queues reset. While a mutation is pending, later edits remain usable and enter the queue, while the displayed snapshot remains the last authoritative state.

When the shared-data connection or capability is unavailable, `DAEMON` remains selectable but shows the reason and no editable rows. The hidden local daemon never supplies the capability. Mixing local and daemon rows was rejected because they have different persistence and ownership planes.

## Risks / Trade-offs

- [Eight fields can still drift from runtime consumption] -> Keep validation, session capture, and protocol conversion in one typed registry and verify each boundary directly.
- [Queued edits can become stale under concurrent users] -> Serialize locally, use document CAS globally, drop only the rejected intent, and preserve later explicit user actions.
- [A LAN-authenticated user can change daemon-wide behavior] -> This is the intentional playground trust model; no second authorization layer is added.
- [An invalid stored record disables management] -> Fail non-destructively to inherited behavior, preserve the record, and keep playback and per-user state operational.
- [Runtime and document state have different clocks] -> Carry document revision plus runtime generation in snapshots and use only revision for mutations.
- [Playout delay may not justify its complexity] -> Keep current behavior for this capability while #442 evaluates removal separately.

## Migration Plan

1. Complete #441 so daemon-host configuration no longer owns client playback preferences.
2. Add packaged/local daemon role, typed models, fixed-key storage, strict record validation, and startup resolution without advertising protocol support.
3. Add the runtime settings holder and capture all eight values at their specified playback boundaries.
4. Add capability-guarded snapshot, mutation, subscription, and notification messages.
5. Add client snapshot state and serialized mutation queuing.
6. Add the F2 scope pill, read-only daemon rendering, typed editing, reset, and disconnected states.
7. Existing installations start with no override record and therefore retain inherited behavior.
8. Roll back by disabling shared-data hosting or using a binary without the capability. The stored record remains dormant and `config.toml` remains unchanged.
