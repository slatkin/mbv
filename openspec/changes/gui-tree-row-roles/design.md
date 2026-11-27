# Design

## Context

See `proposal.md` for the why, and `specs/gui-client/spec.md` for the behaviour. The look source
is the brief `~/mbv-tree-rows-brief.pdf` and its one mockup image (an 840 × 2200 screenshot at
about 2 image px per CSS px). All paths below are under `crates/mbv-gui/src/window/` unless they
say otherwise.

### Inventory: every list in the GUI, and which ones are trees

| List | Component and model | Depth | Level → role | How its rows paint today |
| --- | --- | --- | --- | --- |
| Music (Library, Music destination) | `components/library_list.rs`; model `library/list.rs` (`LibraryRow`, `build_library_rows`) | 3 | artist → root, album → branch, track → leaf | Shared parts: artist through `FlatRow` (56 px, thumb), album through `TreeRow` (`TreeLevel::Parent`, `TreeDepth::One`, 30 px), track through `TreeRow` (`Leaf`, `Two`, 28 px). Virtual list (`library_virtual_list`): only rows in view paint. Per-row `TreeGuide` spans. Nested rows paint `--canvas-tree-subtree`. Letter headings sit between artists. |
| Shows, All view (TV) | `components/library_shows_list.rs`; model `library/shows_list.rs` (`ShowsListRow`, `series_shows_rows`) | 3 | series → root, season → branch, episode → leaf | The same parts, heights, guides, and virtual list as Music. A series or a season can be non-expandable (`expandable: false`). Episodes carry a play state (played mark, `RowPlay` tones). A season carries no year. |
| Playlists screen | `components/playlists_panel.rs` (`playlist_node`, `items_block`, `rename_field`) | 2 | playlist → root, item → leaf | Playlist through `FlatRow` with no thumb; item through `TreeRow` (`Leaf`, `One`). Not virtual: every row paints. The core holds one open playlist at a time. The rename field paints inside the open playlist's subtree. An item carries a title only (no number, no runtime, no play state). |

Lists that do not expand, and so stay out of scope (brief §3, "a list that cannot expand at all"):

- Shows New and Upcoming views: flat `FlatRow` lists in the same component.
- Home (`components/library_home_list.rs`): flat, with section headings.
- Plain Emby libraries — Movies, mixed libraries, folders, and boxsets
  (`library/lettered/adapters.rs`, `EmbyList`): flat. A collection row plays by the folder-play
  rule; it never expands.
- Audiobooks (`AudiobooksList`): a flat book list. Chapters show only in the Hero's Workspace
  table, which is in the centre pane (brief §7).
- Podcasts (`PodcastsList`): a flat episode list. A show is a collection filter, not a root row.
- Feeds, YouTube included (`FeedsList`): a flat entry list. YouTube is a Feeds platform
  (`service_mark.rs`), not a destination of its own.
- The Favourites lists of the in-progress `emby-favourites` change: flat (its design paints
  them with "the shared flat-row parts").
- The Hero tracklist and Workspace tables, the Queue (its fold is not a tree), Settings.

No tree is deeper than 3 levels. The depth-4+ rule of brief §3 has no list to apply to, so this
design does not build for it (D1 keeps the role enum open to a later level).

### Where the brief and the code disagree

1. **The tree lists.** The brief lists Music, Movies, TV, YouTube, Podcasts, Audiobooks, and
   Feeds. Only Music, Shows All, and Playlists expand in the GUI.
2. **The rows already share parts.** `gui-unified-list-rows` gave every row one of two parts:
   `FlatRow` (top-level) and `TreeRow` (nested parent and leaf), with `RowLevel` rules in
   `components/list_row.rs` and the guides in `components/tree_guide.rs`. Invariant 30 and 38
   name them as the one painter of their shape. This change reshapes the nested part by role; it
   does not have to unify bespoke painters.
