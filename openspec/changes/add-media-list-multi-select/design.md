## Context

See proposal.md — Why. Relevant current state:

- `MediaList<Target>` is the one owner of rows, cursor and scroll for every
  canonical list (Wide, Inline, Grid variants). It has a single cursor and no
  selection set.
- `MouseGestureState` recognizes `Click`, `DoubleClick`, `RightClick`, `Drag`,
  `Scroll` but discards `event.modifiers`.
- The context menu is built shell-side from one focused item keyed on
  `effective_panel_focus`. Bulk actions already exist for podcasts
  (`MarkItemsPlayed(Vec<_>)`, `MarkItemsUnplayed(Vec<_>)`), and the Player's
  queue append already takes a `Vec` of items.
- `v` is bound to the queue artwork/visualizer toggle; `Space` is global
  `TogglePlayPause` (FallThrough).

## Goals / Non-Goals

**Goals:**
- One selection model in the shared owner, uniform across every canonical
  list and breakpoint.
- Mouse and keyboard paths produce the same selection state.

**Non-Goals:**
- Drag-to-select (rubber band) and drag-reordering of a selection.
- Selection spanning multiple lists or tabs.
- Bulk actions outside the context menu (no new direct hotkeys for them).

## Decisions

**D0 — Slice 1: one context-menu entry path.** Today only Home and Queue open
row menus via the owner's `RowIntent::Context`; Browser, TV, Music and the
Emby library paths emit bespoke `ShellRequest` variants
(`BrowserContextMenu`, `BrowserRowContextMenu`, `EmbyLibraryContextMenu`,
`MusicAlbumContextMenu`, `MusicTrackContextMenu(At)`, `TvHitContextMenu`),
and Feeds has no row menu. Slice 1 routes every destination's right-click
and menu key through `RowLocalInput::{Context, ContextClick}` →
`RowIntent::Context`, translated into one shell request carrying the
resolved target(s) and an optional pointer anchor. Behaviour of existing
menus is preserved (same entries, same anchoring). This is what makes the
selection work land in one place: after slice 1, D3 only widens that single
path from one target to a list. *Alternative:* thread selection through
each bespoke request — rejected; six parallel edits is the divergence the
unify campaigns removed elsewhere.

**D1 — Multi-selection lives in `MediaList<Target>`.** A set of stable targets plus
an anchor target, next to the cursor. Visual mode is not a separate flag: it
is "selection non-empty". *Alternative:* per-destination selection state —
rejected; it would diverge per screen and break the one-owner rule.

**D2 — Stable targets, preserved across refresh.** `set_content` retains
selected targets that still exist and drops the rest; the anchor falls back
to the cursor if its target vanished. Selection clears on tab/destination
switch away (focus loss of the list) and after any selection action runs.

**D3 — Resolved list crosses the boundary.** The owner emits a new
`RowIntent::ContextSelection(Vec<Target>)` in list order; the shell never
recomputes the selection. Right-click on a row outside the selection clears
the selection and falls back to the single-row menu (Explorer behaviour).

**D4 — Gestures carry modifiers.** A click with Ctrl or Shift is never
promoted to `DoubleClick` and never starts a drag grab (Queue). `Click` gains a modifier field (Ctrl,
Shift); `RowLocalInput` gains `ToggleClick(Position)` and
`RangeClick(Position)`. Following Explorer, the first Ctrl+Click adds both
the prior cursor row and the clicked row to the selection.

**D5 — `XTSHIFTESCAPE`.** Emit `CSI > 1 s` after `EnableMouseCapture` and
`CSI > 0 s` on teardown/suspend. Terminals that ignore it need their own
setting (Ghostty `mouse-shift-capture`, kitty `terminal_select_modifiers`);
accepted — mbv has no text worth native-selecting. Ctrl+Click over a
terminal-detected hyperlink stays the terminal's; accepted (one link).

**D6 — Keyboard: `V` enters Visual mode.** `v` is taken by the visualizer
toggle; `V` matches vim's linewise visual mode. In Visual mode the focused
list consumes `Space` (toggle row) and `Esc` (exit Visual mode) outright:
they neither fire nor arm the shell's double-tap play/pause/stop, and they
override list-local Space meanings (e.g. ABS FocusOrPlay). Once Visual mode
exits, every key behaves as before.

**D7 — Menu shows only actions valid for all selected items.** Capability is
derived per item (playable, queue-admissible, removable-from-this-list,
supports played state) and intersected. An action with no backend for a
Service is simply not offered; Audiobookshelf and Feeds use the same
target-keyed action path so backends can be added later without menu work.
Bulk effects run off the UI thread and report partial failure. Played state uses two bulk-set
entries, "Mark Played" and "Mark Unplayed" (user decision), each forcing
one state on every item; matches the podcast bulk precedent.

**D8 — Queue is in scope as a canonical list.** Its selection menu offers
"Remove from Queue" (bulk) and the played entries; Play/Shuffle/Add to Queue
are omitted there as they are self-referential.

## Risks / Trade-offs

- [Shift+Click silently not delivered in some terminals] → keyboard `V` and
  Ctrl+Click remain; document terminal setting in help.
- [Bulk admission may reject some items (media kind / Service setup)] →
  admission remains shell/Player-owned; rejected items are reported through
  existing enqueue feedback, admitted ones proceed in order.
