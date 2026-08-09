## Why

The Emby wire type is named `MediaItem`, but the queue is about to hold a second
kind of item (feed entries). Renaming `MediaItem` → `EmbyItem` now, as its own
atomic change, lets the future `QueueItem::Emby(EmbyItem)` variant read clearly
and keeps the naming churn out of the feature work. Enabling first step for the
RSS feeds decomposition (GitHub #469, parent #461).

## What Changes

- Rename the type `MediaItem` → `EmbyItem` (defined in
  `crates/mbv-core/src/api_types.rs`) and update every reference across the
  workspace (~92 files, ~332 sites).
- **No behavior change.** serde field names are unchanged, so the rename is
  wire-invisible: persisted queue state (`queue_state.json`), shared queue
  documents, and ctrl messages all decode and serialize exactly as before.
- Pure refactor — no new capability, no requirement change.

### Out of scope

- No `QueueItem` enum (that is GitHub #470 / a later change).
- No renderer generalization, no accessor extraction.
- No signature or logic changes. Any line that changes for a reason other than
  the identifier rename does not belong in this change.

## Capabilities

No spec-level behavior changes. This is a pure rename refactor, so the change
opts out of specs via `skip_specs: true` in `.openspec.yaml`. No requirement is
added or modified.

## Impact

- `crates/mbv-core/src/api_types.rs` — the type definition.
- ~92 files across `crates/mbv-core/` and `src/` referencing the type.
- No wire/persistence impact (serde field names unchanged).
- No collision: the in-progress `type-and-naming-cleanup` change does not touch
  `MediaItem` (verified).
