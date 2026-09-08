## Why

The Wide hero two-pane layout (hero pane and browser/list pane) has a fixed ~40/60 split. After mouse-drag queue-column resize landed (#682), the same standard precise interaction is missing for the wide split: mouse users cannot widen the list to read long titles or narrow it to give the hero artwork more room.

## What Changes

- Make the existing 2-column gap between the Wide hero browser (list) pane and hero pane draggable when a wide surface is active.
- Resize the list pane live at one-column precision while preserving the shared arrangement's minimum pane widths (both panes stay at or above `WIDE_HERO_MIN_PANE_WIDTH`).
- Keep the override in memory only: one session-wide width shared by all wide surfaces, reset to the default split by a screen refresh (F5), clamped (not dropped) on terminal resize, and never persisted to preferences or config.
- Keep click-only interaction, pane row gestures, and the existing visual layout unchanged: the grab zone is the gap columns already present, painted with the backdrop they already show, so no divider, gutter, hover treatment, or new chrome appears.
- Apply no keyboard equivalent; mouse is the only way to move this boundary.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `mouse-input`: Add a requirement for Wide hero split boundary dragging — ownership, gesture recognition, exact-width resolution, clamping, session-only reset semantics, and arbitration — alongside the existing queue-column boundary requirement.

## Impact

- `src/app/render/arrangements/wide_hero.rs` — the one shared split function gains an override input; all wide surfaces (Home, Movies, TV, Home Videos, Music, Audiobookshelf books/podcasts, Feeds) pick it up through this single function.
- Geometry call sites that recompute the split at paint time thread the override mechanically; no per-screen resize logic is added.
- One new shell-mounted boundary Interactive Component (mirror of `QueueBoundaryComponent`), its `ComponentId`, sync, eligibility, and one dispatch arm.
- `App` gains one in-memory override field and a normalize/clamp helper; `refresh_current_view` resets it.
- Focused component and buffer tests, live-tick mouse integration tests, and the interactive-surface ledger.
- No new dependency, no persistence change, no keyboard policy change.
