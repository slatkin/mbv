# Design

## Context

See proposal.md for the why and for the list of lists in scope and out of scope.

The GUI rows that carry a play state paint through four painters, and these are all of them:

| Painter | File | Lists | State type |
| --- | --- | --- | --- |
| Flat row | `window/components/list_row/flat.rs` | Home (Continue watching, Recently added), Podcasts episodes, Feeds entries | `GuiHomeRowStateFrame` (`play: Some(_)`) |
| Tree row (leaf) | `window/components/list_row/tree.rs` | Shows All view episode rows | `GuiHomeRowStateFrame` (`play`, from `episode_state`) |
| Workspace table | `window/components/library_workspace_table.rs` | Shows episodes Workspace, Feeds entries Workspace | `WorkspaceRow::state` |
| Queue row cells | `window/components/queue_panel/row_cells.rs` | The Queue panel | `GuiSemanticState` |

Other painters pass no play state, and this change leaves them as they are: the flat row with `play: None` (Emby, Audiobooks, Music artist, Shows series and Latest/Upcoming, Playlists playlist), the tree row with `play: None` (Music album and track, Shows season, Playlists item), the Workspace table with `state: None` (Music tracklist, Audiobooks chapters), and the search dropdown.

The three Library painters already share one derivation: `list_row::played_title_tone` (and `RowLevel::title_tone`), over `library::episode::is_played_episode`. The queue has its own rule in `row_look` and strikes a played title with `line-through`.

Current facts that shape the approach:

