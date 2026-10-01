# Proposal

## Why

The Queue seekbar rounds playback progress to whole terminal cells, leaving a short bar visibly empty for roughly a minute near the start of a 45-minute episode, and longer for a movie. Rendering fractional cells makes early progress visible and reduces each visual step without changing playback timing.

## What Changes

- Render the Queue playback panel's active seekbar with solid accent blocks and a partial leading block in eighth-cell increments, over a solid `#272e33` track.
- Preserve the bar's existing width, row, elapsed/total labels, spacing, and pointer target.
- Apply the same Queue painter behavior wherever the Queue transport is rendered, independent of media kind or playback target.
- Keep the Library playback panel's thin seekbar and whole-cell rounding unchanged.

### Non-goals

No changes to playback position reporting, polling frequency, interpolation, keyboard seeking, pointer seek resolution, layout, transport visibility, remote APIs, configuration, or dependencies. No unrelated progress indicators or artwork changes.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `queue-playback-panel`: Specify eighth-cell precision and solid fill for the Queue seekbar while preserving its geometry, interaction boundaries, and the Library bar's presentation.

## Impact

The production change is confined to `crates/mbv-render/src/components/chrome_player/title/queue_band.rs`. It reuses the existing Ratatui Gauge with Unicode fill; the shared `seek_fill()` remains unchanged for Library playback. Render-owned regression coverage will establish fractional fill, and existing Queue panel tests will retain ownership of semantic pointer requests. No protocol, persistence, Service, or dependency-version changes are needed.
