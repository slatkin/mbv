## Why

The visualizer currently consumes queue-list space in a separate area even though the queue card already provides a stable visual slot. Sharing that slot keeps the queue layout intact and makes `v` a direct choice between artwork and visualization.

## What Changes

- Render the visualizer in the queue card's existing artwork rectangle instead of below the queue list or inside the wide queue-only playback area.
- Make `v` switch the queue card between artwork and the visualizer rather than enabling or disabling a separate visualizer area.
- Keep the visualizer slot visible but empty when visualization is selected without supported active playback or captured samples.
- While visualization is selected, use it instead of the bundled placeholder whenever the current queue item has no usable artwork.
- Preserve the existing artwork rectangle dimensions and visualizer capture restrictions.
- Amend ADR 0009 so its `v`-key decision describes artwork/visualizer selection rather than visualizer visibility.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `system-audio-visualizer`: Define `v` as queue-card content selection, empty visualization behavior, and missing-artwork fallback while visualization is selected.
- `queue-only-playback`: Replace the separate wide and bottom-of-queue visualizer placements with the queue artwork slot in every panel mode.

## Impact

- Queue-card rendering and artwork fallback in `src/app/render/card.rs`.
- Queue and queue-only geometry in `src/app/render/mod.rs`.
- Visualizer selection, lifecycle synchronization, preferences, input handling, help text, and focused render tests in `src/app/`.
- ADR 0009 and the two modified OpenSpec capabilities.
- No protocol, daemon, provider API, dependency, or playback-routing changes.
