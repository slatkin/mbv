# Design

## Context

See `proposal.md` for the why and the user decisions, and `specs/gui-client/spec.md` for the behaviour. The look source is the mockup `~/indent-guides.zip` (`Guides.dc.html`). Its row heights (56 / 30 / 28 px) and its thumb box (64 × 36) are the GUI's current values, so only the guides and the nested insets are new. Its colours are a design-tool grey palette; the GUI maps them to its Everforest canvas tokens below.

Current state that shapes the approach:

- Three GUI trees exist. Music (`components/library_list.rs`) and Shows All (`components/library_shows_list.rs`) paint over `library_virtual_list`: only the rows in view paint, and each row paints alone from its row model (`library/list.rs::LibraryRow`, `library/shows_list.rs::ShowsListRow`). A painter does not see its neighbour rows. The Playlists tree (`components/playlists_panel.rs::tree_column`, `playlist_node`, `items_block`, `rename_field`) is not virtual: each playlist node is a `div` with its row, the rename field while renaming, and the open playlist's items.
- Top-level parents (Music artist, Shows series) paint through the shared `LibraryItemRow` (`components/library_item_row.rs`): row padding 8 px, a 64 px thumb first, `border-box` with a 1 px divider at the bottom. Both virtual lists add 8 px of left padding, so the row box starts at the same x in both lists.
- Nested rows keep their own painters. Today's insets: album / season label at 26 px, track / episode at 44 px, playlist item at 32 px (`docs/architecture/gui-client.md`, tree rows). The season label at 26 px sits left of the thumb's centre (40 px), so a guide under the thumb would cross it.
- A parent row's `open` flag can be true while it shows no child row: an open album whose tracks have not loaded, an open season with no episodes yet, a series with no seasons yet, an open playlist with no items yet.
- Every `button` takes the shared hover veil as a `background-image` (`assets/app.css`), and the picked fills are the button's own background. Both paint under the button's children.
- The in-progress change `gui-playlists-in-queue-column` moves the Playlists tree into the Queue column "as it is"; the tree's row functions keep their shape.

## Goals / Non-Goals

**Goals:**

- One shared guide part that every tree paints, so a guide fix lands in all three trees.
- Guides that stay whole while the virtual lists paint only the rows in view.

**Non-Goals:**

- The mockup's group framing and row highlight (user decision; see proposal).
- The TUI trees (user decision).
- Elbow or "last child" corner marks: the mockup has none; a guide is a straight line.
- Any change to row heights, the row-model build order, the filter, the open state, or the press, double-click, and hint behaviour.

## Decisions

### D1. One shared part paints one guide segment per row

Add `crates/mbv-gui/src/window/components/tree_guide.rs` with:

- `pub enum GuideTone { Top, Nested }`: `Top` is a guide under a top-level parent (artist, series, playlist); `Nested` is a guide under a nested parent (album, season).
- `#[component] pub fn TreeGuide(props: TreeGuideProps) -> Element` with props `x: f32` (the guide's left edge in the row box, px), `tone: GuideTone`, `top: f32` (px from the row's top; `0.0` for a row the guide passes through), and `bottom: f32` (px from the row's bottom; `0.0`, or `-1.0` for a top-level row, see D3).
- It paints one `span` with class `pointer-events-none absolute`, mark `data-tree-guide` (`top` or `nested`), and inline style `left: {x}px; top: {top}px; bottom: {bottom}px; width: 1px; background: {tone var};`.
- It owns the geometry constants (D2) and the parent start offset (D3).

Each row paints the segments that pass through it: a row inside an open subtree paints its ancestors' guides from top to bottom, and an open parent that shows children paints its own guide's start. The segments of adjacent rows meet, so the guide reads as one line.

*Why per-row segments:* the virtual lists paint only the rows in view, so one tall line on a container would need the subtree's height and the scroll window; per-row segments need nothing but the row's own kind and flag, and stay whole under any scroll (spec scenario "A scrolled list keeps the guide whole"). The Playlists tree uses the same per-row model, so the three trees share one rule. *Alternative rejected:* a CSS `border-left` on a nested row — it cannot paint two guides at two x positions, and a top-level row cannot start a guide part-way down.

Guide spans are the last children of their row, before `TreeHint` on a parent row, so existing tests that read a row's first children keep their order. A row that paints a guide carries the `relative` class so it is the span's containing block (the parent rows already carry it through `tree-parent`).

### D2. The geometry: guides hang under the parent's leading element

All x values are px from the row box's left edge, which is the same in Music and Shows (the virtual lists' 8 px padding is outside the row box). Mockup offsets are kept relative to the thumb, so the GUI's 8 px row padding (mockup 10 px) shifts every value by the same 2 px.

