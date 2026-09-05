## Context

The canonical media-list foundation provides a provider-neutral fixed-row control. Queue is a fixed-row destination but has additional shell-owned controls and effects. This slice composes the control without broadening the foundation or importing hero/inline presentation concerns.

## Decisions

### D1 — Fixed-row child only

Queue embeds `WideMediaList<QueueSlotId>` directly. Queue SHALL NOT use `InlineMediaBrowser`, Hero-on-left, Inline hero, or responsive Wide/Inline handoff. Queue remains one fixed-row list in each supported Queue presentation.

### D2 — Prepared Queue projection

The parent prepares canonical rows containing stable `QueueSlotId`, title/metadata, semantic ordinary/active state, and optional bounded `progress_percent`. Queue-specific domain data stays shell-side; no ticks, runtime, source, credentials, callbacks, or effects cross the child boundary. Clamp progress at the projection boundary to `0..=100`.

### D3 — Authority stays in Queue parent/shell

Local/Remote scope and its controls, reorder, playback, title, Player/queue authority, persistence, and active-state decisions remain in Queue parent/shell code. Child movement is local; every slot-targeted effect uses a stable `QueueSlotId`. A destination position is permitted only for reorder and must be resolved against that same canonical queue. Do not add an App mirror or per-frame writeback.

### D4 — Mouse is out of scope

Mouse is out of scope for this slice. Queue's existing hit-region path stays wired and untouched. `restore-mouse-support` (#638), landing after this slice, adds `HitRegions<QueueSlotId>` to the embedded `WideMediaList` and migrates Queue's row hits. This slice adds no mouse subscription, `MouseGestureState`, or parent-to-child point delegation.

### D5 — Continuous verification and acceptance

Implementation, focused tests and automated gates, review, and acceptance form one uninterrupted slice without a pre-test visual-approval checkpoint. Tests and evidence prove one painter, child row geometry, target preservation, and ≤800-line changed source files. Live review covers supported widths; defects found there are fixed as bugs before rerunning affected tests and gates.

### D6 — Stacking and rollback

At implementation start, record the accepted canonical-foundation merge SHA, then stack on that foundation and PR #606's feature branch. Keep this Queue slice independent of Home/Feeds, Music/Audiobookshelf, Feeds Service, and Emby homevideos work, with a distinct rollback boundary. No runtime data migration or dependency change.

## Risks and mitigations

- Cursor/scroll drift on refresh: preserve stable target and clamp only when content changes invalidate it.
- Progress or domain leakage: keep the projection closed and bounded.
- Underpainting: source-trace and execution evidence for one Queue body painter per reachable breakpoint.
