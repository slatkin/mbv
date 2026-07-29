## Context

In pipe-output deployments, mbv can observe its own request handling and player events but cannot observe when a downstream consumer, network buffer, client audio backend, or speaker makes sound audible. Treating an early player-active status as audible makes the interface look idle during the confusing part of startup and allows an equivalent Play to be interpreted as a restart while the user still hears silence.

This change is intentionally niche and builds on `reliable-daemon-playback-intents`. It improves visibility without turning mbv into a Snapserver client or control plane.

## Goals / Non-Goals

**Goals:**

- Report honest startup phases derived from mbv-owned events.
- Optionally estimate the unobservable downstream playout interval.
- Extend same-target startup coalescing through a configured estimate.
- Provide useful timing evidence for manual diagnosis.
- Remain inert for non-pipe and non-direct-daemon playback.

**Non-Goals:**

- Detecting or guaranteeing the moment audio becomes audible.
- Discovering, querying, configuring, restarting, or tuning Snapserver or another downstream component.
- Automatically deriving downstream latency.
- Reintroducing daemon spectrum streaming or audio capture.
- Adding an external dependency.

## Decisions

### 1. Separate observed phases from estimated playout

The daemon reports request-correlated phases: `Accepted`, `Resolving`, `PlayerOpening`, `OutputStarted`, and optional `OutputBuffering`. `OutputStarted` is an observation at the mbv-owned player boundary, not proof that bytes have traversed every downstream buffer. UI and documentation use “output started” and “estimated output buffering,” never “audible.”

Calling the phase `PCM flowing` would overstate what an mpv event proves; polling Snapserver would still not prove hardware audibility and would couple mbv to a downstream product.

### 2. Use one optional generic delay estimate

Pipe-output configuration accepts an optional nonnegative expected downstream playout delay. It affects only the buffering estimate, remaining-delay presentation, and deadline through which equivalent same-target Play remains startup work. The daemon owns the deadline and reports approximate remaining duration.

When unset, the daemon settles at `OutputStarted` and reports downstream delay as unknown. It MUST NOT create an indefinite buffering phase or claim an audibility estimate.

### 3. Extend playback-intent settlement

With an estimate configured, `OutputStarted` does not immediately settle Play. Equivalent same-target Play remains coalesced; different-target Play supersedes; Stop wins; and the original Play settles when the generation-bound deadline expires. After settlement, same-item Play restarts normally.

### 4. Keep presentation route-specific

Only a TUI directly controlling pipe-output `mbvd` renders these phases. With an estimate, it may show approximate time remaining; without one it states that output started and downstream delay is unknown. Local playback, attached Emby sessions, and non-pipe daemon output are unchanged.

### 5. Emit diagnostic timing without downstream integration

Each phase transition and terminal outcome is logged with request identity, generation, and elapsed monotonic milliseconds. No polling, RPC, process management, or configuration writes are made against the pipe consumer.

### 6. Document manual calibration

Documentation explains observed versus estimated phases, manual calibration, estimate drift, and that downstream tuning remains the user's responsibility. It does not prescribe a particular downstream server or promise automatic tuning.

## Risks / Trade-offs

- **[Risk]** The estimate is mistaken for an audibility guarantee → Label it estimated and distinguish it from observed phases.
- **[Risk]** A stale timer settles the wrong Play → Bind deadlines to connection and generation and re-check both on the daemon loop.
- **[Risk]** An overlarge estimate suppresses an early deliberate restart → Keep it opt-in and document that restart resumes after the deadline.
- **[Risk]** An undersized estimate permits duplicates before sound is audible → Expose timings and make calibration easy; never imply automatic accuracy.
- **[Risk]** An unset estimate leaves playback pending forever → Settle at `OutputStarted` and show downstream delay as unknown.
- **[Trade-off]** No downstream query means less automatic precision → Preserve the read-only, product-agnostic boundary for this niche setup.

## Migration Plan

1. Land `reliable-daemon-playback-intents` first.
2. Add the optional pipe-output delay setting with no behavior change when absent.
3. Add generation-bound startup phases and timing logs.
4. Extend Play settlement through the configured buffering deadline.
5. Add route-specific presentation and calibration documentation.
6. Exercise configured, unconfigured, superseded, and stopped pipe-startup scenarios.

Rollback removes the optional setting and phase extension; configurations without it retain Proposal A behavior.

## Open Questions

None. Downstream control and automatic tuning are explicit non-goals.
