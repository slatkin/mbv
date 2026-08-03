## Context

See `proposal.md` for motivation and `specs/daemon-settings-management/spec.md` for observable behavior. The shared-data service already provides an authenticated full-duplex connection, additive capability negotiation, one serialized `redb` worker, durable acknowledgements, and notifications. Its existing records are deliberately per Emby user and limited to four roaming document kinds.

The daemon currently receives one flattened `Config` whose values no longer reveal whether they came from TOML or a compiled default. It captures `daemon_broadcast_ms` when starting the broadcast thread. Audio-pipe settings are consulted when playback sessions start, while playout delay is also read later when output starts. The F2 panel currently renders one fixed client-side setting collection and saves changes back to the client's `config.toml`.

## Goals / Non-Goals

**Goals:**

- Reuse the shared-data connection and durable worker without making daemon settings a fifth per-user roaming document.
- Keep the remote surface typed, small, and server-resolved so clients do not duplicate daemon defaults or runtime knowledge.
- Preserve the existing F2 local settings behavior behind an explicit scope boundary.
- Represent persisted desired values separately from values active in the running daemon.
- Make every accepted mutation durable and conflict-detecting.

**Non-Goals:**

- Remotely reading or patching arbitrary TOML or serializing the full `Config` structure.
- Managing credentials, endpoints, listeners, TLS, shared-data enablement, or authorization policy.
- Adding administrator roles beyond successful shared-data authentication.
- Dynamically applying settings classified as restart-required.
- Generalizing the first two fields into a plugin, schema registry, or generic form system.

## Decisions

### Keep a separate daemon-wide override record

Add a dedicated daemon-settings table or fixed-key record in the existing database rather than adding `DaemonSettings` to `SharedDocumentKind`. The record envelope contains an independent revision and a typed document:

```text
DaemonSettingsRecord
  revision: u64
  document:
    schema_version: 1
    broadcast_ms: Option<u64>
    audio_pipe_playout_delay: Option<Disabled | Milliseconds(u64)>
```

An absent field means inherit. The explicit delay enum distinguishes removing the override from overriding a configured delay with disabled behavior. Revision zero represents no record; the first mutation writes revision one. If resetting the last field produces an empty document, retain the revisioned empty document so concurrent clients cannot accidentally recreate from revision zero.

The existing storage worker gains global read and compare-and-swap mutation requests. Validation and mutation occur inside that serialized operation, and the transaction commits before a response is returned. Per-user records and their export shape remain independent; the administrative JSON export may include the non-secret global daemon-settings record in a separate top-level section.

Using a synthetic user ID in the existing per-user table was rejected because it weakens the storage model's user-isolation invariant and invites accidental filtering or export behavior. Storing one document per field was rejected because the settings are presented and edited as one small control surface and clients need one coherent revision.

### Build a daemon-owned resolved snapshot

Define typed protocol models rather than sending arbitrary JSON paths:

```text
DaemonSettingKey = BroadcastMs | AudioPipePlayoutDelayMs
SettingSource = Override | Config | Default
ApplyMode = NextPlayback | RestartRequired

DaemonSettingsSnapshot
  revision
  runtime_generation
  rows[]:
    key
    effective_value
    active_value
    override_present
    source
    apply_mode
```

The daemon resolves every row as `override > explicit host config > compiled default`. `active_value == effective_value` means no pending application. The client renders this model and never imports default values, TOML paths, validation limits, or apply classifications.

`runtime_generation` increments when an active value changes without a document commit, such as promoting a playout delay at the next playback boundary. Clients order snapshots by `(revision, runtime_generation)`: a lower revision is stale; for equal revisions a lower or equal runtime generation is stale. The document revision alone remains the compare-and-swap token.

Returning only the raw override document was rejected because the client cannot reliably know explicit host configuration, daemon defaults, or whether a startup-captured value is active. Returning only effective values was rejected because reset affordances and pending state require override and active metadata.

### Preserve explicit host-config provenance at daemon startup

Add a daemon-specific configuration loader that returns the existing parsed `Config` plus a small `DaemonSettingsBaseline`. The baseline contains each allowlisted parsed value and whether its current TOML key was explicitly present. Both packaged `mbvd` and the detached local-daemon entrypoint pass this baseline into daemon startup; ordinary client configuration loading remains unchanged.

The baseline follows the paths consumed by the parser for the two existing settings and centralizes those paths so source reporting cannot drift from parsing. It does not add provenance for unrelated configuration fields. Comparing parsed values to defaults was rejected because an explicitly configured value equal to the default must still report source `config`. Re-reading TOML for every settings request was rejected because the daemon does not otherwise hot-reload host configuration and repeated reads could report a baseline different from the one actually used at startup.

### Resolve overrides before constructing daemon runtime

When shared-data hosting is enabled, daemon startup opens the existing database and reads the global override record before constructing `Player` or starting setting-dependent loops. A valid record is resolved over the startup baseline and supplies the initial active values. The listener still starts only after playback-critical initialization. If the database cannot be opened or the record cannot be validated, log the error, use host configuration/defaults, and keep playback operational without exposing daemon-settings management.

Disabling shared-data hosting disables remote settings management and ignores stored overrides on the next daemon start; the database remains intact for re-enablement. This provides the same non-destructive rollback boundary as the underlying shared-data feature.

