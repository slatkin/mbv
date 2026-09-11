## Why

The top of the queue column is a now-playing visual that says nothing when it is empty: with playback
idle the card collapses to zero rows (`src/app/shell_draw.rs:250`), so "nothing is playing" and
"artwork has not arrived" look identical, and the only text that names the playback target is the
queue title's unlabelled hostname (`src/app/render/components/queue.rs:62-150`). Transport lives
somewhere else again: the right-column strip under the tab bar in `both`/`library-only`, and a second
panel in `queue-only` painted by the base frame (`src/app/shell_draw.rs:277-330`) — a panel whose
glyphs have no hit geometry, because `player_area` is deliberately empty in that mode so the mounted
`PlaybackComponent` can neither paint nor resolve a click there (surface ledger row 104,
`src/app/shell_draw.rs:390`).

Converting the queue column into a now-playing sidebar gives every queue-visible layout one player
surface, makes the idle state explicit rather than invisible, and leaves exactly one painter per
layout instead of a mounted painter and a base-frame painter that disagree about who owns the panel.

## What Changes

- The left column's content gains one always-visible header row above the visual slot, on the existing
  chrome surface (`#1e2326`): playback status on the left (`PLAYING` / `PAUSED` / `IDLE`) and
  `on <host>` on the right, where `<host>` is the same target label the queue title row already
  resolves, minus its ` · TRACKING` suffix. The header is the "total state" line: it follows the
  playback target (cast, then connected session, then local), not the queue scope being viewed.
- The playback panel (seekbar, title row, controls) renders in the queue column in **every**
  queue-visible layout — `both` and `queue-only`, wide and narrow — instead of only in `queue-only`.
- Image and panel are hidden and reserve zero rows whenever playback is idle; the header stays.
  **BREAKING (behaviour)**: the `queue-only` exception "a connected transport that is not playing
  keeps its panel" is removed.
- The right-column player strip keeps its code and paints exactly when the queue column is hidden
  (`library-only`, wide or mini view). The right column's content area stops reserving
  `PLAYER_BOX_HEIGHT` whenever the strip is not painted.
- One painter: the mounted `PlaybackComponent` paints whichever panel rect is live this frame — the
  sidebar slot or the strip — through the existing `player_area` handoff, and the base-frame
  `render_player_panel` calls in `render_main` are deleted. Side effect: the mini-view queue-only
  panel's transport glyphs and seekbar become clickable, which they currently are not.
- The idle feed title is displayed wherever the playback panel renders (the strip), instead of
  "the two-panel layout keeps it as today"; the `o` gate follows the panel's presence rather than
  `panel_mode == QueueOnly`.
- Docs: the surface ledger's Root playback row and Queue row, and the `CONTEXT.md` terms for the
  sidebar and the strip.

Out of scope: removing or restyling the queue title bar (it keeps its host/connection text for now;
a later change may delete it), and any change to the queue list's rows, scope pills, or title.

## Capabilities

### New Capabilities

- `now-playing-sidebar`: the queue column's now-playing sidebar — the header row's presence and
  content, the visual slot, the playback panel's placement in every queue-visible layout, the idle
  collapse, and the rule for where the right-column player strip renders instead.

### Modified Capabilities

- `queue-only-playback`: retired. The capability describes a panel that only exists in `queue-only`
  and is now superseded by `now-playing-sidebar`; all six of its requirements are removed and their
  behaviour re-homed (the surviving rules keep their meaning, the queue-only-only framing does not).
- `panel-mode`: `queue-only` renders the panel inside the queue column per the new capability, and
  `library-only` is where the right-column strip renders; the strip's reservation rule moves with it.
- `idle-feed-rotation`: the feed title's host row is the playback panel itself, so display and the
  open-link command now follow the panel's presence instead of "the two-panel layout".

## Impact

- `src/app/render/arrangements/chrome.rs` (`PLAYER_BOX_HEIGHT` reservation, `right_area`, the
  `player_area` gate), `src/app/render/arrangements/queue.rs` (`QueuePanelInputs` gains the header
  row), `src/app/shell_draw.rs` (`render_main`: header row, sidebar slot publication, deletion of the
  queue-only panel painting).
- `src/app/shell_playback.rs` (the live-slot rect and the `narrow_player` meaning),
  `src/app/components/playback.rs` (painting into the sidebar slot), `src/app/shell_run.rs` (draw
  order is already base frame then component, so nothing moves).
- `src/app/render/components/queue.rs` (one extracted host-label resolver shared with the queue
  title), `src/app/render/components/chrome_player.rs` (header painter, if the header is not painted
  inline), `src/app/render/theme/mod.rs` (no new role unless the header must diverge from
  `SURFACE_CHROME`).
- `src/app/action.rs` + `src/app/key_policy.rs` + `src/app/shell.rs` (the idle-feed gate's input),
  and `src/app/render/tests.rs` (the strip-reservation proof moves to the sidebar slot).
- No protocol, daemon, provider, or dependency changes. Interacts with the in-flight
  `unify-wide-hero-content-box-frame` (queue-column width and left-column content area) and
  `add-media-list-multi-select` (Queue row gestures) only by file overlap.
