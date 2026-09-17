# Design: Inline Search results on the canonical media-list rows

## Context

Today the Library panel paints an open Inline Search session through a
bespoke render path: the `ListSlot::Search` arm of
`paint_browser_pane` (`src/app/components/library_panel/wide.rs`, the one
paint site shared by Wide and Narrow) materializes the scored order into
`EmbyItem`s and calls `render_inline_search`
(`src/app/render/components/inline_search.rs`), which paints the bar via
`render_search_box` and the rows via the legacy painter
`render_generic_movies_home_video_rows_with_ctx`
(`src/app/render/components/list.rs` → `list_letter_groups.rs` |
`render/components/media_list/plain_rows.rs`).

The search control (`src/app/components/inline_search.rs`) duplicates
row-flow state the canonical owner already provides: `order` (corpus index +
score pairs), `cursor`, `scroll`, `layout`, its own
`move_cursor`/`page_size`/`select_row_at` logic, and a raw-event mouse path
(`handle_mouse`, `InlineSearchMouse`, `MouseGestureState`, `left_press`)
that has no live caller — owners receive the panel-normalized
`MediaListSurfaceInput` and translate it per-owner in
`handle_search_pointer`.

Constraints (see proposal + specs deltas):

- One owner, one painter per surface per breakpoint
  (`canonical-media-lists`): the search bar is selector-row chrome; the
  result rows are a media-list flow in the list box.
- Destination components own local row state; the shell projects content;
  `MediaListCarrier<Target>` never switches presentation owners.
- The inline search behaviors added this cycle stay: deferred corpus load
  on first keystroke, 300 ms debounce, empty query shows nothing, Series
  activation navigates + opens the workspace/overlay.

## Goals / Non-Goals

Goals:

- Result rows flow through `MediaListCarrier<String>` with stable item-id
  targets and the canonical fixed-row presentation — selection bar, zebra,
  theme roles, scrollbar — with content parity (labels, folder suffixes,
  year, placeholders).
- Search-specific state and effects stay in the search control; row-flow
  state moves to the carrier.