Opening and resolving after `Player` construction was rejected because restart-required overrides would never become active. Failing daemon startup on an unavailable settings store was rejected because remote management is optional and must not take playback down.

### Apply the initial fields at their real runtime boundaries

The initial registry is static code with two entries:

| Setting | Host config value | Validation | Apply mode |
|---|---|---|---|
| `broadcast_ms` | integer milliseconds | `>= 100` | restart required |
| `audio_pipe_playout_delay_ms` | disabled or integer milliseconds | `>= 0` | next playback |

For restart-required fields, a commit updates the effective value but leaves the runtime value unchanged. At the next daemon start, pre-runtime resolution makes the persisted value active.

For playout delay, keep a daemon runtime settings holder with separate effective and active values. A successful commit updates only the effective value. Acceptance of the next play request promotes it to active, increments `runtime_generation`, and captures that active delay for the playback intent so an update after playback begins cannot alter that playback's output-start accounting.

Pretending both are live was rejected because the current broadcast loop captures its value at startup, while playout delay is applied at the next playback boundary. Restarting the daemon automatically after a commit was rejected because it would disrupt playback and hide an operationally significant action.

### Extend the shared-data protocol additively

Advertise a new capability string such as `daemon-settings-management-v1` without changing either protocol version. Add commands for requesting a snapshot and mutating one typed setting with an expected document revision. Mutation operations are `set` and `reset`; the daemon constructs and validates the replacement document rather than accepting a caller-supplied whole document.

Responses are snapshot, committed snapshot, stale snapshot, and request error. After commit, notify all other authenticated sessions that have requested daemon settings during the current connection, regardless of Emby user ID, because the document is daemon-wide. Runtime activation also sends a refreshed snapshot to those subscribed sessions. Existing per-user document notifications retain their current filtering.

Any authenticated shared-data session may read and mutate daemon settings. Tracking subscribers prevents older clients from receiving unsolicited event variants they do not understand even when connected to a newer daemon.

Adding settings commands to the playback ctrl connection was rejected because the F2 daemon surface depends on the stable shared-data endpoint, not the currently selected playback target. A separate listener was rejected because the existing service already supplies the needed LAN transport, authentication, and durability boundary.

### Give F2 separate local and daemon view state

Add a `SettingsScope` with `Local` and `Daemon`, plus independent daemon cursor/editor state. Opening F2 defaults to `LOCAL`, preserving current behavior. A canonical two-item pill bar occupies the first content row. `Tab` and `BackTab` switch scopes, and the pills have settings-specific mouse hitboxes rather than reusing the library selector hit map.

The local branch keeps the existing sections, activation behavior, delayed TOML save, cursor, and scroll handling. The daemon branch renders only the two server-provided rows, including effective value, source, and a pending `next playback` or `restart required` annotation when active differs from effective. It never calls the local config save path.

Boolean activation sets an explicit value opposite the effective value, or toggles the existing override. Numeric activation opens a small typed editor seeded with the effective value; playout delay also accepts `off`. Pressing `r` on a daemon row requests reset. All mutations use the snapshot revision and leave the UI on the last acknowledged snapshot until a committed or stale response arrives. A stale response replaces the displayed snapshot and raises the existing high-priority status notification.

When no active shared-data connection or capability exists, the daemon pill remains selectable but the branch shows a reason and has no editable rows. Cached snapshots are discarded on disconnect so stale values cannot look authoritative.

Mixing local and daemon rows in one list was rejected because duplicate labels control different processes and persistence planes. Reusing the current selector hit map was rejected because its click dispatcher assumes library navigation semantics.

## Risks / Trade-offs

- [The initial allowlist is intentionally small] -> Keep field metadata centralized so later additions are explicit capability changes rather than arbitrary config exposure.
- [Restart-required changes may surprise users] -> Always show active and effective divergence with the apply mode; never auto-restart.
- [A LAN-authenticated user can change daemon-wide behavior] -> Make this explicit and reuse the existing shared-data boundary; administrator roles remain a future capability if the playground's trust model changes.
- [An override database failure can make startup behavior differ from the prior run] -> Log clearly, fall back to host config/defaults, and do not expose an apparently writable daemon tab.
- [Current configuration parsing loses source provenance] -> Capture only the two required presence bits in the daemon-specific loader and test explicit-default values.
- [Protocol notifications are global while roaming notifications are per-user] -> Track daemon-settings subscribers separately and keep event variants and fan-out paths distinct.
- [Runtime and document state have different clocks] -> Carry both document revision and runtime generation in every resolved snapshot; use only the document revision for mutations.

## Migration Plan

1. Add the daemon baseline, typed models, fixed-key database record, and startup resolution without advertising protocol support.
2. Add capability-guarded snapshot/mutation messages and global subscriber notifications.
3. Add runtime active/effective tracking and next-playback promotion for playout delay.
4. Add the F2 scope pill bar and read-only daemon snapshot rendering, then enable validated editing and reset.
5. Existing installations start with no override record and therefore retain their current host configuration/default behavior.
6. Roll back by disabling shared-data hosting or running a binary without the capability. The stored record remains dormant and `config.toml` remains unchanged.
