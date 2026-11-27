# Tasks

Rules for every task. Read `crates/mbv-gui/AGENTS.md` and `design.md` first. The tasks follow
the defaults of `design.md` → Open Questions. Paths are under `crates/mbv-gui/` unless a path
starts at the repository root. Run `make format` after each Rust edit. Run `make checks
PKG=mbv-gui` and `make test PKG=mbv-gui` in the foreground and wait for them; never run them in
the background, never sleep, never poll. Add a test only where the task names its contract
(`design.md` D10); otherwise update only the expectations that a changed look breaks. Use no
lint suppression. Keep every file under 800 lines. Commit each group with a message that names
the group.

## 1. Terms and tokens

- [x] 1.1 In `CONTEXT.md` (repository root), add the terms *Branch swatch (GUI)* and *Leaf well
  (GUI)* with the definitions and *Avoid* lines of design D9, next to the other GUI Library
  terms. Verify: `rg -n "Branch swatch|Leaf well" CONTEXT.md` prints both terms and their
  *Avoid* lines.
- [x] 1.2 In `assets/app.css`, in the `:root` canvas block after `--canvas-tree-subtree`, add the
  new tokens of design D6 with their values and a comment that names `gui-tree-row-roles`
  design D6: `--canvas-tree-rail`, `--canvas-tree-rail-muted`, `--canvas-tree-branch-open`,
  `--canvas-tree-branch-open-picked`, `--canvas-tree-well`, `--canvas-tree-well-rule`,
  `--canvas-tree-well-shade`, `--canvas-tree-facts`, and `--canvas-tree-swatch-1` to `-5`. Do
  not change `--canvas-tree-subtree` or remove a guide token yet. Run `make gui-css`. If it
  reports the Tailwind CLI missing, stop and report that to the parent; install nothing.
  Verify: `rg -o -e "--canvas-tree-(rail|branch|well|facts|swatch)[a-z0-9-]*:" assets/app.generated.css`
  prints 13 definitions (the file is minified on one line), and `git diff --stat` shows only the two CSS files and `CONTEXT.md`.

## 2. Music row model

- [x] 2.1 In `src/window/library/list.rs`, add `pub const BRANCH_ROW_HEIGHT: f32 = 42.0;`, the
  well gap constants `WELL_GAP_TOP_PX = 4.0` and `WELL_GAP_BOTTOM_PX = 6.0`, and
  `pub enum WellEdge { Only, First, Middle, Last }` with `WellEdge::of(index: usize, len: usize)`
  and `WellEdge::height(self) -> f32` (`TRACK_ROW_HEIGHT` plus the gaps its edge carries), each
  with a doc comment that names design D3. Re-export them from `components/list_row.rs` beside
  the other row constants. Verify: `make checks PKG=mbv-gui` passes.
- [x] 2.2 In `src/window/library/list.rs`, give `LibraryRow::Album` the fields `sibling: usize`
  and `facts: String`, and `LibraryRow::Track` the field `well: WellEdge`. In `push_albums`,
  count the pushed albums of the artist from 0 into `sibling`; build `facts` per design D5 (the
  year; for an open album whose frame carries `total`, `year · N tracks · total`, skipping an
  empty year); push albums at `BRANCH_ROW_HEIGHT`; set each track's `well` with
  `WellEdge::of` over the album's kept tracks and push it at `well.height()`. Fix every pattern
  and constructor the compiler names with `..` or the new field. Verify: `make checks
  PKG=mbv-gui` passes.
- [x] 2.3 In `src/window/library/list/tests.rs`, add two contract tests (design D10): the album
  `sibling` counts from 0 under each artist and restarts under the next open artist; and the
  track tops of an open album follow `WellEdge` heights (a `#[case]` table for one, two, and
  three tracks). Update the existing height expectations that the 42 px album row and the well
  gaps break. Verify: `make test PKG=mbv-gui` passes. The Music painter still paints the old
  look at the new heights; group 4 changes it.

## 3. The shared role parts

