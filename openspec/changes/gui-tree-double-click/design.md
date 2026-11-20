# Design

## Context

See proposal.md for the why. The facts that shape the approach:

- Three GUI tree lists paint their own parent rows:
  - Music: `library_list.rs`, `paint_library_row` (artist and album rows).
    A press calls `LibraryState::press_library_row`
    (`window/library/state/rows.rs`), which calls `set_node_open(row, !open)`
    and then `show_detail`. The chevron is a `ChevronRight` in an 18 px span,
    rotated 90 degrees while open.
  - Shows: `library_shows_list.rs`, `paint_shows_node` (series and season
    rows). A press calls `LibraryState::press_shows_row`
    (`window/library/shows.rs`), which calls `toggle_shows_node` and then
    `select_shows_row`. The chevron is `paint_chevron`, which carries the
    `data-library-shows-chevron` mark. A node that is not expandable paints an
    empty 18 px spacer in its place.
  - Playlists: `playlists_panel.rs`, `playlist_row`. A press sets the pick and
    sends `GuiPlaylistsCommand::Open { id }` or `Collapse`. A double-click
    sends `GuiPlaylistsCommand::Action { action: Play }`. The chevron is a
    `ChevronDown` or `ChevronRight` in a 20 px span. The core holds one open
    playlist at a time.
