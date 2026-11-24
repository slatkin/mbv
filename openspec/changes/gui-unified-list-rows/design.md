# Design

## Context

See proposal.md (Why). The painters today, in `crates/mbv-gui/src/window/`:

| Row | Painter today |
| --- | --- |
| Home, Podcasts, Feeds, Emby, Audiobooks, Shows New/Upcoming, Shows series, Music artist | `LibraryItemRow` with caller-passed `title_color`, `background`, `progress`, `played_mark` |
| Music album, track | hand-built `button` in `library_list.rs` |
| Shows season, episode | hand-built `button` in `library_shows_list.rs` |
| Playlists playlist, item | hand-built `button` in `playlists_panel.rs` |

The play-state rules already have one home: `window/library/episode.rs` (`is_played_episode`, `episode_progress`), which the Feeds and Shows Workspace heroes also use. The row painters restate it in `name_color`, `row_name_color`, and `row_progress`.

The rows that carry a play state today: Home (both sections), Podcasts, Feeds, and Shows episodes. Emby, Audiobooks, Shows flat and series, Music, and Playlists rows carry none. This change adds none.

## Goals / Non-Goals

**Goals:**

- Two row parts paint every Library and Playlists row. No list paints a row.
- One look for each row level in every list, taken from the Music list.
- A list passes no look value, so a restyle in a list does not compile.
- One derivation for each shared rule.

**Non-Goals:**

- New play state, marks, or Service data. Projection and frame changes.
- The Queue rows, the Workspace tables, the Hero tracklist, and the TUI.

## Decisions

### D1. One module owns row painting

`window/components/list_row.rs` holds the shared rules (D4), the level constants, and the constants that move from `library_item_row.rs`. `list_row/flat.rs` holds `FlatRow`, `list_row/tree.rs` holds `TreeRow`, and `list_row/thumb.rs` holds the thumb painter. `library_item_row.rs` is deleted.

### D2. `FlatRow`: the top-level row

Props: `attr`, `kind`, `id`, `title`, `subtitle`, `trail`, `trail_part`, `thumb: Option<LibraryRowThumb>`, `play: Option<GuiHomeRowStateFrame>`, `selected`, `tree_open`, `tree_guide`, `on_press`, `on_play`, `on_toggle`, `on_context`. Removed: `height`, `title_color`, `background`, `progress`, `played_mark`.

The look is the Music artist row's: height `LIBRARY_ROW_HEIGHT`, padding `ROW_PADDING_PX`, gap 12, the 1 px divider (none under an open parent that shows children), square corners, the 64 × 36 thumb when `thumb` is `Some`, the title in sans `TREE_ROW_FONT_PX` at `LIBRARY_ROW_TITLE_WEIGHT`, the subtitle and trailing text in muted 12. A selected row paints the `library-raise` fill. A tree parent paints `tree-parent`, `aria-expanded`, `TreeHint`, and its guide start at `TOP_GUIDE_X`.

Users: Home, Podcasts, Feeds, Emby, Audiobooks, Shows New and Upcoming, Shows series, Music artist, and Playlists playlist. A playlist has no art, so it passes `thumb: None` and its count as the trailing text.

### D3. `TreeRow`: the nested parent and leaf rows

Props: `attr`, `kind`, `id`, `level: TreeLevel`, `depth: TreeDepth`, `title`, `number: Option<String>`, `trail: Option<TreeTrail>`, `play: Option<GuiHomeRowStateFrame>`, `selected`, `open: Option<bool>` (a parent that opens), `shows_children`, `tail`, `on_press`, `on_play`, `on_toggle`, `on_context`.

- `TreeLevel::Parent` is the Music album look: height `NODE_ROW_HEIGHT`, sans `TREE_ROW_FONT_PX` at weight 400 (painted and measured at 400), text tone, trailing text as `TypedText` Date (repeat opacity unless selected, muted when selected), `tree-trailing`, `TreeHint` and `tree-parent` when `open` is `Some`, and its guide start when it shows children.
- `TreeLevel::Leaf` is the Music track look: height `TRACK_ROW_HEIGHT`, the leaf face (`leaf_label_css`, `leaf_row_text`), muted tone, the 18 px right-aligned number cell in `canvas-ordinal` when `number` is `Some`, the trailing text in muted mono 12.
- Both levels: gap 8, the `tree-subtree` fill, the `library-raise` fill and a 4 px radius while selected (0 otherwise), and the tail divider when `tail`.
- `TreeDepth` sets the inset and the guides that pass through the row. `One` (a child of a top-level row): inset `NESTED_INSET_X` and the top guide. `Two` (a child of a nested parent): inset `LEAF_INSET_X` and both guides. The guides come from `tree_guide.rs`.

| Row | Level | Depth |
| --- | --- | --- |
| Music album | Parent | One |
| Music track | Leaf | Two |
| Shows season (expandable or not) | Parent | One |
| Shows episode | Leaf | Two |
| Playlists item | Leaf | One |

`TreeTrail` is `Date(String)` for a parent and `Duration(String)` for a leaf. An empty value paints nothing.

### D4. One derivation for each shared rule

- Title tone: a row with `play: Some(state)` paints the muted tone when `is_played_episode(state)` and the text tone otherwise (the Shows rule today). A row with `play: None` paints its level's tone: text for top-level and parent rows, muted for leaf rows.
- Progress run: `FlatRow` paints the thumb's run at `episode_progress(state)`. `TreeRow` paints no run, as no tree row does today.
- Selection fill and radius: the part derives them from the level (D2, D3).
- Context press: one handler in `list_row.rs` stops the default, calls `on_press`, then calls `on_context` with the pointer position.
- Hooks: every part paints `library_attr(attr, part)` for `row`, `row-id`, `selected`, `title`, `subtitle`, `number`, the trailing part, `thumb`, and `thumb-progress`. The Music list keeps `attr: ""`, Shows `"shows"`, and Playlists `"playlists"`.
- Shows episode state: the Shows list keeps its lookup of the carried episode, which returns the carried state (`episode_state`), and passes it as `play`.
- Home: both sections pass `play: Some(item.state)`.

### D5. Tests

- `list_row/tests.rs` owns the part contract. `library_item_row/tests.rs` moves there for `FlatRow`. One `#[case]` table covers `TreeRow` over level and depth (inset, height, face, hint) and over `play` (`None`, unplayed, played: the title tone). No other new test.
- Each list's tests keep their contracts (which rows paint, their hooks, their handlers). They change only for the hook names (Playlists, Shows) and for the looks this change moves to the Music look. Tests of removed behaviour go with it: the Recently added mark test, and any test that asserts a Playlists, Shows, or Recently added look that this change removes.
- The user does the live look check.

## Risks / Trade-offs

- [Lists that differ from the Music look change their look] -> Intended. The proposal lists the visible results.
- [The playlist row is 56 px high with no thumb] -> It takes the top-level look. A playlist thumb is a feature and out of scope.
- [Open changes edit the same painters and hooks] -> See proposal Impact.
- [`list_row/tree.rs` grows large] -> If it nears 800 lines, split the guide and hook derivation into `list_row/tree/` children.

## Migration Plan

None. Rollback is a revert.
