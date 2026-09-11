## Why

Media lists can only act on one row at a time. Queueing an album run, clearing
several Continue Watching entries, or marking a season's worth of episodes
played means repeating the same context-menu action row by row. Multi-select
with Explorer-style mouse gestures (Ctrl+Click, Shift+Click) is deeply ingrained
muscle memory, and a vim-style visual mode gives the keyboard an equivalent.

## What Changes

- Slice 1 (prerequisite): every destination opens row context menus through
  the shared list owner's context intent and one shell request, replacing
  the per-destination menu requests (Browser, Emby library, Music album/track,
  TV hit). Feeds gains a row context menu as a result.
- Every canonical media list gains a multi-row selection held by the shared
  list owner, keyed by stable targets.
- Mouse: Ctrl+Click toggles a row into/out of the selection; Shift+Click
  selects the range from the anchor to the clicked row; a plain Click clears
  the selection back to single-row behaviour. mbv requests Shift+Click
  delivery from the terminal (`XTSHIFTESCAPE`) while mouse capture is on.
- Keyboard: `V` enters visual mode anchored at the cursor; movement extends
  the range; `Space` toggles the current row while a selection is active;
  `Esc` clears it.
- A non-empty multi-selection is shown as visual mode in the status row
  (`-- VISUAL (n) --`) with a clickable clear control.
- The context menu (right-click or keyboard menu key) acts on the whole
  selection, offering only actions valid for every selected item:
  Play, Shuffle, Add to Queue, Remove (removable lists only, e.g. Continue
  Watching), Mark Played, Mark Unplayed. Play/Shuffle/Add to Queue use the
  selection in list order.
- Running a selection action clears the selection.
- Mark Played / Mark Unplayed force every selected item into that one state
  (bulk set, never a per-item toggle).

## Capabilities

### New Capabilities

- `media-list-multi-select`: multi-row selection on canonical media lists,
  its mouse and keyboard gestures, visual-mode indication, and the bulk
  context-menu actions over a selection.

### Modified Capabilities

(none — existing single-row behaviour is unchanged when no multi-selection
exists; the new capability layers on top of `canonical-media-lists`,
`mouse-input`, and `context-menu`.)

## Impact

- `src/app/components/media_list/` — selection set + anchor on the shared
  owner; new row-local inputs/intents carrying the resolved target list.
- Slice 1: `src/app/components/msg/shell.rs` context-menu request variants,
  `browser/`, `music_workspace*.rs`, `tv_workspace/`, `feeds.rs`,
  `audiobookshelf_*.rs` menu entry points.
- `src/app/components/mouse/gesture.rs` — click gestures carry modifiers.
- `src/app/key_policy.rs` / router — `V`, and `Space`/`Esc` precedence while a
  selection is active.
- `src/app/context_menu_actions.rs`, `types_context_menu.rs` — selection-aware
  menu builder and multi-item actions (Play/Shuffle/Enqueue/Remove).
- `src/app/mod.rs` — `XTSHIFTESCAPE` set/reset alongside mouse capture.
- Status-row render for the visual-mode indicator.
- Interacts with in-flight `add-configurable-keybinds` (new bindable
  commands).