| Constant (in `tree_guide.rs`) | Value | Meaning | Mockup |
| --- | --- | --- | --- |
| `TOP_GUIDE_X` | `ROW_PADDING_PX + THUMB_WIDTH_PX / 2` = 40 | a top-level parent's guide: under the thumb's centre | 42 |
| `NESTED_INSET_X` | `TOP_GUIDE_X + 16` = 56 | album and season label inset (was 26) | 58 |
| `NESTED_GUIDE_X` | `NESTED_INSET_X + 4` = 60 | a nested parent's guide: under its label's first letter | 62 |
| `LEAF_INSET_X` | `NESTED_GUIDE_X + 8` = 68 | track and episode inset: the number's box starts here (was 44) | 70 |
| `PLAYLIST_GUIDE_X` | `PLAYLIST_LABEL_X + 4` = 12 | a playlist's guide: under its label's first letter | — |

- `ROW_PADDING_PX: f32 = 8.0` becomes a public constant in `library_item_row.rs`, and the row's style string reads it, so the top-level guide follows the thumb if the padding changes. `THUMB_WIDTH_PX` already exists there.
- `PLAYLIST_LABEL_X: f32 = 8.0` is the playlist row's left padding (`playlists_panel.rs::row_style`, `8 + indent`). The playlist item inset stays 32 px (8 + 24): it already clears the guide at 12 px by 20 px, so the Playlists tree's insets do not change.
- The track and episode rows keep their number boxes, gaps, and fonts; only their left inset moves. Their text moves right by 24 px, and album and season labels by 30 px. Nothing else in the rows moves.

*Why under the leading element:* the mockup hangs the series guide under the thumb and the season guide under the label's first letter; a playlist row has no thumb, so it takes the nested rule (label + 4 px).

### D3. Where a guide starts and stops

- **Start.** An open parent that shows at least one child row paints its own guide from `top = row height − GUIDE_START_INSET_PX` to the row's bottom, with `GUIDE_START_INSET_PX: f32 = 6.0` (mockup: 50 of 56 px on the series row, 24 of 30 px on the season row). On a 36 px playlist row that is 30 px.
- **Top-level row's divider.** `LibraryItemRow` is `border-box` with a 1 px bottom divider, and an absolute child's `bottom: 0` is the padding edge, above the divider. The top-level start segment uses `bottom: -1.0` so the guide crosses the divider with no 1 px gap. Nested and playlist rows have no bottom border and use `0.0`.
- **Through.** Every row inside an open subtree paints each ancestor's guide with `top = 0`, `bottom = 0`. The guide ends where the parent's last subtree row ends; the next row outside the subtree paints no segment for it.
- **Which rows carry which guides** follows from the row kind alone, because a nested row only exists under an open ancestor:

| Row | Through segments | Own start segment |
| --- | --- | --- |
| Music artist, Shows series | — | `Top` at `TOP_GUIDE_X` when it shows children |
| Music album, Shows season | `Top` at `TOP_GUIDE_X` | `Nested` at `NESTED_GUIDE_X` when it shows children |
| Music track, Shows episode | `Top` at `TOP_GUIDE_X`, `Nested` at `NESTED_GUIDE_X` | — |
| Playlist | — | `Top` at `PLAYLIST_GUIDE_X` when it shows items |
| Playlist rename field (under an open playlist that shows items), playlist item | `Top` at `PLAYLIST_GUIDE_X` | — |
| Letter heading, Shows flat row, any flat list row | — | — |

