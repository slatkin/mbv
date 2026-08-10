## Why

The keyed feed-entry state store from #492 has shipped, but playback does not yet read or write it, so feed entries always restart and never become played. Wiring that store into lifecycle events now completes the next RSS milestone while consolidating Emby and feed resume eligibility under the same 6% rule.

## Tracking

GitHub issue: [#493 — RSS-D2: Feed resume + played wiring (event-driven) + shared 6% resume threshold](https://github.com/slatkin/mbv/issues/493)

Depends on shipped issue [#492](https://github.com/slatkin/mbv/issues/492). This change absorbs [#438](https://github.com/slatkin/mbv/issues/438) and supersedes the standalone `raise-playback-resume-threshold` OpenSpec change.

## What Changes

- Give queued feed entries the feed identity and playback state needed to address #492's keyed store and survive queue/ctrl serialization.
- Read stored feed-entry state when preparing playback and resume from a qualifying saved position.
- Persist feed position and played state on stop, pause, seek completion, and EOF only; do not add periodic writes.
- Mark a known-runtime feed entry played at EOF or when stopped at or beyond 95% of runtime; played entries store a zero position.
- Extract one shared resume predicate for Emby and feed items and raise the known-runtime threshold from 1% to 6%, inclusive; positive positions with unknown runtime remain resumable.
- Degrade feed state reads and writes to no-ops when the shared feed-entry store is unavailable, without preventing playback.
- Keep feed state local to the shared redb path: do not report feed progress to Emby or create an Emby Session.

## Capabilities

### New Capabilities

- `playback-resume`: Defines the shared 6% resume-eligibility rule used by Emby items and feed entries.

### Modified Capabilities

- `feed-queue-item`: Feed entries gain addressable playback progress, resume from stored state, and persist event-driven completion state through the shipped feed-entry store.

## Impact

- Feed entry and queue progress models in `crates/mbv-core/src/playback_queue_items.rs` and `playback_queue.rs`, including backward-compatible persisted and ctrl wire shapes.
- Resume and completion decisions in `api_types.rs`, `player_run_commands.rs`, `player_run_queue.rs`, and `player_run_events.rs`.
- Feed hydration and lifecycle-event handling in `src/app`, using #492's `SharedClient` get/put operations.
- No ctrl or shared-data protocol version bump, new dependency, Emby API reporting, watched-filter UI, or periodic progress writer.