- [x] 3.1 Add `src/window/components/tree_rail.rs` (register it in `src/window/components.rs`):
  `RailTone { Open, Group }` and `#[component] TreeRail` (one `pointer-events-none absolute`
  span at x 0, full height, 3 px wide, `data-tree-rail` = `open` or `group`, fill
  `--canvas-tree-rail` or `--canvas-tree-rail-muted`); the five swatch token names as a
  `const` array and `swatch_var(sibling)` with design D4's wrap rule; `subtree_tails`, moved
  here from `tree_guide.rs` with its callers' imports updated. Add its own tests: a `#[case]`
  table for `swatch_var` at siblings 0, 4, 5, and 9; and one assertion that every token name
  the module holds is a property in `assets/app.css` (the pattern of
  `every_ef_token_names_a_stylesheet_property`). Verify: `make test PKG=mbv-gui` passes.
- [x] 3.2 Add `src/window/components/list_row/branch.rs` with `BranchRow` and `BranchRowProps`
  per design D1, D5, and D7: the hover host with `--row-fill`, the group rail, the swatch from
  `swatch_var`, the title and facts `Truncate` boxes, the state mark (`ChevronRight` /
  `ChevronDown`, `row-hover-trail`), the open and picked fills, `aria-expanded`, the tail
  divider, and `RowHoverActions` with the row's own toggle. Name its layout constants
  (`SWATCH_X`, `SWATCH_PX`, `SWATCH_RADIUS_PX`, the 12 px text gap) with doc comments. Rename
  `RowLevel::Parent` to `RowLevel::Branch` in `components/list_row.rs` and set the selection
  radius to 0 at every level. Verify: `make checks PKG=mbv-gui` passes.
- [x] 3.3 Add `src/window/components/list_row/leaf.rs` with `LeafRow` and `LeafRowProps` per
  design D1, D3, and D7: the strip, the group rail, the well segment with its edge radii, top
  shade, bottom lip, and `overflow: hidden` on edge segments, the host, the 28 px button with
  the separator off the first row, the number cell, the title in `--canvas-text` (or the
  `RowPlay` tone), the duration or `PlayCluster`, the `data-tree-well` hook, and the tail
  divider on the strip. Name `WELL_X` and `WELL_RIGHT_GAP_PX`, `WELL_RADIUS_PX` with doc
  comments. Verify: `make checks PKG=mbv-gui` passes.
- [x] 3.4 In `src/window/components/list_row/flat.rs`, paint `TreeRail { tone: Open }` while
  `tree_open == Some(true)`, and stop painting `parent_guide_start`. Update the module doc.
  Verify: `make checks PKG=mbv-gui` passes.
- [x] 3.5 Split `src/window/components/list_row/tests.rs` into `tests.rs` plus `tests/`, and add
  `tests/branch.rs` and `tests/leaf.rs` with the painted-structure contracts of design D10: a
  branch paints the swatch hook of its sibling, `aria-expanded` and the open mark when open,
  and no mark when it opens nothing; a leaf paints its `data-tree-well` edge and the separator
  only off the first row; an open flat row paints `data-tree-rail="open"`. Verify: `make test
  PKG=mbv-gui` passes and `make check-code-file-lines` passes.

## 4. Music pilot

- [x] 4.1 In `src/window/components/library_list.rs`, paint the album row through `BranchRow`
  (`sibling`, `facts`, `open`, `selected`, `tail`, the same handlers) and the track row through
  `LeafRow` (`well`, `number`, `duration`, the same handlers). Remove the `TreeRow` and guide
  imports. Update the module and function doc comments. Update the expectations in
  `src/window/components/library_list/tests.rs` that named a guide, an inset, or the 30 px
  album height; keep every behaviour assertion. Verify: `make test PKG=mbv-gui` passes.
- [x] 4.2 In `docs/architecture/gui-client.md`, in "The top-level Library row and its art",
  describe the Music tree's new rows: the root rail, the group, the branch row and its swatch,
  the leaf well, and the parts that paint them; mark the guide text as Shows and Playlists only
  until groups 5 and 6. Add a row to the "Blitz gaps" table: an inset `box-shadow` averages the
  corner radii and needs an alpha colour for a soft edge; handling: solid 1 px edge lines
  (design R1). Verify: `rg -n "Leaf well|BranchRow|averages" docs/architecture/gui-client.md`
  prints the new text.

## 5. Shows