- Pointer behavior reaches result rows through the standard
  `delegate_operation` intent path so the follow-up context-menu/shortcut
  work (#720's successor) builds on the same machinery as other lists.
- Delete the legacy painters and the dead search plumbing.

Non-Goals:

- Any new action coverage (context-menu chord, enqueue-all, shortcuts) —
  the agreed follow-up, not this change.
- Multi-select/visual mode on search rows (the query consumes printable
  keys by contract).
- Semantic played-state dimming of search rows (legacy rows never dimmed;
  parity keeps `Ordinary` — enabling it is a one-field change later).
- Moving the search bar or changing its painter (`render_search_box`
  survives as-is).
- Two-column search results (the legacy call already passed `columns: 1`;
  the canonical presentation is fixed one-column).

## Decisions

### D1 — The carrier lives inside the search control

`InlineSearch` gains `results: MediaListCarrier<String>` (target = item
id) and keeps `query`, `pool`, scored `order`, `loading`, `active`, and
the debounce `deadline`. `cursor`, `scroll`, `layout`,
`mouse_gestures`, `left_press`, `move_cursor`, `page_size`,
`select_row_at`, `handle_mouse`, and `InlineSearchMouse` are deleted.

Alternative considered — the destination's existing browser carrier
switches to search rows while search is open — rejected: it mixes two
logical row flows in one owner (violates the one-flow-per-owner contract)
and loses the destination's retained browse selection.

### D2 — Row materialization at re-score/pool time, not paint time

`set_pool` and the debounce's re-score build
`Vec<MediaListRow<String>>` from the scored order and call
`results.set_content(rows)`:

- Item row: `target` = item id; `primary` = the composed label parity of
  `plain_rows.rs` (folders: `Name · N items` for a `Folder` with
  `total_count > 0`, `Name [unplayed]` for a non-`Series` folder with
  unplayed items, otherwise the display label — an indexed album uses its
  display label via the pool's existing substitution); `trailing` =
  `Year(production_year)` for playable leaves with a year; `duration` =
  None; `kind` = `Collection` for folders else `Media`;
  `semantic_state` = `Ordinary` (parity).
- No `Heading`/`Spacer` rows (flat by construction).
- Selection: a re-score (query changed) calls `results.select_first()`
  explicitly (spec: reset to first); a pool refresh with unchanged query
  relies on the carrier's native stable-target preservation
  (`set_content` preserves the selected target when present), replacing
  the manual `selected_item`-preservation code in `set_pool`.
- Empty query / empty order ⇒ zero rows; the panel paints the placeholder
  (D4).

### D3 — Panel slot keeps `ListSlot::Search`; painting reuses the `PanelList` surface

`ListSlot::Search(&mut InlineSearch)` remains the slot type (the panel
must know a search is active to swap the Selector row, and the bar+results
form one session). `InlineSearch` implements the panel's object-safe
`PanelList` trait by forwarding each method to its embedded carrier —
the same surface `ListSlot::Media` drives — so the paint arm composes:

1. `render_search_box(f, pills_area, query, loading)` (bar, unchanged);
2. zero rows ⇒ `render_placeholder` with `" Loading…"` / `" (empty)"`
   (string parity with the legacy empty branch);
3. otherwise the exact `ListSlot::Media` driving sequence:
   `sync_viewport`, `set_paint_policy(PanelListPaintPolicy::Wide {
   focused })` (MainContentBox zebra — the same box the results live in),
   `set_geometry`, `view`, and `selected_row_rect` for the panel's
   context-anchor geometry.

Alternative considered — a new slot variant carrying
`(&mut InlineSearch, &mut MediaListCarrier)` — rejected: two payloads for
one session invites divergence; a delegating trait impl is 7 one-line
forwards.

### D4 — Keyboard and pointer route through the carrier

Keyboard: `InlineSearch::handle_key` keeps query editing, Enter/Esc/
Backspace, and the debounce; Up/Down/PageUp/PageDown/Home/End become
`results.delegate_operation(MediaListSurfaceInput::{Move,Page,First,Last})`
conversions (as owners do), replacing `move_cursor`/`page_size`.
`selected_item()` resolves `results.selected_target()` → pool lookup
through the stored order.

Pointer: each owner's `handle_search_pointer` collapses to
`delegate_operation` over the embedded carrier plus intent translation:
`RowIntent::Context(target)` → the same `RowContextMenu` message the owner
emits today (Emby owner: `Browser(vec![id])`; TV/Music: `Emby(vec![item])`),
double-click select+activate → `InlineSearchActivate`, wheel →
`MouseClaimed`. The three bespoke `select_row_at_point` bodies and the
per-owner gesture translation die with them.

### D5 — Transitions and restore

`restore_query` re-scores and selects first; `restore_target` maps to
`results.select_target(id)` + `set_scroll(row_offset)`. Wide↔Narrow
transitions keep one session and one carrier across geometry changes
(carrier-native clamping), so the "open search survives transitions"
requirement needs no new mechanism.

### D6 — TV's grouping gate moves off the render ctx

`LibraryListRenderCtx`'s search fields (`search_query`, `search_loading`,
`with_search`, `is_search_active`) have exactly two consumers on the
search path: the deleted `render_inline_search` and
`TvContent::set_content`'s grouped computation
(`!context.list.is_search_active()`), plus the Music push's `with_search`
decoration. After this change: TV reads its own session
(`self.inline_search.is_active()`) — the owner owns the session, not the
ctx — the Music `with_search` decoration is deleted, and the ctx fields
die with the legacy painter. `LibraryListRenderCtx` itself stays (TV/Music
wide contexts, detail contexts use it).

### D7 — Deletions

- `src/app/render/components/list.rs`,
  `src/app/render/components/list_letter_groups.rs`,
  `src/app/render/components/media_list/plain_rows.rs` (+ the
  `render_plain_rows` re-export). `render/components/media_list/{row,wide}.rs`
  are the live canonical painters and stay.
- `src/app/render/components/inline_search.rs` (`render_inline_search`);
  the bar call site becomes `render_search_box` in `paint_browser_pane`.
- The dead raw-mouse path in the search control (D1) and the ctx search
  fields (D6); re-exports in `src/app/render/mod.rs` trimmed.

## Risks / Trade-offs

- [Selected-row styling visibly changes] → Intended (the issue's point):
  full-bleed legacy style becomes the canonical bar + zebra. Pinned by
  rendered tests against `palette::SELECTED_ROW_BG` like
  `panel_list_tests`.
- [Carrier `set_content` preserves target by id; duplicate ids in a
  malformed payload could collapse onto the first] → The scored order is
  unique per corpus item; the carrier's ordinal resolution keeps two
  visible rows distinct (same guarantee the browser lists rely on).
- [Placeholder strings drift] → Parity strings asserted in tests
  (`" Loading…"` / `" (empty)"`).
- [Tick-integration search tests break on cursor-implementation churn] →
  They drive real keys through `Application::tick()`; they must keep
  passing unchanged except where they poke control internals (the few
  `set_pool`/`layout_mut` seeds switch to carrier seeding).
- [Music/TV `with_search`/`is_search_active` removals touch live pushes]
  → Behavior-neutral gates audited per D6; covered by the existing
  Music/TV search integration tests.

## Migration Plan

Single branch, no staged rollout (a TUI binary): migrate the control
(D1/D2), then the paint arm (D3), then owners (D4), then deletions (D6/D7),
keeping `cargo nextest run -p mbv` green at each step. Rollback = revert
the change; no persisted state or wire format is touched.

## Open Questions

None blocking. Assumptions recorded for review (no user decision required
to proceed): placeholder text parity (D3), `Ordinary` semantic state for
result rows (D2), deletion scope (D7), and the already-dropped
press-in-bar/release-on-row gesture (documented deviation since task
6.1; the panel-normalized input cannot carry raw presses and no caller
remains).