3. **"Recessed" root rows.** A root row today is a solid `--canvas-library-column` fill (#272e33)
   on the list column's `--canvas-library-panel` (#2d353b), with a solid 1 px `--canvas-border`
   line under it. No shadow paints anywhere in the trees. So "the same technique" is a darker
   solid fill plus solid 1 px lines — the brief's own fallback (D3).
4. **The mockup hex values are not in the code.** The mockup uses a neutral palette (#262d2e,
   #1e2425 …); the code's panel and root fills are #2d353b and #272e33. D6 maps each value by
   its role and its step from its neighbours, not by nearest hex.
5. **The swatch colours are in use.** The five swatch hexes are exactly the values of five live
   canvas tokens: `--canvas-destructive` (red), `--canvas-source` / `--canvas-type-date`
   (blue), `--canvas-owner` / `--canvas-type-role` (yellow), `--canvas-type-secondary`
   (purple), `--canvas-partial` / `--canvas-type-count` (orange). They are not "colours the app
   doesn't use elsewhere".
6. **Names.** `CONTEXT.md` reserves "Tray" for the desktop status icon and lists "chip" as an
   *Avoid* word for *Fact pill*; `track_chips.rs` and the transport chips also use "chip".
7. **Guides and chevrons.** The spec requires indent guides; the mockup has none. The
   `gui-tree-double-click` decision removed every open-or-closed glyph from tree rows
   (`docs/architecture/gui-client.md`: "No tree row paints an open-or-closed glyph"); the mockup
   adds one. The hover Expand control uses `ChevronDown` to mean "press to open"; the mockup
   uses a down chevron to mean "is open".
8. **Open and picked overlap.** A double-click's first click presses the row, and a press picks
   it (the Hero shows it). So the album the user just opened is nearly always the picked one.
9. **Transparency in states.** The pointer look on every control is a translucent veil
   (`--canvas-hover-veil`, 14 % `color-mix`, design D16 of the GUI) and a parent row's year
   paints at `--canvas-type-repeat-opacity` (0.65).
10. **Leaf title tone.** A leaf with no play state (track, playlist item) paints its title in
    `--canvas-muted`; the mockup's leaf title sits close to the branch title. An unplayed Shows
    episode already paints `--ef-fg` (= `--canvas-text`).
11. **Missing facts.** A Shows season carries no year and no total time. A Playlists item carries
    no number and no runtime. The Music album frame carries `total` (count and time) once its
    tracklist loads (`bridge/frame/library/music.rs`).
12. **Keyboard focus.** GUI lists have no cursor and no row focus (spec "Library rows have no
    cursor and no selection"). Keys act on the row the pointer marks. "Focus" in the brief's API
    is the hover state.
13. **Heights.** The leaf row is already 28 px. The branch row is 30 px today; the brief asks 42.

### Constraints that shape the approach

- The Music and Shows rows paint one row at a time through the virtual list, and a painter does
  not see its neighbours. A group fill, a rail, and a well cannot be one container box: each row
  must paint its own segment, and segments of adjacent rows must meet. The guides already work
  this way (`gui-tree-indent-guides` D1).
- The row model holds every row's top and height (`GeometryRow`). Any gap above or below the
  well must be part of a row's height, or the scroll, the reveal, and the rail jump go wrong.
- `Truncate` (`truncate.rs`) cuts one text in one style. It cannot cut a line of typed segments.
- `list_row/tests.rs` has 773 lines; new part tests need their own files (800-line limit).

## Goals / Non-Goals

**Goals:**

- One part per role (root, branch, leaf) that every tree paints, so a look fix lands in all
  three trees, and one rail part that the three roles share.
- The look survives the virtual list: whole under any scroll, stable swatch colours.
- Every surface and every state paints a solid canvas token.
- Music first, reviewable live before the other trees move.

**Non-Goals:**

- Any behaviour change (press, double-click, Enter, hover controls, Expand all, Collapse all,
  filter, refresh, pointer target, letter rail, sort, fetch, open state).
- Artwork for branch rows; the Hero, Workspace tables, Queue, player panel; the TUI.
- A depth-4 tree; making a flat list expand.
- Making the Playlists tree virtual (Risk R8).

## Decisions

### D1. Three role parts and one rail part

The roles map onto parts as follows. `components/list_row/tree.rs` (`TreeRow`, `TreeLevel`,
`TreeDepth`, `TreeTrail`) and `components/tree_guide.rs` are deleted at the end (D8).

| Role | Part | File |
| --- | --- | --- |
| Root | `FlatRow` (unchanged name) | `components/list_row/flat.rs` |
| Branch | `BranchRow` (new) | `components/list_row/branch.rs` |
| Leaf | `LeafRow` (new) | `components/list_row/leaf.rs` |
| Rails | `TreeRail` (new, replaces `TreeGuide`) | `components/tree_rail.rs` |

The API the brief asks for (role, depth, sibling index, expanded, open, selected, focus) maps
like this. The part is the role, so no `role` prop exists. Depth is fixed by the role (no depth-4
tree), so no `depth` prop exists and `TreeDepth` goes. Focus is CSS hover (Context 12), so no prop
exists. A destination still passes no look value (invariant 30, 38).

```rust
// Root: FlatRow keeps every prop. Two props change meaning.
pub tree_open: Option<bool>,   // Some(true): the row paints the open rail (TreeRail::Open)
pub tree_group: bool,          // was `tree_guide`: the row shows child rows, so it paints no
                               // divider; the group's last row paints it (subtree_tails)

// Branch
#[derive(Props, Debug, Clone, PartialEq)]
pub struct BranchRowProps {
    pub attr: &'static str,            // hook prefix, as today
    pub kind: String,                  // "album" | "season"
    pub id: String,
    pub title: String,
    pub sibling: usize,                // position among the painted siblings: picks the swatch
    pub facts: String,                 // the secondary line; empty paints none (D5)
    pub open: Option<bool>,            // None: the row opens nothing (no state mark, no toggle)
    pub selected: bool,                // the Hero's item
    pub tail: bool,                    // the group's last row: paints the root's divider
    pub on_press: Callback<()>,
    pub on_play: Callback<()>,
    #[props(default)] pub on_toggle: Option<Callback<()>>,
    #[props(default)] pub on_hover_play: Option<Callback<()>>,
    #[props(default)] pub on_hover_queue: Option<Callback<()>>,
    #[props(default)] pub on_hover_shuffle: Option<Callback<()>>,
    #[props(default)] pub on_context: Option<Callback<(f32, f32)>>,
}

// Leaf
#[derive(Props, Debug, Clone, PartialEq)]
pub struct LeafRowProps {
    pub attr: &'static str,
    pub kind: String,                  // "track" | "episode" | "item"
    pub id: String,
    pub title: String,
    pub well: WellEdge,                // the row's place in its well (D3)
    #[props(default)] pub number: Option<String>,
    #[props(default)] pub duration: Option<String>,
    #[props(default)] pub play: Option<GuiHomeRowStateFrame>,
    pub selected: bool,
    pub tail: bool,
    // the same handler props as BranchRow, without on_toggle and on_hover_shuffle
}

// Rails (tree_rail.rs)
pub enum RailTone { Open, Group }     // the bright root rail, the muted group rail
#[component] pub fn TreeRail(props: TreeRailProps) -> Element  // one absolute 3 px span
pub fn swatch_var(sibling: usize) -> String                      // D4
```

`RowLevel` in `list_row.rs` keeps the shared rules: `Top`, `Parent` → renamed `Branch`, `Leaf`.
The selection fill, the title tone and weight, and `RowPlay` stay its one derivation.

*Alternative rejected:* one `TreeRow` with a `role` enum. The two roles share no layout, only
the hover host and the handlers. Two small parts keep each under the 800-line limit and give
each role a type that cannot be built with the other role's data.

### D2. Per-row segments paint the group

Each row of a group paints its own segment, as the guides did:

- **Root (open):** a `TreeRail { tone: Open }` span at x 0, full row height.
- **Branch:** the group fill (`--canvas-tree-subtree`, retuned) as its row fill, and a
  `TreeRail { tone: Group }` span at x 0.
- **Leaf:** an outer strip in the group fill with a `TreeRail { tone: Group }` span; the well
  segment sits inside the strip (D3).
- **Rename field (Playlists):** the group fill and a `Group` rail, as a leaf strip has.

The group's last row paints the root's 1 px `--canvas-border` divider at its bottom, through the
existing `subtree_tails` (moved from `tree_guide.rs` to `tree_rail.rs`). Segments meet because
every row's height is a whole number of pixels and rows have no margins.

*Why:* the virtual list paints only the rows in view, so a container would need the group's
height and the scroll window. Per-row segments need only the row's own facts.

### D3. The leaf well: segments, edges, and solid depth cues

`WellEdge` is plain row-model data in `library/list.rs`: `Only`, `First`, `Middle`, `Last`, with
`WellEdge::of(index, len)` and `WellEdge::height(self) -> f32`. The builders set it on every leaf
row and use its height, so the scroll geometry holds the well's gaps.

A leaf row paints this DOM (two boxes more than today):

```text
div  strip     group fill, `relative`, the row's full height, padding: gap-top, WELL_RIGHT_GAP,
               gap-bottom, WELL_X; the tail divider when `tail`
  span         TreeRail { Group }
  div  segment well fill; First/Only: top radii, 1 px top shade; Last/Only: bottom radii, 1 px
               bottom lip; First/Last/Only: `overflow: hidden` (clips the fill to the corners)
    div  host  `row-hover-host`, `--row-fill`: the well fill, or the selection fill
      button   28 px; Middle/Last: 1 px top separator (`border-box`, inside the 28 px)
      RowHoverActions
```

The inner shadow of the brief is not built as `box-shadow` (Risk R1). The depth cue is the root
rows' own technique: solid fills and solid 1 px lines — a darker line on the well's top edge
(`--canvas-tree-well-shade`) and the root divider's lighter line on its bottom edge
(`--canvas-border`).

In the depth-2 tree (Playlists) the same `LeafRow` paints; the well sits under the open root.

### D4. Branch swatch colour comes from the row model

The row builders (`push_albums` in `library/list.rs`, `series_shows_rows` in
`library/shows_list.rs`) count the branch rows they push under one root from 0 and store the
count as `sibling: usize` on `LibraryRow::Album` and `ShowsSeasonRow`. The count is over the
painted siblings, so a filter can re-colour rows (brief §5 accepts this) and two adjacent rows
never share a colour. The rows are memoized (`row_memo.rs`) and the virtual list reads them as
they are, so a scroll never changes the count.

`swatch_var(sibling)` names one of five tokens, `--canvas-tree-swatch-1` … `-5`. Default rule
(Open Question 1): `1 + sibling % 5` for the first pass, then the cycle restarts at blue —
indices 5, 6, 7, 8 → swatches 2, 3, 4, 5, then 2 again — so orange is never followed by red.

The swatch is a 28 × 28 px span with a 4 px radius and the token as its fill. It carries the
`data-tree-swatch` hook with the swatch number. It loads nothing.

### D5. The branch row's content

Height 42 px (`BRANCH_ROW_HEIGHT`). Left to right: the rail, the swatch at `SWATCH_X`, a text
column (title over facts), the state mark.

- Title: sans 14 px (`TREE_ROW_FONT_PX`), `Truncate`; tone and weight in D7.
- Facts: one mono 11 px line in one tone, `Truncate`, no repeat opacity. A Music album: its year;
  an open album with a loaded tracklist: `year · N tracks · total` from the carried `total`.
  A Shows season: empty while closed; when open and loaded: `N episodes` from the carried
  episodes. The builder writes the string into the row model (`facts: String`), so the part
  does not know the media kind. An empty string paints no line and the title centres.
- State mark (Open Question 4 default): Lucide `ChevronRight` (closed) or `ChevronDown` (open),
  14 px, `pointer-events-none`, carrying the `row-hover-trail` class so it hides with the trail
  while the hover controls show. A branch with `open: None` paints no mark.

### D6. Theme tokens

All tokens live in the GUI-local `:root` canvas block of `crates/mbv-gui/assets/app.css`, next to
the tree tokens of `gui-tree-indent-guides`, each with a comment naming this change. Rust names
them through `canvas_var` only. The swatch names are a `const` array in `tree_rail.rs`.
`make gui-css` regenerates `app.generated.css`.

New values are derived from the code's own fills with the mockup's step ratios, so the steps the
mockup shows survive on the code's palette:

| Brief element | Mockup | Token | Value | Status |
| --- | --- | --- | --- | --- |
| Panel background | #262d2e | `--canvas-library-panel` | #2d353b | existing, unchanged |
| Root row background | #1e2425 | `--canvas-library-column` | #272e33 | existing, unchanged |
| Root row divider (the recess line) | — | `--canvas-border` | #3d484d | existing, unchanged |
| Expanded-root rail, 3 px | #9dc37b | `--canvas-tree-rail` | `var(--canvas-accent)` (#a7c080, Everforest green) | new name, alias |
| Branch group background | #212829 | `--canvas-tree-subtree` | #293136 (was #313940): 5/8 of the way from panel to root, the mockup's step | existing concept, new value |
| Branch group rail, 3 px | #596e4e | `--canvas-tree-rail-muted` | #627157: 45 % accent over the group, the mockup's ratio | new literal |
| Open branch background | #2d3731 | `--canvas-tree-branch-open` | #363f3d: 10 % accent over the group, the mockup's ratio | new literal |
| Open and picked branch | — | `--canvas-tree-branch-open-picked` | #404c4a: 10 % accent over `--canvas-library-raise` | new literal (Open Question 6) |
| Picked row, any role | — | `--canvas-library-raise` | #343f44 | existing, unchanged |
| Leaf well background, 6 px radius | #141819 | `--canvas-tree-well` | `var(--canvas-dim)` (#1e2326, Everforest bg_dim: the root fill minus the mockup's step) | new name, alias |
| Leaf row separator, 1 px | #1a1e1f | `--canvas-tree-well-rule` | #24292c: the well plus the mockup's step | new literal |
| Well top edge (shadow fallback) | — | `--canvas-tree-well-shade` | #15191b (the darkest palette value, equal to `--ef-player` but independent) | new literal |
| Well bottom edge (shadow fallback) | — | `--canvas-border` | #3d484d | existing (the root rows' line) |
| Branch title | #c6cecc | `--canvas-text` | #d3c6aa | existing |
| Open branch title, weight 500 | #eef2ee | `--canvas-heading` | #e9e1cc | existing |
| Branch facts line, mono 11 px | #7fb39a | `--canvas-tree-facts` | `var(--canvas-type-date)` (#7fbbb3) | new name, alias (Open Question 8) |
| Branch state mark, closed | — | `--canvas-muted` | #9da9a0 | existing |
| Branch state mark, open | — | `--canvas-tree-rail` | as above | new (above) |
| Leaf index | #66716f | `--canvas-ordinal` | #7f897d | existing, unchanged |
| Leaf title (no play state) | #bcc4c2 | `--canvas-text` | #d3c6aa (was `--canvas-muted`) | existing (Open Question 9) |
| Leaf duration | #8c9894 | `--canvas-muted` | #9da9a0 | existing, unchanged |
| Swatch 1 red | #e67e80 | `--canvas-tree-swatch-1` | #e67e80 | new literal |
| Swatch 2 blue | #7fbbb3 | `--canvas-tree-swatch-2` | #7fbbb3 | new literal |
| Swatch 3 yellow | #dbbc7f | `--canvas-tree-swatch-3` | #dbbc7f | new literal |
| Swatch 4 purple | #d699b6 | `--canvas-tree-swatch-4` | #d699b6 | new literal |
| Swatch 5 orange | #e69875 | `--canvas-tree-swatch-5` | #e69875 | new literal |

The swatch tokens and the rail and well-edge tokens are literals, not aliases of the
destructive, date, role, secondary, partial, or player tokens: they are separate concepts that
must move alone (invariant 7). The rail and the well alias the Everforest palette tier
(`--canvas-accent`, `--canvas-dim`), as `--canvas-tree-guide-nested` aliased
`--canvas-insertion`.

Retired at the end (D8): `--canvas-tree-guide`, `--canvas-tree-guide-nested`.

Every value is opaque. No token uses `rgba`, `color-mix`, or `opacity`.

### D7. States per role

Fill precedence on a row: selection, then open, then the role's rest fill. Every fill is solid.

| State | Root | Branch | Leaf |
| --- | --- | --- | --- |
| Rest | `--canvas-library-column` + divider (as today) | group fill, title `--canvas-text` 400 | well fill, title `--canvas-text`, mono 13 px 500 |
| Open | open rail on the left edge; no divider while it shows child rows | `--canvas-tree-branch-open`, title `--canvas-heading` 500, down mark in rail tone | — |
| Picked (the Hero's item; "selected") | `--canvas-library-raise` (as today) | closed: `--canvas-library-raise`; open: `--canvas-tree-branch-open-picked` | `--canvas-library-raise`, square, clipped by the well corners |
| Hover / "keyboard focus" (the pointer target) | the shared veil and the hover controls (as today) | as root; the state mark hides with the trail | as root; the duration and play mark hide |
| Played / unplayed | no tree root carries a play state | no branch carries one | Shows episodes keep `PlayCluster` and the `RowPlay` tones; tracks and playlist items carry none |
| Now playing | no tree row shows it today; this change adds none (Queue scope) | — | — |

The selection radius of `RowLevel` becomes 0 at every level: a leaf's selection fills the well
width and the well's corners clip it; a branch's fills its row.

### D8. Migration: new parts beside the old, Music first

1. Terms and tokens land with no painter change.
2. The Music row model gains `sibling`, `facts`, `WellEdge`, and the new heights.
3. `BranchRow`, `LeafRow`, `TreeRail`, and `swatch_var` land, with their tests. `FlatRow` paints
   the open rail for every open root in all three trees and stops painting a guide start.
4. Music moves to the new parts and stops painting guides. **The user looks at Music live
   before step 5** (Migration Plan).
5. Shows: model, then painter.
6. Playlists: painter (it has no row model; edges come from the item index).
7. Delete `TreeRow`, `TreeLevel`, `TreeDepth`, `TreeTrail`, `TreeGuide`, the guide constants,
   `NODE_ROW_HEIGHT`, and the two guide tokens; rename `FlatRow::tree_guide` to `tree_group`;
   update the docs.

Between steps 3 and 6, an open Shows series or playlist shows the new root rail beside the old
guides of its child rows (the guide start under the thumb no longer paints). This lasts only
until that tree moves. A temporary flag on `FlatRow` to avoid it would be an escape hatch
(invariant 30), so the design accepts the short mixed look.

### D9. Names

`CONTEXT.md` gains two GUI terms, each with an *Avoid* line:

- *Branch swatch (GUI)*: the flat colour square on a branch row that tells it apart from its
  siblings. It carries no meaning and no artwork. *Avoid*: chip, colour chip, tag.
- *Leaf well (GUI)*: the recessed block that holds the leaf rows of one open parent in a GUI
  tree. *Avoid*: tray (the *Tray* is the desktop status icon), card.

Code and hooks use `swatch` and `well` (`data-tree-swatch`, `data-tree-well`, `data-tree-rail`).

### D10. Tests: the contracts and their owners

A look is verified by looking (invariant 30 rule 4). Tests own mappings only:

- Row model (`library/list/tests.rs`, `library/shows_list/tests.rs`): the sibling position counts
  the painted branch rows of one root from 0 and restarts under the next root; `WellEdge::of`
  and the edge heights add up into the row tops (one `#[case]` table per contract).
- `swatch_var` (in `tree_rail.rs`'s own tests): the wrap rule, one case table.
- The stylesheet defines every tree token the Rust names (the pattern of
  `every_ef_token_names_a_stylesheet_property`).
- Painted structure (new files `list_row/tests/branch.rs`, `list_row/tests/leaf.rs`): a branch
  paints its swatch hook for its sibling and its open state; a leaf paints its well edge and
  the separator only off the first row; an open root paints the open rail.
- The list tests (`library_list/tests.rs`, `library_shows_list/tests.rs`,
  `playlists_panel/tests.rs`) keep their behaviour assertions. Only expectations that named a
  guide, an inset, or a height change.

No screenshot test, no browser, no live mpv or server (AGENTS rules).

## Risks / Trade-offs

- **R1. Inset `box-shadow` (Blitz).** `blitz-paint 0.3.0-beta.4` paints inset shadows
  (`render/box_shadow.rs::draw_inset_box_shadow`) but uses the average of the four corner radii
  ("TODO draw shadows with matching individual radii"), so a well segment with only its top
  corners rounded gets a wrong shape. A soft shadow also needs an alpha colour, which the
  no-transparency rule forbids, and a per-row shadow would draw a line at every row seam. →
  Solid 1 px top and bottom lines (D3). No `box-shadow` in this change.
- **R2. Rounded clipping (Blitz).** `overflow: hidden` clips to the padding-box path, and that
  path follows each corner's own radius (`render.rs` clip layer, `kurbo_css/css_box.rs::shape`).
  So the first and last segments clip the selection fill and the hover cover to the well's
  corners. → If a clip misbehaves live, give the host and the hover cover the same corner radii
  instead of clipping, and drop `overflow: hidden`.
- **R3. Corner arcs with mixed border widths (Blitz).** blitz-paint splits a rounded corner's
  arc by border width; a 1 px top line next to 0 px sides can draw a tapered corner, and a
  zero-width rounded border once made NaN paths (the `border-color: transparent` rule in
  `app.css` covers that). → If the corner looks wrong, paint the shade and the lip as 1 px
  absolute spans inside the segment instead of borders.
- **R4. Seams between segments.** At a fractional scale factor, two abutting row segments can show
  a hairline. The guides had the same exposure and showed none. → Keep every height a whole
  pixel. Fallback: a 1 px overlap from each strip into the next row.
- **R5. A barely visible group.** The group fill sits 2–5 levels per channel from the panel and
  the root fill, as in the mockup. On some displays it may not read. → The user judges it in the
  Music pilot; the token is one value to retune.
- **R6. Typed facts cannot truncate.** `Truncate` cuts one text in one style. A facts line in
  typed colours (Open Question 8, "no") would clip with no ellipsis at narrow widths. → The
  default is one tone with `Truncate`.
- **R7. The hover veil is translucent.** It composites a 14 % veil over the row fill. It is an
  overlay, not a surface, and it is app-wide (design D16), so this change keeps it (Open
  Question 7). Replacing it only in trees would give trees a second pointer look.
- **R8. Performance.** Music and Shows paint only rows in view, so a 100+ episode season costs
  the same as today per frame plus two boxes per leaf row and one swatch per branch row. No
  shadow, no opacity layer, no filter. Clip layers: at most two per well (its first and last
  rows). Each branch row measures two `Truncate` boxes instead of one. The Playlists tree is not
  virtual: a playlist with 1,000 items paints 1,000 rows today and will paint 3,000 boxes more.
  → Accepted for this change; making Playlists virtual is a separate change.
- **R9. Geometry change.** Branch rows grow from 30 to 42 px and wells add 4 + 6 px. A saved
  scroll offset lands on a different row once after the update; the reveal of the picked row and
  the rail jump read the new geometry, so they stay correct.
- **R10. Interim mixed look** between D8 steps 3 and 6 (D8).
- **R11. Spec sync collision.** `emby-favourites` also has a `gui-client` delta. No requirement
  overlaps; the second change to sync rebases.

## Verification

Gates for every step: `make format`, `make checks PKG=mbv-gui`, `make test PKG=mbv-gui`, and
`make check-code-file-lines` before a push.

Behaviour (automated, existing tests that must pass with only look expectations updated):
press picks, double-click toggles, a leaf double-click plays, the hover control set per kind,
Expand all and Collapse all, the filter's auto-open and the closed override, the refresh of open
rows in view, the pointer marker on every non-heading row, the letter rail jump, and the reveal
of the picked row.

Look (the user, live, after `make run`), per tree:

- **Music (pilot):** compare an open artist with an open album against the mockup; check each
  swatch differs from its neighbour and keeps its colour while scrolling; scroll so only the
  middle of a long album is in view and see the group, the rail, and the well whole; narrow and
  widen the list column and see titles cut with an ellipsis and the well keep its right gap;
  hover each role; pick an open album and a closed one; filter so a track match opens its
  album; check the next artist after a group looks ordinary.
- **Shows:** the same, plus an open season whose episodes are still loading, a non-expandable
  series and season, played and unplayed episode marks inside the well, and the New and
  Upcoming views unchanged.
- **Playlists:** an open playlist's well directly under it with no swatches; rename a playlist
  while it is open; an empty open playlist; a long playlist for scroll speed.
- **Flat lists:** a glance at Home, a Movies library, Audiobooks, Podcasts, Feeds (one YouTube
  feed), and the Shows New view: no change.

## Migration Plan

Each D8 step is one commit or a small group of commits on the change's branch. After step 4 the
user runs `make run` and reviews Music; findings change the tokens or constants before step 5
starts. Rollback is a revert of the change's commits; no data, setting, or protocol changes, so
nothing needs a migration.

## Open Questions

The tasks follow each default. A different answer changes the named tasks before work starts.

1. **Swatch wrap (brief §9).** Restart the cycle at blue after the fifth sibling, so orange is
   never next to red? Default: yes (D4).
2. **Depth-2 leaf colour (brief §9).** Give depth-2 leaves (Playlists items) no per-row colour?
   Default: yes, none.
3. **Open-branch counts (brief §9).** Show the count and total only where the frame carries the
   facts — a Music album's count and total time, a Shows season's episode count with no total
   (the core carries no season total; adding one is core projection work)? Default: yes (D5).
4. **State mark.** Paint the right/down chevron on branch rows, reversing the
   `gui-tree-double-click` decision of "no open-or-closed glyph", as a mark that hides under the
   hover controls? Default: yes.
5. **Guides.** Remove the indent guides, with their three spec requirements, now that rails and
   wells show the groups? Default: yes.
6. **Open and picked.** Give an open branch that is also the Hero's item its own solid fill
   (`--canvas-tree-branch-open-picked`), so both facts show? Default: yes (D7).
7. **Hover veil.** Keep the app-wide translucent hover veil on tree rows? Default: yes (R7).
8. **Facts tone.** Paint the whole facts line in one tone, so it can end in an ellipsis, rather
   than in the typed colours (year blue, count orange, time green)? Default: yes (R6).
9. **Leaf title tone.** Paint a leaf title with no play state in `--canvas-text` (brighter than
   today's `--canvas-muted`), as the mockup and the unplayed Shows episode do? Default: yes.
10. **Names.** Use *Branch swatch* and *Leaf well* in `CONTEXT.md` and the code? Default: yes (D9).
11. **Well's left edge.** Align the well's left edge with the swatch's left edge, as the brief's
    text says? (The mockup's pixels put it a little to the right of the swatch.) Default: yes.
12. **List gutter.** Keep the 8 px left gutter of the Music and Shows lists, so the rails sit
    8 px in from the panel edge rather than flush as in the mockup? Default: yes.
13. **Scope.** Leave the TUI trees and the flat GUI lists (Movies, YouTube, Podcasts,
    Audiobooks, Feeds, Home) as they are? Default: yes.