- The row components hold the open state (Music `ui.open` and `ui.closed`,
  Shows open and closed node keys, and the Playlists frame's `open`).
  No core state changes.
- Blitz has no native tooltip: `blitz-dom` 0.3.0-beta.4 reads `title` only as
  the `<title>` element. Blitz supports CSS `:hover` through Stylo. The player
  stage's auto-hide rule in `assets/app.css` already uses it.
- A double-click arrives as click, click, double-click. Today the two clicks
  open and then close a parent row, so a double-click on a parent row leaves it
  as it was.
- The `gui-delineated-rows` change (in progress, no task done) moves the Music
  artist row and the Shows series row onto the shared `LibraryItemRow` part
  (its D1). That part's double-click is `on_play`.

## Goals / Non-Goals

**Goals:**

- One parent-row rule in all three lists: press picks, double-click opens or
  closes, a hover hint, no chevron.
- One shared hint part and one CSS rule, so no list paints its own copy.

**Non-Goals:**

- A keyboard change. Right and Left still open and close the hovered tree node
  (`keys_shared.rs`), and the Playlists keys do not change. The hint names
  only the double-click.
- Another indicator of the open state when the pointer is not on the row. The
  child rows under an open row show the state.
- The TUI.

## Decisions

### D1. Split the press into pick and toggle

- Music: `press_library_row` keeps the track arm. The artist and album arm
  calls only `show_detail`. A new `LibraryState::toggle_library_node(row)`
  calls `set_node_open(row, !open)` for an artist or album row and does
  nothing for other rows. The artist and album button binds `ondoubleclick` to
  it. `set_node_open` keeps the album track load and the filter override, so
  the "node the filter opened" behavior and the track load do not change.
- Shows: `press_shows_row` calls only `select_shows_row`. A series or season
  button binds `ondoubleclick` to `toggle_shows_node(node_id)` when the row is
  expandable. A row that is not expandable takes no double-click.
- Playlists: the playlist row's `onclick` only sets the pick. Its
  `ondoubleclick` sends `Open { id }` when closed and `Collapse` when open, and
  no longer sends `Play`. The choice of the command goes in one small pure
  function, `toggle_command(is_open, id) -> GuiPlaylistsCommand`, so a unit test
  owns the contract without a DOM fixture.

The double-click reads the open state that the row painted. A double-click
always comes after a repaint, because the two clicks no longer change the open
state. So the painted state is the current state.

Alternative: one handler that tells a single click from a double-click with a
timer. Rejected: the pick must not wait, and the GUI has no click timer.

### D2. No chevron, no spacer

The chevron spans and the Shows empty spacer go. The `data-library-shows-chevron`
mark goes with `paint_chevron`. `aria-expanded` stays on Music and Shows parent
rows, and the Playlists playlist row gains it, so tests and accessibility still
read the open state.

The indent comes only from the row inset. Each nested level steps 18 px:

| List | Depth 0 | Depth 1 | Depth 2 |
| --- | --- | --- | --- |
| Music (`row_padding`) | artist 8 px | album 26 px | track 44 px |
| Shows (`row_inset`) | series 8 px | season 26 px | episode 44 px |
| Playlists (`row_style`) | playlist 8 px | item 32 px (no change) | — |

Depth 0 moves from 4 px to 8 px, so the first-level name lines up with the
letter heading and the flat rows (8 px). The other levels move with it.

Alternative: keep the 18 px column empty as an indent. Rejected: it keeps the
TUI's gutter without its purpose.

### D3. One hint part, shown by CSS `:hover`

A new shared part, `components/tree_hint.rs`, exports `TreeHint { open: bool }`.
It paints one span with the class `tree-hint` and the mark
`data-tree-hint`. The text is exactly:

- `open == false`: `Double-click to expand`
- `open == true`: `Double-click to collapse`

The span uses the muted canvas tone (`var(--canvas-muted)`) at 12 px, and
`flex-none` with no wrap.

Each parent row button gets the class `tree-parent`. Its trailing text (Music
album year, Playlists item count, and any trailing text a parent row gets
later) gets the class `tree-trailing`. One rule in `assets/app.css`:

```css
.tree-hint { display: none; }
.tree-parent:hover .tree-hint { display: block; }
.tree-parent:hover .tree-trailing { display: none; }
```

A row with no trailing text shows the hint at the same trailing end: the title
keeps `flex: 1`, so the hint sits at the right edge.

The hint holds no window state and needs no timer: Blitz sets and clears the
hover. A pointer that leaves the window clears the hover chain, as the
auto-hide rule notes.

Alternative: a floating tooltip near the pointer after a delay. Rejected: it
needs pointer tracking and a timer. The user chose the in-row hint.

Alternative: a `title` attribute. Rejected: Blitz shows nothing for it.

### D4. Order with `gui-delineated-rows`

Neither change waits for the other. The change that merges second does the
merge work:

- If this change merges first, `gui-delineated-rows` moves the artist and
  series rows onto `LibraryItemRow` with this change's behavior. For that,
  `LibraryItemRow` gains an optional `tree_open: Option<bool>` prop. With
  `Some(open)`, the part adds `tree-parent`, `aria-expanded`, and `TreeHint`,
  marks its trailing text `tree-trailing`, and its double-click calls a new
  `on_toggle` callback in place of `on_play`.
- If `gui-delineated-rows` merges first, this change adds that prop to
  `LibraryItemRow` in the same way, in place of the edits to the artist and
  series painters.

The tasks below assume this change merges first. The task for
`LibraryItemRow` exists only in the other order, and the implementing agent
picks it from the branch state.

### D5. Tests

Each test names its contract. Existing tests change in place where they
already own the claim:

- Music (`library_list/tests.rs`):
  `a_press_on_an_album_without_its_tracklist_opens_it_and_asks_for_the_tracks`
  becomes the double-click test: `toggle_library_node` opens the album and asks
  for the tracks. A new test, `a_press_on_an_album_picks_it_and_leaves_it_closed`,
  owns the press-picks contract. `a_press_closes_and_reopens_a_node_while_the_filter_narrows_the_rows`
  calls `toggle_library_node`.
- Shows (`library_shows_list/tests.rs`):
  `a_press_on_a_series_node_opens_it_and_selects_it` splits into a press that
  selects and leaves the node closed, and a double-click (`toggle_shows_node`)
  that opens it. `an_expandable_node_paints_its_open_state_as_aria_expanded`
  opens the node through the toggle.
- Playlists (`playlists_panel/tests.rs`): one `#[case]` table for
  `toggle_command` with two cases (closed opens, open collapses).
- Hint (`tree_hint/tests.rs` or the part's test module): one `#[case]` table,
  open and closed, that checks the hint text and the `data-tree-hint` mark.
  The hover itself is CSS and has no unit test.

No test checks the absence of a chevron.

## Risks / Trade-offs

- [A user expects a single press to open a row, as before] → The hover hint
  tells the gesture on every parent row.
- [A closed parent row and a leaf row look the same when the pointer is not on
  them] → The user accepted this. The hint shows on hover, and in Music and
  Shows the depth inset and the row kind (artist weight, series weight) still
  tell the level.
- [A user played playlists with a double-click] → The playlist's Play control
  and the Play entry of its right-click menu still play it.
- [`gui-delineated-rows` edits the same rows] → D4 names the merge work for
  each order.

## Migration Plan

No data or config migration. The open and closed node state is window state
and does not persist.
