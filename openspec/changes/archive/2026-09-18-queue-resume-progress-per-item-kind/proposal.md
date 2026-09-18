## Why

A Queue row showed progress only while it was the playing row, for every item kind except Emby.
The projection read each kind's stored resume position for the playing row and substituted a
literal `0` for the others, so a half-listened podcast or audiobook sitting in the Queue painted no
resume badge. The stored position was never missing: `QueueItem::playback_position_ticks()` covers
all four kinds, and the same `0` stood in the legacy painter this projection replaced — the badge
was never wired for feed and Audiobookshelf rows rather than removed.

## What Changes

- Every Queue row derives its position from the item's own stored resume ticks when it is not the
  playing row; the playing row's live ticks keep winning over them.
- A non-active row's inline trailing progress badge therefore appears for every item kind, which is
  what the projection requirement already described.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `queue-canonical-list`: the projection requirement names the stored resume position as the
  non-active row's source, whatever the item's provider kind.

## Impact

- `src/app/components/queue.rs` — the row field projection, one accessor instead of a per-kind
  literal.
- `src/app/components/queue_component_tests.rs` — a projection test for a non-active feed row.
