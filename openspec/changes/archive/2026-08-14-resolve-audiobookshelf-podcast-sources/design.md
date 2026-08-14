## Context

See `proposal.md` and the delta specs. #516 supplies the Audiobookshelf QueueItem, typed identity, Service-aware admission, persistence, and owner-local context seam while leaving every owner ineligible. mpv currently mirrors all resolved queue URLs and its playlist coordinates are interpreted as canonical queue coordinates.

Audiobookshelf permits one same-device playback session at a time and may start a transcode. Resolving inactive slots would close the active session and create unused work. This child builds the source machinery and projection behind the still-disabled admission gate.

## Goals / Non-Goals

**Goals:**

- Pin the live Audiobookshelf 2.36 playback contract before implementing decoders.
- Prepare one active source with safe per-file options and authoritative resume.
- Keep one canonical queue while mpv materializes one file for lifecycle-backed runs.
- Clean up sessions opened by failed preparation or load.

**Non-Goals:**

- User-facing Audiobookshelf playback eligibility or episode actions.
- Periodic listening-progress synchronization and full teardown finalization.
- ctrl transport, daemon playback, Socket.IO, or audiobook support.

## Decisions

### 1. Treat live fixtures and mpv HLS validation as hard gates

Capture sanitized direct, forced-transcode, sync, close, and failure responses from Audiobookshelf 2.36. Verify REST-only HLS readiness and seeking with the repository's libmpv integration before finalizing wire types or retry behavior. If Socket.IO is required, stop and revise rather than expanding scope.

Exact readiness timing follows observed behavior and remains bounded; the design does not invent retry counts before validation.

### 2. Keep playback context and prepared sources owner-local

The in-process Player receives a generation-tagged Audiobookshelf context containing current setup, secret access, and one persistent non-secret mbv device identifier. It clears on rejection, replacement, or removal. The context does not reuse Emby credentials and never enters queue state.

A fallible prepared-source result carries URL, per-file mpv options, authoritative start position, and optional provider lifecycle. Preparation occurs inside the owner, not in serializable `PlayerCommand`, ctrl, UI events, or persistence. Emby and Feed adapt to the boundary without changing behavior.

### 3. Validate one returned podcast track and isolate credentials

Session creation validates requested library/episode identity and exactly one supported audio track. Direct paths are joined safely to the configured server and receive a per-file Bearer option. HLS paths are session-scoped, receive no credential, and use bounded readiness probing. All authenticated options and URLs are redacted from diagnostics.

Alternative rejected: global mpv headers, URL credentials, media proxying, or eager session creation.

### 4. Add a one-way projection mode transition

Playback runs have two projection modes:

```text
Eager:       canonical slots <=> mpv playlist entries
                         |
                         | first Audiobookshelf slot enters the run
                         v
Active-file: canonical slots => mpv [active file]
```

Cold active-file start prepares the selected canonical slot. Transition from an eager run retains its active canonical slot, removes inactive mpv entries, and leaves exactly that file materialized. The run never returns to eager mode; a later run may choose eager mode if it contains no lifecycle-backed item.

Canonical slot identity drives explicit selection, advance, skip, consume, remove, move, append, replace, and stop. Inactive mutations never require mpv commands. If the active slot changes, finalize the current source lifecycle within this child's cleanup scope, choose the next canonical slot, prepare it, and replace mpv's file.

### 5. Treat mpv playlist observations as projection-local

In active-file mode, `playlist-pos` and `playlist-count` describe only `[active file]`. They may confirm adapter state but cannot change canonical length, order, or current slot. Natural EOF is interpreted against the active canonical slot and advances through canonical queue policy rather than mpv index arithmetic.

### 6. Install lifecycle before load and close failed sources

Once a server session opens, retain enough owner-local lifecycle state to close it even if validation, HLS readiness, or mpv load/start fails. Use bounded authenticated close and clear local state regardless of outcome. Full periodic synchronization and ordered finalization across every Service/process exit belong to #518.

## Risks / Trade-offs

- **[Risk] Active-file mode leaves old mpv index assumptions live** -> Isolate command/event handling by projection mode and assert observations cannot mutate canonical coordinates.
- **[Risk] Per-file headers leak to the next source** -> Construct options per load and verify the following Emby/Feed source has no Audiobookshelf header.
- **[Risk] A session opens before source failure** -> Install lifecycle state before validation/load and route all failure exits through bounded close.
- **[Trade-off] Source machinery lands before users can invoke it** -> Keep admission disabled; this makes the risky API/projection boundary independently reviewable without shipping incomplete reporting.

## Migration Plan

1. Confirm #516 is applied and all owners still reject Audiobookshelf items.
2. Capture fixtures and validate direct/HLS behavior; stop if the contract invalidates the plan.
3. Add playback API types, methods, stable device identity, and owner-local context.
4. Add prepared sources and header isolation.
5. Add active-file projection and projection-specific command/event rules.
6. Verify failed-session cleanup and confirm user-facing admission remains disabled.

Rollback removes dormant source/projection support; queue representation from #516 remains intact.
