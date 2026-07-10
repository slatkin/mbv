# Movie Power View Banner (Issue #114)

## Problem

Power view's Movies tab currently requires drilling in (Enter on a leaf `Movie` row)
to see the movie's overview, director, tech info, and poster. This replaces the whole
right panel with a scrollable detail view, hiding the list, and repurposes `Enter` away
from its standard "play the selected item" meaning that every other list type in power
view uses.

Issue #114 asks for a "banner mode" for movie browsing: a way to see the selected
item's rich detail without leaving list context, patterned after list+preview UIs in
other apps.

## Goal

Combine the list and the drilled-down movie view: whichever movie is under the list
cursor renders a compact detail "banner" above the list, live, with no explicit
drill-in step. `Enter` reverts to the standard behavior (play the selected item). A
"Show More" hotkey (`Alt+M`) still gives access to the full, today's-style detail view
when wanted.

## Scope

Movies library only. TV/episodes, music, home videos, and any other library type in
power view are unaffected — they keep their current list/drill-down behavior and code
paths untouched.

Within a movies library, this only applies when the list cursor sits on a leaf `Movie`
item. Folder/genre/box-set levels (non-`Movie` items) render exactly as they do today:
plain list, no banner, and `Alt+M`/`Enter` behave as they currently do for a folder
row (Enter drills into the folder).

## Layout

The right panel (movies list area, `lib_area` in `render_power_library`) splits
vertically only when the cursor is on a leaf `Movie` item:

```
┌─────────────────────────────────────┐
│  Banner (fixed ~9 rows)              │  ← poster (8 rows + 1 shadow row),
│                                       │    title, meta, tech info, overview
├───────────────────────────────────── │  ← 1-row muted divider rule
│  List (remaining rows)               │  ← today's render_power_list, unchanged,
│  ...                                 │    just given a shorter area
└─────────────────────────────────────┘
```

- Banner height is fixed (not content-dependent) so the list below doesn't reflow as
  the cursor moves between movies with differently-sized posters/overviews.
- The banner recomputes from whatever item is currently under the list cursor, every
  frame. There is no pinning step — moving the cursor immediately updates the banner,
  the same way `render_power_card`'s left-column art already tracks the cursor for
  other library types.
- When the cursor is on a folder item, or the list is empty, no split happens — the
  list fills the full panel, identical to today.

## Content changes to the movie detail rendering (`detail.rs`)

These changes apply to the "individual movie view" in general, so they affect both
the new compact banner and the existing full detail view reused for "Show More":

1. **Remove the "Press [ENTER] to play" line and its two blank spacer rows.** Replace
   with: a single "Playing" line (green, no padding) when this movie is the one
   actively playing; nothing at all when it isn't (no reserved blank rows).
2. **Overview truncation split by mode:**
   - Compact banner: overview goes through `trunc_overview` (the existing 400-char /
     URL-stripped truncation already used for home-video descriptions), word-wrapped,
     and then silently clipped to whatever rows remain in the fixed banner height — no
     scrollbar in compact mode.
   - Expanded ("Show More"): unabridged overview, scrollable, as today.
3. **Director line:** hidden entirely in the compact banner. Only shown in the
   expanded view (as today).
4. **Poster size:** compact banner uses an 8-row-tall poster (down from the existing
   12). Expanded view keeps the existing 12-row poster, unchanged.

## Expanded ("Show More") state

- Triggered by `Alt+M`, only when the right (library) panel has focus and the cursor
  is on a leaf `Movie`.
- Reuses today's full detail view (`render_power_detail`) practically as-is: full
  overview, director shown, 12-row poster, scrollable — minus the play-status block
  change from item 1 above. Takes over the whole right panel; the list is hidden,
  exactly like today's drill-in view.
- Dismiss via `Alt+M` (toggle off), `Backspace`, or `Esc` — the same dismissal keys
  detail mode already supports.
- The list cursor cannot move while expanded (arrow keys are already consumed for
  overview scrolling in detail mode, as today), so there's no separate "collapse on
  cursor move" behavior to build — you always dismiss before selecting a different
  movie.

## Input handling changes (`input.rs`)

- Remove the `Movie`-specific `Enter` → open-detail interception. `Enter` always plays
  the selected item now, movies included, matching every other list type.
- Add `Alt+M` handling in the existing `PowerFocus::Left` block: when not already in
  detail mode and the selected item is a leaf `Movie`, sets `power_detail_item` to
  that item (opens the expanded view). When already in detail mode, `Alt+M` clears
  `power_detail_item` (closes it), same as `Backspace`/`Esc`.
- All existing detail-mode key handling (`Up`/`Down`/`PageUp`/`PageDown` scroll,
  `Backspace`/`Esc` dismiss, `Enter` plays) is unchanged — only how you *enter* that
  mode changes (`Alt+M` instead of `Enter`).

## Unaffected / non-goals

- `power_detail_item` / `power_detail_scroll` fields, session save/restore
  (`actions.rs`), and `card.rs`'s "is a detail pinned?" image-selection branch are all
  unchanged — they already key off `power_detail_item.is_some()`, which still means
  "expanded view is open," just entered via a different key.
- No changes to other library types' rendering (`home.rs`, `episode.rs`, `album.rs`,
  `music.rs`) or their input handling.
- No new persisted state: the compact-vs-expanded choice is not saved across
  restarts; sessions always resume with `power_detail_item: None` (banner mode),
  same as most existing session-restore behavior for this field.

## Open implementation questions (for the plan, not blocking design approval)

- Whether the banner rendering is a new function in `detail.rs` (parameterized by
  `compact: bool`) or a fully separate function that shares helpers with
  `render_power_detail`. Left to the implementation plan.
- Whether a clipped (height-truncated) overview in the banner gets a trailing "…" to
  signal there's more (on top of `trunc_overview`'s own char-based ellipsis). Proposed
  default: yes, append one if the wrapped line count exceeds available banner rows.