The rename field paints the playlist's guide only when its playlist is the open one and shows items, so the guide has no gap while a rename is open; under a closed playlist it paints none.

### D4. "Shows children" is a row-model flag set at build time

The painter of a parent row cannot see the next row, and `open` alone is wrong while children load. The row models gain one field, set where the builder already knows the kept children:

- `LibraryRow::Artist { shows_children, .. }`: `artist_open && any_kept_albums` (`list.rs::push_letter_and_artist`).
- `LibraryRow::Album { shows_children, .. }`: `album_open && !tracks.is_empty()` (`list.rs::push_albums`).
- `ShowsSeriesRow::shows_children`: `series_open && !kept_seasons.is_empty()` (`shows_list.rs::series_shows_rows`, at the `series_row` call; `series_row` takes the flag as a new argument).
- `ShowsSeasonRow::shows_children`: `open && !kept_episodes.is_empty()`.
- Playlists: no model change; `playlist_node` computes `is_open && frame.open.as_ref().is_some_and(|open| !open.items.is_empty())`.

Each field gets a doc comment naming this change and D4. `LibraryItemRow` gains one prop, `tree_guide: bool` (`#[props(default)]`), doc-commented as "the open top-level parent shows child rows: paint its guide's start (gui-tree-indent-guides D3)"; the Music artist and Shows series painters pass `shows_children`.

*Alternative rejected:* a look-ahead in the virtual list's paint closure — it would couple the shared virtual list to tree semantics.

### D5. The two tones are canvas tokens

Add to `:root` in `crates/mbv-gui/assets/app.css`, after `--canvas-library-control`, each with a comment that names this change:

- `--canvas-tree-guide: #4f5b58;` — the neutral `Top` tone: Everforest bg5 (the same hex as `--ef-bg5`), the nearest palette value to the mockup's `#4f5862`; it reads as a rule, not as content.
- `--canvas-tree-guide-nested: var(--canvas-insertion);` — the `Nested` tone: `#83c092`, the mockup's default accent.

`TreeGuide` reads them through `canvas_var("tree-guide")` and `canvas_var("tree-guide-nested")`. `assets/app.generated.css` carries the `:root` tokens, so it is regenerated with `make gui-css` through the repository's pinned Tailwind standalone CLI v4.3.5 (`tailwindcss-linux-x64`, SHA-256 `e47695cee3f0d90623a91f3939b08bdee610eb5ad99340adff9d4145b284d6cc`, the release the Makefile names). No dependency is added or upgraded.

*Why `--canvas-insertion` and not `--canvas-accent` (`#a7c080`):* the mockup's default accent is `#83c092`, which is the GUI's `--canvas-insertion` / `--canvas-type-label` value.

### D6. Paint order

The guide spans are absolutely positioned children of the row button. The picked fill is the button's background and the hover veil is its `background-image`, so both paint under the spans (spec "A selected episode row", "A hovered track row"). The spans take no `z-index`; `TreeHint` keeps `z-30`, so a parent row's hint card still floats over the next row's guides. The spans carry `pointer-events-none`, so a press on a guide is a press on its row.

## Risks / Trade-offs

- [Narrower nested titles: the inset moves 24–30 px right, so long album, season, track, and episode titles cut earlier] → accepted; the `Truncate` part already measures the box it gets, and the mockup has the same insets.
- [Blitz paints `bottom: -1px` on an absolute child of a `border-box` button differently than a browser] → the divider then shows a 1 px gap under the series and artist start segment; this is a look detail for the user's live check, not a behaviour break.
- [Tests that count a row's children or read its last child] → the spans go last but before `TreeHint`; a test that reads the hint as the last child, or counts children, needs its expectation updated in the painter task that broke it, not a new test (user rule: no new tests unless asked).
- [Merge overlap with `gui-playlists-in-queue-column` in `playlists_panel.rs`] → this change edits only `playlist_node`, `items_block`, `rename_field`, and `row_style`'s callers; whichever change lands second rebases those hunks.

## Migration Plan

None: a paint-only change in the GUI window with no stored state. Rollback is a revert.
