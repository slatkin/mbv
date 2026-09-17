## Why

The viewport step lands the dragged selection on the *nearest* selectable row the window shows, which is
always the edge the selection just fell off. A keyboard user stepping up (`Ctrl+e`, `PgUp`) from the
window's last row therefore sees the cursor pinned to the window's bottom line and riding it for every
further press: the cursor stops travelling with the gesture and becomes a passenger of the window. Vim's
`Ctrl+e`/`Ctrl+y` carry the cursor with the scroll, and editors place the caret on the first/last line of
the new page for `PgUp`/`PgDn`; holding the cursor on the edge it is leaving is the outlier, and it is the
behaviour a keyboard-centric list cannot afford.

Reported while reviewing `media-list-viewport-scroll` (archived 2026-09-17); its D3 landing rule is
superseded by this change.

## What Changes

- The drag target is resolved by **step direction** instead of by the side the selection exited: a step
  toward the preceding display row drags the selection to the first selectable row of the new window, and a
  step toward the following row drags it to the last selectable row of the new window. The step still moves
  the selection only when the step would put it outside the window, and the drag still never selects a
  `Heading`/`Spacer` row.
- Because the rule lives in the shared step, the wheel, the list-local chord (`Ctrl+e` / `Ctrl+y`), and the
  page step (`PgUp` / `PgDn`) all inherit it; no per-surface rule is added.
- Nothing else about the step changes: a step whose window still contains the selection moves only the
  window, the window clamps at the content ends, the paint stays read-only, and the cursor path keeps its
  leading-context rule.

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `canonical-media-lists`: the viewport step's drag clause resolves to the leading edge of the step
  direction (and its step/page scenarios state the landing row).
- `mouse-input`: the canonical-list wheel clause names the same landing row, so the wheel and the keyboard
  verbs keep one rule.

## Impact

- `src/app/components/media_list/mod.rs` — `drag_selection_into_window` resolves by step direction.
- `src/app/components/media_list/tests.rs` — the owner drag-at-edge expectations move to the leading edge.
- Per-surface drag-at-edge assertions: `emby_library_content`, `music_content`/`music_interaction`, `queue`,
  `tv_content`, `podcast_content`, `book_content`, `feeds_content`, `inline_search` and their test suites.
- `src/app/tests_tick_integration_*` — the grouped-list viewport-step evidence's expected selection rows.
- Existing specs: `openspec/specs/canonical-media-lists/spec.md`, `openspec/specs/mouse-input/spec.md`.
- No dependency, configuration, wire/API, or on-disk format change.
