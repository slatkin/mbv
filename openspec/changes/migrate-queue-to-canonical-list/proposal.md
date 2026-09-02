## Why

Queue still owns destination-specific fixed-row mechanics instead of composing the canonical list control. This slice makes Queue the next bounded consumer of the canonical fixed-row vocabulary while preserving Queue's playback and persistence authority.

## What Changes

- Characterize the accepted Queue output and interactions, then compose `WideMediaList<QueueSlotId>` directly inside the mounted Queue parent.
- Project Queue rows as provider-neutral selectable items with stable `QueueSlotId` targets, metadata, active state, and bounded `progress_percent` (`0..=100`) as presentation data only.
- Keep Local/Remote scope, scope controls, reorder, playback, title, Player/queue authority, persistence, and active state in the Queue parent/shell.
- Preserve visual output after live user verification, then update focused tests; include one-painter and file-size evidence.

## Scope

Queue only. Hero-on-left, InlineMediaBrowser, Inline hero, responsive Wide/Inline handoff, Feeds, and Audiobookshelf are not applicable. Mouse (subscription, gestures, `HitRegions<QueueSlotId>`) is out of scope and owned by `restore-mouse-support` (#638); Queue's existing `QueueHitRegion` path stays wired and untouched. The work stacks on PR #606's feature branch and remains independently reversible from sibling slices.

## Impact

Interactive Queue component, canonical-list composition, Queue render/geometry tests, and planning evidence. No Service, provider, protocol, daemon, or dependency changes.