- The Everforest Dark Hard values all exist as `--ef-*` tokens in `assets/app.css` and as `EfToken` in `window/theme.rs`. These are `Fg` (#d3c6aa), `Grey1` (#859289), `Grey2` (#9da9a0), and `Green` (#a7c080). No new token is needed.
- IBM Plex Sans and JetBrains Mono ship at 400, 500, and 600 (`assets/fonts`), so weights 600 and 400 load with no network.
- Icons are Lucide inline SVG through `window/icons.rs`. `Check` already exists.
- Blitz builds an accessibility tree (`blitz-dom` feature `accessibility` is on through `dioxus-native`). It reads `aria-label` and maps `role="img"` to `Role::Image`.
- Blitz 0.3.0-beta.4 parses `filter: grayscale()` into a colour-matrix filter. The GUI renders with `anyrender_vello_hybrid` 0.11.0, and its `convert_filter_effect` returns `None` for `ColorMatrix`, so the filter paints nothing. `opacity` is a layer alpha and paints.
- `Truncate` measures text in Rust with a `TextStyle` (face, size, weight). A painted weight that differs from the measured weight cuts the ellipsis at the wrong place.
- The hover controls hide everything that carries `row-hover-trail` while the row's host is hovered (`assets/app.css`).

## Goals / Non-Goals

**Goals:**

- One derivation and one mark painter that every painter in the table above uses.
- No row moves when its state changes, and every row of a list that carries a play state keeps the same width for the mark.

**Non-Goals:**

- No change to a row that carries no play state.
- No change to row heights, to column widths, to the hover controls, to the selection fill, or to any progress mark (thumb resume run, Workspace progress run, queue ring).
- No new frame field, bridge type, core method, or token.
- No TUI change.

## Decisions

### D1. One shared play look in a new `play_mark` component module

Add `crates/mbv-gui/src/window/components/play_mark.rs` (registered in `components.rs`). It holds:

- `RowPlay { Unplayed, Played }` is the one derivation. `RowPlay::of_row(&GuiHomeRowStateFrame)` maps `Played` to `Played`, and `Ordinary` and `Active { .. }` to `Unplayed`. `RowPlay::of_queue(&GuiSemanticState) -> Option<RowPlay>` maps `Played` to `Played`, `Ordinary` and `Active { .. }` to `Unplayed`, and `NowPlaying { .. }` to `None`.
- The look methods return `EfToken` values through `ef_var` and the weight constants:

  | Method | Unplayed | Played |
  | --- | --- | --- |
  | `title_tone()` | `Fg` | `Grey1` |
  | `title_weight()` | `600.0` | `400.0` |
  | `secondary_tone()` | `Grey2` | `Grey1` |
  | `trail_tone()` | `None` (keep the painter's own tone) | `Some(Grey1)` |
  | `thumb_opacity()` | `None` | `Some(0.35)` |

- Named constants: `PLAY_MARK_SLOT_PX = 12.0` (the slot is the larger of the two marks), `PLAY_DOT_PX = 8.0`, `PLAY_CHECK_PX: u16 = 12`, `PLAY_MARK_GAP_PX = 6.0`, `UNPLAYED_TITLE_WEIGHT = 600.0`, `PLAYED_TITLE_WEIGHT = 400.0`, `PLAYED_THUMB_OPACITY = 0.35`.
- `PlayMark { play: Option<RowPlay> }` is a component. It always paints one `flex-none` span, `PLAY_MARK_SLOT_PX` square, that centres its content and carries `data-play-mark` with the value `unplayed`, `played`, or `none`:
  - `Some(Unplayed)` paints an 8 px circle: a span with `border-radius: 50%` and `background: var(--ef-green)`. The slot carries `role="img"` and `aria-label="Unplayed"`.
  - `Some(Played)` paints Lucide `Check` at size 12 in `var(--ef-grey1)`, with `aria_hidden: true`. The slot carries `role="img"` and `aria-label="Played"`.
  - `None` paints the empty slot with no role and no label. The now-playing queue row and the playing Workspace row use it, so their trailing text stays aligned with the other rows.

Why a new module and not `list_row.rs`: the queue and the Workspace table use the same rule, and `list_row` is the Library and Playlists row vocabulary. A sibling module lets all four painters import it without the queue depending on the Library row parts.

`list_row::played_title_tone` is deleted. Its callers (`RowLevel::title_tone` and the Workspace `title_tone`) use `RowPlay`. `is_played_episode` stays, because the Workspace `state_wire` hook uses it. `RowPlay::of_row` calls it, so the played test is still one function.

Alternative considered: a dot drawn as an inline SVG circle. A rounded span is simpler, and Blitz paints `border-radius` and a solid background. The check uses the codebase's Lucide approach, as the request asks.

### D2. Thumb dims by opacity alone

`paint_thumb` takes the row's `RowPlay` look as an `Option<f32>` opacity. When it is `Some(0.35)`, the box style adds `opacity: 0.35;`. No `filter` is written. See Context: the renderer drops `grayscale()`, and the request says to use opacity alone in that case and to build no workaround.

### D3. The mark sits in a trailing cluster that the hover hides

In each Library painter, a row that carries a play state paints one trailing cluster: a `flex flex-none items-center` span with `gap: 6px`. It holds `PlayMark` and then the existing trailing text span, if the row has trailing text. The cluster takes the `row-hover-trail` class (through `hover_trail_class`) in place of the text span, so the mark and the text hide together while the hover controls show. The text span keeps its hook attribute (`data-library-<attr>-date` or `-duration`), its face, and its size.

A row with no trailing text paints the cluster with the mark alone. The title's `flex-1` box pushes the cluster to the row's trailing end. This meets "Lists without trailing metadata: put the marker at the right edge".

A row with no play state paints exactly the markup it paints now. This keeps every out-of-scope list unchanged.

The mark is placed directly before the trailing text, as the request says. Trailing text is right-aligned, so the text stays aligned. The mark moves with the width of the text (for example, `45:12` and `1:02:11`). That is acceptable, because the request asks for the slot width to be reserved and for the trailing text to stay aligned, and both hold.

### D4. Per-painter application

- **Flat row** (`flat.rs`): with `play: Some(_)`, the title paints `title_tone()` at `title_weight()`, and `Truncate` measures `TextStyle::sans(TREE_ROW_FONT_PX, title_weight())`. The subtitle paints `secondary_tone()`. The trail paints `trail_tone()`, or the existing muted tone. The thumb gets `thumb_opacity()`. The trailing cluster follows D3. With `play: None`, the row keeps `LIBRARY_ROW_TITLE_WEIGHT`, `tree_row_text()`, and the muted subtitle and trail.
- **Tree row** (`tree.rs`): with `play: Some(_)`, the title paints `title_tone()`, and the CSS sets `font-weight: title_weight()` on the `Truncate` box (this overrides the leaf label's 500). `Truncate` measures `TextStyle::mono(LEAF_ROW_FONT_PX, w)` on a leaf and `TextStyle::sans(TREE_ROW_FONT_PX, w)` on a parent. The trail paints `trail_tone()` or its current tone. The trailing cluster follows D3. Only Shows episode leaves pass a play state today. The parent body gets the same rule, so the prop means the same thing at each level. `RowLevel::title_tone` keeps its level tone for `None` and returns `RowPlay`'s tone for `Some`. A new `RowLevel::title_weight(play)` returns the level's present weight for `None`.
- **Workspace table** (`library_workspace_table.rs`): `paint_trailing_cell` wraps the duration in the cluster. The cluster takes `row-hover-trail`, and the duration text keeps its hook. The cluster paints only when the table row carries `state: Some(_)`. A playing row paints `PlayMark { play: None }`, and the others paint `RowPlay::of_row`. The title paints `title_tone()` and `title_weight()` for `Some` state and not playing. The playing row keeps `heading` at 400. A row with `state: None` keeps the text tone at 400. `TypedText` paints each segment in its kind's colour, so a played row's duration paints as a plain text span in `Grey1` in place of `TypedText`. An unplayed row keeps `TypedText` with `repeat: true`. `GRID_COLUMNS` does not change: the 12 px slot, the 6 px gap, and `1:02:11` at 12 px mono (about 51 px) fit in 88 px.
- **Queue row cells** (`row_cells.rs`): `RowLook` drops `strike`, and the title class drops `line-through`. `row_look` keeps the now-playing branch as it is (green fill, green title, 600). For other rows, the title tone is `Fg` while hovered (the existing pointer rule), else `RowPlay::of_queue(..).title_tone()`, and the weight is `title_weight()`. In `RowChrome::Row`, the right column paints `[ring][PlayMark][duration]` with its existing `gap: 6px`. The mark comes from `RowPlay::of_queue`, and `None` gives the empty slot on the now-playing row. `RowChrome::Hover` and `RowChrome::Ghost` paint no mark, because the hover Play control takes the duration's place and the ghost is a drag preview. The right column's colour stays `Grey1` for every row, which is already the played trail tone. `RIGHT_COL_PX` stays 96: ring 14, gap 6, slot 12, gap 6, and `10:02:11` at 12 px mono (about 58 px) total about 96.

### D5. Tests

The tests follow `crates/mbv-gui/AGENTS.md`: headless painted-structure assertions, with no screenshot and no window.

- `play_mark` gets one `rstest` table in its own `play_mark/tests.rs`. It covers `RowPlay::of_row` and `RowPlay::of_queue` over each state variant (`NowPlaying` to `None`, `Active` to `Unplayed`). It also asserts that `PlayMark` paints the `data-play-mark` value, the role, and the `aria-label` for each of the three inputs.
- The existing tone tables are extended in place, and no new parallel test is added. These are `list_row/tests.rs` `a_nested_rows_title_paints_its_play_tone`, `library_workspace_table/tests.rs` `a_rows_play_state_decides_its_title_tone_and_its_resume_run`, and `queue_panel/tests/rows.rs` `the_rows_paint_the_watched_and_now_playing_states`. Each must now assert the new tokens and weight, the mark value, and, for the queue, that no `line-through` remains.
- Add one flat-row case in `list_row/tests.rs`. It asserts that a played row's thumb box carries `opacity: 0.35` and that its mark and trail sit in the one `row-hover-trail` cluster. A `play: None` row paints no `data-play-mark`.

## Risks / Trade-offs

- [The mark moves with the trailing text's width] → The request asks for aligned trailing text and a reserved slot, and both hold. A fixed trail column would be a layout change, which the request rules out.
- [Weight 600 widens the unplayed title, so more titles truncate] → `Truncate` measures at the painted weight (D4), so the ellipsis stays correct. The row height does not change.
- [Library rows switch from `--canvas-text`/`--canvas-muted` to `--ef-fg`/`--ef-grey2`] → The values are the same hexes (`--ef-fg` and `--ef-grey2` alias the canvas tokens). Only the existing tests' expected strings change.
- [A future renderer update starts to paint `grayscale()`] → No action now. Adding the filter later is a one-line change in `paint_thumb`.
- [The Queue's right column is close to full for a `10:02:11` duration with a ring] → The fit is computed in D4. A longer duration does not occur for queue items in practice. A clip would show only on a ten-hour item that is in progress.

## Migration Plan

This is a look change in one crate, with no data or protocol change. To roll back, revert the commits.