- [ ] 5.1 In `src/window/library/shows_list.rs`, give `ShowsSeasonRow` the fields `sibling:
  usize` and `facts: String`, and `ShowsEpisodeRow` the field `well: WellEdge`. In
  `series_shows_rows`, count the season rows of each series from 0; set `facts` per design D5
  (empty while closed or not loaded; `N episodes` from the carried episodes when open and
  loaded); push seasons at `BRANCH_ROW_HEIGHT` and episodes at `well.height()`. Add the sibling
  and well-height contracts to `src/window/library/shows_list/tests.rs` as in task 2.3, and
  update the broken height expectations. Verify: `make test PKG=mbv-gui` passes.
- [ ] 5.2 In `src/window/components/library_shows_list.rs`, paint seasons through `BranchRow`
  (with `open: None` on a season that does not expand) and episodes through `LeafRow` (with the
  episode's play state). Remove the `TreeRow` and guide imports and update the doc comments.
  Update the expectations in `library_shows_list/tests.rs` and its `tests/` that named a guide,
  an inset, or a height. Verify: `make test PKG=mbv-gui` passes.
- [ ] 5.3 In `docs/architecture/gui-client.md`, extend the tree-row text of task 4.2 to the
  Shows All view and its seasons' facts. Verify: `rg -n "season" docs/architecture/gui-client.md`
  shows the Shows text in that section.

## 6. Playlists

- [ ] 6.1 In `src/window/components/playlists_panel.rs`, paint each item through `LeafRow` with
  `well: WellEdge::of(index, items.len())` and `tail` on the last item, and paint the rename
  field's wrapper in the group fill with `TreeRail { tone: Group }` while the playlist is open.
  Remove the `TreeRow`, `TreeGuide`, and guide-constant imports, and update the module doc.
  Update the expectations in `playlists_panel/tests.rs` that named a guide. Verify: `make test
  PKG=mbv-gui` passes.
- [ ] 6.2 In `docs/architecture/gui-client.md`, extend the tree-row text to the Playlists tree
  (the well directly under the open playlist, no branch row). Verify: `rg -n "Playlists" docs/architecture/gui-client.md`
  shows the text in that section.

## 7. Remove the old tree parts

- [ ] 7.1 Delete `src/window/components/list_row/tree.rs` (`TreeRow`, `TreeLevel`, `TreeDepth`,
  `TreeTrail`) and `src/window/components/tree_guide.rs`, their module lines, and their
  re-exports. Delete `NODE_ROW_HEIGHT` and every constant only they used. Rename
  `FlatRowProps::tree_guide` to `tree_group` at every call site. Move any test that still
  asserts an old part's contract to the new part, or delete it when a new test owns the same
  contract. Verify: `rg -n "TreeRow|TreeGuide|TreeDepth|NODE_ROW_HEIGHT|tree_guide" src`
  prints nothing, and `make test PKG=mbv-gui` passes.
- [ ] 7.2 In `assets/app.css`, remove `--canvas-tree-guide` and `--canvas-tree-guide-nested`,
  set `--canvas-tree-subtree` to `#293136` with a comment naming design D6, and run `make
  gui-css`. Verify: `rg -n "canvas-tree-guide" assets src` prints nothing.
- [ ] 7.3 Update the docs: in `docs/architecture/gui-client.md`, remove the guide text and the
  old insets from the tree-row section; in `docs/invariants/30-shared-ui-is-never-bent-to-fit-a-consumer.md`
  and `docs/invariants/38-repeated-gui-shapes-have-one-mechanics-owner.md`, replace `TreeRow`
  and `TreeGuide` with `BranchRow`, `LeafRow`, and `TreeRail`. Verify: `rg -n "TreeGuide|TreeRow\b|indent guide" docs`
  prints nothing.

## 8. Integration gates

- [ ] 8.1 Run `make format-check`, `make checks`, `make test`, and `make check-code-file-lines`
  for the workspace. Fix each failure at its cause. Verify: all four exit 0.

## Workflow follow-up

- After group 4, the user reviews the Music tree live (`make run`) against the mockup before
  group 5 starts; findings change tokens or constants first.
- Sync the delta into `openspec/specs/gui-client/spec.md` and archive the change only when the
  user approves the PR for merge, as the last commit before the merge.
