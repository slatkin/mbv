## Why

The Queue sidebar's drag-resized width survives restarts (persisted to `prefs.json`), but the Wide hero list/hero split resets to the default ratio on every launch — even though both are one-column-precision mouse drags the user deliberately set. The split should be equally sticky.

## What Changes

- Wide hero split drag gains a drag-end message that persists the resolved list-pane width to `prefs.json` (mirroring the queue `ResizeColumnLive`/`ResizeColumnEnd` live-then-persist split).
- Startup loads the persisted width; paint-time clamping via the existing `normalize_list_pane_width` handles narrow terminals with no new logic.
- Library refresh (F5) keeps its current reset behavior AND writes the clear through to `prefs.json`, so a refresh-then-restart does not resurrect the pre-refresh width. Queue-focus refresh still leaves the split untouched.
- No keyboard binding moves the split (unchanged); the split remains a single width shared by all Wide hero surfaces (unchanged).

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `mouse-input`: the Wide hero split requirement flips from never-persisted to persisted across restarts, with refresh clearing the persisted value.

## Impact

- `src/app/components/msg/shell.rs` (new `ResizeListPaneEnd` variant), `src/app/components/library_panel/panel.rs` (`DragEnd` arm in `split_gesture_msg`), `src/app/shell_messages.rs` (persist arm), `src/app/input.rs` + `src/app/construct.rs` (prefs save/load), `src/app/library_load_actions.rs` (refresh write-through).
- `openspec/specs/mouse-input/spec.md` scenarios: "never persisted" and "refresh reverts" updated.
- No config format change beyond one `prefs.json` key; no new dependencies.
