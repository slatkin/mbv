## Context

See `proposal.md` and the delta specs. Unified ctrl transport can already serialize tagged `QueueItem` values, but daemon admission explicitly excludes Audiobookshelf and existing unified peers may predate that enum variant. Daemon initial state and broadcasts project one canonical queue per connection capability. Step 3 already defines provider-qualified acknowledged progress at the Player boundary for bare mode.

#523 is a hard prerequisite so Local daemon and packaged `mbvd` share Service-independent Control authentication and runtime construction before this wire seam lands.

## Goals / Non-Goals

**Goals:**

- Establish the complete compatibility boundary needed by later daemon Audiobookshelf children.
- Keep old unified peers safe from unknown QueueItem variants and events.
- Preserve one canonical internal queue and secret-free transport.
- Land dormant progress plumbing without accidentally enabling playback.

**Non-Goals:**

- Loading or reconciling owner Audiobookshelf setup.
- Daemon admission, source preparation, progress generation, or client browse updates.
- Protocol-version changes or legacy queue redesign.

## Decisions

### 1. Negotiate queue and progress behavior additively

Add distinct capability strings for Audiobookshelf queue values and provider-qualified progress events. Record them in `CtrlCompatibility` and per daemon connection. Keep `CTRL_PROTOCOL_VERSION` unchanged under the additive-change rule.

Using unified-queue support alone was rejected because it does not prove that an older peer can decode the newer `QueueItem` variant. Bumping the protocol was rejected because peers can preserve all old behavior by omitting optional Audiobookshelf state.

### 2. Gate at every serialization and mutation edge

Centralize per-connection projection so initial snapshots, mutation broadcasts, track-change broadcasts, rejection snapshots, and reconnect state cannot accidentally serialize an Audiobookshelf item to an unsupported peer. Apply the same negotiated check before accepting incoming unified operations containing that item kind.

Filtering only on submission was rejected because a capable client could put an episode in the canonical queue before an older client connects. Filtering only broadcasts was rejected because an unsupported peer could still submit a value it happened to deserialize through a mismatched build.

The owner retains one canonical queue. Compatibility projection never becomes a second authoritative queue and must rebase any projected active slot/cursor coherently.

### 3. Define one redacted progress payload

Reuse the provider-qualified episode identity and acknowledged progress semantics from step 3. The ctrl payload carries identity, position/completion, and setup generation only. It omits session ID, source, header, credential, request outcome internals, and listening-time accumulator.

Transporting generic Player events was rejected because provider progress has identity and generation semantics not shared by status events. Reconstructing progress from queue snapshots was rejected because later browse reconciliation may target content with no queued occurrence.

### 4. Keep transport dormant behind existing admission

Do not change the daemon's Service-capability predicate or install owner context. Even two capable peers receive the existing visible unsupported-owner result for Audiobookshelf submission. Add an internal daemon event/broadcast helper that later activation can call, but no code path emits it in this change.

This preserves an independent rollback and proves mixed-version transport before credentials and playback lifecycles depend on it.

## Risks / Trade-offs

- **[Risk] One broadcast path bypasses capability projection** -> Route every full queue snapshot through one projection helper and cover initial, mutation, rejection, and track-change paths.
- **[Risk] Filtering changes queue coordinates for old peers** -> Derive projected active position from stable slot identity and never expose an active slot absent from the projection.
- **[Risk] Capability claims imply readiness** -> Name and document capabilities as wire support; leave owner admission unchanged.
- **[Risk] New progress payload leaks lifecycle state** -> Use a purpose-built redacted type and serialization checks for forbidden fields.
- **[Trade-off] Older clients see an incomplete owner queue** -> They preserve representable behavior without crashing; capable clients remain the authoritative presentation for Audiobookshelf queues.

## Migration Plan

1. Verify #523 and the completed bare-playback baseline are present.
2. Add capability constants, hello/compatibility derivation, and per-connection support flags.
3. Add provider-qualified redacted progress wire types and dormant fan-out plumbing.
4. Centralize queue projection and gate all inbound/outbound Audiobookshelf paths.
5. Verify mixed-capability connections while daemon Audiobookshelf admission remains disabled.

Rollback removes capability advertisement and progress plumbing first. Daemon admission is already disabled, so no playable or persisted owner behavior requires migration.
