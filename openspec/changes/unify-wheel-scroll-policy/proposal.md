## Why

Wheel scrolling is inconsistent across interactive surfaces: different components move by rows, pages, or three-row increments, and several relay their local movement through shell requests that can be dropped or do no useful work. A uniform local policy removes these visible failures while preserving each component's ownership of its own interaction state.

## What Changes

- Standardize accepted wheel gestures to move one row or viewport line in the gesture direction on every scrollable interactive surface.
- Centralize the shared wheel step and direction policy, while leaving cursor and viewport state in the component that owns it.
- Make list surfaces resolve wheel eligibility through their embedded canonical list control; text and irregular-row surfaces use the geometry published by their own painter.
- Remove obsolete shell wheel relays for Home, Browser, and TV, and move Queue scrolling into its component.
- Preserve shell round trips only where a component's resolved selection must trigger an existing shell-owned effect or persistence seam.
- Update the interactive-surface ledger and wheel scenarios to record the uniform policy and its verification.

## Capabilities

### New Capabilities

- None.

### Modified Capabilities

- `mouse-input`: define uniform wheel-scroll direction, single-step behavior, local ownership, geometry resolution, and verification requirements for scrollable interactive surfaces.

## Impact

- Interactive components and canonical media-list controls under `src/app/components/`, especially Browser, Home, Queue, TV, Audiobookshelf, Feeds, Music, and sidebar surfaces.
- Typed shell requests and handlers in `src/app/components/msg/shell.rs` and `src/app/shell_*.rs`.
- Mouse integration/component tests and `docs/architecture/interactive-surface-ledger.md`.
- No new dependencies, configuration, or wire/API changes.
