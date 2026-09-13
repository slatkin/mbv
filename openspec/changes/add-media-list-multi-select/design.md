## Context

See proposal.md — Why. Relevant current state:

- `MediaList<Target>` is the one owner of rows, cursor and scroll for every
  canonical list (Wide, Inline variants — `Presentation::Grid` was deleted,
  design D13/task 5.8, and no longer exists). It has a single cursor and no
  selection set.
- `MouseGestureState` recognizes `Click`, `DoubleClick`, `RightClick`, `Drag`,
  `Scroll` but discards `event.modifiers`.
- The context menu is built shell-side from one focused item keyed on
  `effective_panel_focus`; there is no existing bulk played/unplayed action
  to extend — Play/Shuffle/Add to Queue reuses the Player's queue append,
  which already takes a `Vec` of items, but Mark Played/Unplayed bulk
  actions must be built from scratch.
- `v` is bound to the queue artwork/visualizer toggle; `Space`/`Esc`
  double-tap fire play/pause/stop via the shell's FallThrough arm.

## Goals / Non-Goals

**Goals:**
- One selection model in the shared owner, uniform across every canonical
  list and breakpoint.
- Mouse and keyboard paths produce the same selection state.

**Non-Goals:**
- Drag-to-select (rubber band) and drag-reordering of a selection.
- Selection spanning multiple lists or tabs.
- Bulk actions outside the context menu (no new direct hotkeys for them).
- A shared cross-Service "replace queue and play" entry point. `App::play_
  items_routed`, the only existing one, stays `EmbyItem`-typed; a non-Emby
  multi-selection (Feeds, Audiobookshelf) only ever offers the actions its
  own existing single-item backend supports (see D10 for Feeds), never
  queue-replacement Play/Shuffle semantics.

## Decisions

**D0 — Slice 1: one context-menu entry path.** Only Home's and Queue's
*keyboard* paths route through the owner's `RowIntent::Context` today; their
mouse right-click paths, like every other destination's, build a
`ShellRequest` directly from the resolved target. Ten-plus bespoke
`ShellRequest` variants exist across Home, Queue, Browser, TV, Music and the
Emby library path — including duplicate keyboard/mouse variants for the same
action (e.g. Home's `HomeContextMenu`/`HomeRowContextMenu`, Queue's
`QueueContextMenu`/`QueueRowContextMenu`) alongside `BrowserContextMenu`,
`BrowserRowContextMenu`, `EmbyLibraryContextMenu`, `MusicAlbumContextMenu`,
`MusicTrackContextMenu`/`MusicTrackContextMenuAt`, `TvHitContextMenu` — and
Feeds has no row menu. Slice 1 routes every destination's right-click and
menu key through `RowLocalInput::{Context, ContextClick}` →
`RowIntent::Context`, translated into one shell request carrying the
resolved target(s) and an optional pointer anchor. Behaviour of existing
menus is preserved (same entries, same anchoring). This is what makes the
selection work land in one place: after slice 1, D3 only widens that single
path from one target to a list. Slice 1 has six-plus call sites to convert,
not one existing path to widen. *Alternative:* thread selection through each
bespoke request — rejected; the parallel edits are the divergence the unify
campaigns removed elsewhere.

The unified request is `ShellRequest::RowContextMenu(ContextMenuTargets,
Option<(u16, u16)>)`, where `ContextMenuTargets` is a destination-tagged
enum, each variant wrapping that destination's *existing* stable-target
type rather than forcing one universal type across every destination
(`MediaList<Target>` already varies `Target` per destination; a single
cross-destination identity would be a bigger, unrelated refactor):
`Home(Vec<HomeRowTarget>)`, `Browser(Vec<String>)`, `Emby(Vec<EmbyItem>)`
(shared by Music, TV, and the Emby library path, which already resolve to
`EmbyItem`), `Queue(Vec<QueueSlotId>)`, `Feeds(Vec<FeedEntry>)`. Browser's
keyboard `.` path currently resolves `EmbyItem` while its mouse path
resolves `String` for the same destination (`BrowserContextMenu` vs
`BrowserRowContextMenu`, `shell.rs:376-464`) — Slice 1 reconciles both to
`String`, Browser's actual `MediaList<Target>` type, since the keyboard
path can resolve the same stable target the mouse path already does.
"One entry path" means one `ShellRequest` variant and one shell-side
dispatch point that branches once on `ContextMenuTargets` to reach D7's
per-item capability derivation — not one `Target` type everywhere.

**D1 — Multi-selection lives in `MediaList<Target>`.** A set of stable targets plus
an anchor target, next to the cursor. Visual mode is not a separate flag: it
is "selection non-empty". *Alternative:* per-destination selection state —
rejected; it would diverge per screen and break the one-owner rule.

**D2 — Stable targets, preserved across refresh.** `set_content` retains
selected targets that still exist and drops the rest; the anchor falls back
to the cursor if its target vanished. Selection clears when the
destination/list *identity* changes — a tab switch or a breakpoint-driven
destination replacement — not on an ordinary TuiRealm active-component
change. Opening the context menu overlay makes it the TuiRealm-active
component (`shell_overlays_modals.rs`'s `OverlayRequest::ContextMenu` calls
`application.active(&id)`); that transition must not clear the selection,
since acting on the selection is the entire point of opening the menu. The
shell keys clearing off the same destination/tab identity
`effective_panel_focus`/tab switching already tracks, never
`Application::focus()`. Selection also clears after any selection action
runs.

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
Bulk effects run synchronously on the UI thread, like every existing
single-item context-menu action today (`execute_context_action` has no
async plumbing to extend, and none is added in this change): each item's
action runs in list order, and a failure on one item is reported the same
way a single-item failure is today (flash/toast) without aborting the rest
of the selection. True background execution is out of scope here — revisit
only if bulk actions over large selections prove slow enough to need it.
Played state uses two bulk-set entries, "Mark Played" and "Mark Unplayed"
(user decision), each forcing one state on every item.

**D8 — Queue is in scope as a canonical list.** Its selection menu always
offers "Remove from Queue" (bulk); the played entries are offered only when
D7's capability intersection allows them. `context_set_played` is
Emby-specific today, so a Queue selection containing a Feed,
Audiobookshelf, or AudiobookshelfBook item (`QueueItemKind`) drops Mark
Played/Unplayed the same way any other list drops an action no selected
item's Service backs, until those Services gain a played-state backend.
Play/Shuffle/Add to Queue are omitted there as they are self-referential.

**D9 — Feeds gets a minimal context menu.** Feeds has no context menu today
(`context_menu_actions.rs` explicitly returns no menu for Feeds and
Audiobookshelf rows alike) and neither does Audiobookshelf; this change adds
one for Feeds only, scoped to what already exists: Play (extend
`ShellRequest::FeedsPlay` to carry the selection's `Vec<FeedEntry>` and play
it in list order) and Add to Queue (same treatment for `FeedsEnqueue`), plus
Mark Played/Unplayed — `FeedEntry.played` is already persisted
(`feed_entry_state.rs`) and drives the existing Played/Unplayed filter, but
there is no setter independent of a playback lifecycle event, so this change
adds one. Feeds has no Remove concept and offers none. Audiobookshelf keeps
no context menu in this change; proposal.md's Impact section previously
implied it already had one to convert — it does not, and none is added
here.

## Risks / Trade-offs

- [Shift+Click silently not delivered in some terminals] → keyboard `V` and
  Ctrl+Click remain; document terminal setting in help.
- [Bulk admission may reject some items (media kind / Service setup)] →
  admission remains shell/Player-owned; rejected items are reported through
  existing enqueue feedback, admitted ones proceed in order.
