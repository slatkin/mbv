## Context

The canonical media-list foundation provides a provider-neutral fixed-row control. Queue is a fixed-row destination but has additional shell-owned controls and effects. This slice composes the control without broadening the foundation or importing hero/inline presentation concerns.

## Decisions

### D1 — Fixed-row child only

Queue embeds `WideMediaList<QueueSlotId>` (or the foundation's equivalent canonical fixed-row child). `InlineMediaBrowser`, Hero-on-left, Inline hero, and responsive Wide/Inline handoff are explicitly out of scope. Queue remains one fixed-row list in each supported Queue presentation.

### D2 — Prepared Queue projection

The parent prepares canonical rows containing stable `QueueSlotId`, title/metadata, semantic ordinary/active state, and optional bounded `progress_percent`. Queue-specific domain data stays shell-side; no ticks, runtime, source, credentials, callbacks, or effects cross the child boundary. Clamp progress at the projection boundary to `0..=100`.

### D3 — Authority stays in Queue parent/shell

Local/Remote scope and its controls, reorder, playback, title, Player/queue authority, persistence, and active-state decisions remain in Queue parent/shell code. Child movement is local; typed intents carry resolved targets/indices when an effect is required. Do not add an App mirror or per-frame writeback.

### D4 — Mouse ownership

The mounted Queue parent subscribes to mouse events and owns `MouseGestureState`. The child records and resolves `HitRegions<QueueSlotId>` while painting. The parent delegates row point resolution and translates it to a semantic request; scope controls and other Queue chrome remain parent-owned. Do not restore the legacy global hit map or duplicate coordinate arithmetic from restore-mouse-support.

### D5 — Verification order

First characterize current Queue rendering and interaction with metadata-bearing Local/Remote, reorder, active-progress, and remote fixtures. Then perform visual correction and obtain explicit user live confirmation at supported widths. Only afterward change/add UI tests. Tests and evidence must prove one painter, child hit geometry, target preservation, and ≤800-line changed source files.

### D6 — Stacking and rollback

Stack on PR #606's feature branch and accepted canonical foundation. Keep this Queue slice independent of Home/Feeds, Music/Audiobookshelf, Feeds Service, and Emby homevideos work, with a distinct rollback boundary. No runtime data migration or dependency change.

## Risks and mitigations

- Cursor/scroll drift on refresh: preserve stable target and clamp only when content changes invalidate it.
- Progress or domain leakage: keep the projection closed and bounded.
- Scope/row hit overlap: child resolves only painted rows; parent resolves scope/chrome.
- Underpainting: source-trace and execution evidence for one Queue body painter per reachable breakpoint.
