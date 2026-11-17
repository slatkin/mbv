# Tasks

## 1. Root row: Library panel left, Queue column right

- [x] 1.1 In `crates/mbv-gui/src/window/app.rs`, in the non-Settings branch of the root `div`
  (`data-arrangement`), paint the children in the order `LibraryPanel`, Queue `PanelDivider`,
  Queue column `div` (`data-queue-column`). Keep each child's condition (`library_visible &&
  has_library_list`, `two_columns && !panel.collapsed`, `queue_visible`) and props as they are.
  Change the Queue divider to `side: DividerSide::Right` (design D1, D2). Update the comments on
  these children that name a side. Verify: `cargo check -p mbv-gui`.
- [x] 1.2 In `crates/mbv-gui/src/window/app/tests/resize.rs`, reorder the copied root in
  `resize_app` so the Library column `div` comes before the Queue column `div`, to match
  `app.rs` (design D7). Verify: `cargo nextest run -p mbv-gui resize` passes.

## 2. Wide Library panel: list left, Hero pane right

- [x] 2.1 In `crates/mbv-gui/src/window/components/library_panel.rs`, in the content row, paint the
  list (`data-library-region="list"`) first, then the list `PanelDivider` with
  `side: DividerSide::Left`, then the Wide Hero pane (`data-library-region="hero"`). Change the
  Wide list's `border-l` class to `border-r`. Update the comments that say the Hero pane is "to
  the left of" the list (design D1, D2, D3). Verify: `cargo check -p mbv-gui`.
- [x] 2.2 In `crates/mbv-gui/src/window/components/library_panel/tests/destinations.rs`, change
  `the_panel_paints_the_top_bar_the_pill_row_and_the_hero_above_the_list_region` to assert the new
  region order (list before Hero) and rename it to match (design D7). Verify: the test passes.
- [x] 2.3 In `crates/mbv-gui/src/window/components/library_panel/tests/press.rs`, add one test
  that owns the spec scenario "The Wide list is at the left window edge": in a Wide fixture with a
  picked item, the list region's box (`rect_of("[data-library-region=\"list\"]")`) ends at or left
  of the Hero region's box start (design D7). Verify: `cargo nextest run -p mbv-gui library_panel`
  passes.

## 3. Collapsed rail expand icon

- [x] 3.1 In `crates/mbv-gui/src/window/icons.rs`, add the `ChevronsLeft` component as a port of
  Lucide `chevrons-left` from `@lucide/svelte` 1.70.0 (name `chevrons-left`, paths
  `m11 17-5-5 5-5` and `m18 17-5-5 5-5`), in the same form as the existing `ChevronsRight`. In
  `crates/mbv-gui/src/window/components/collapsed_rail.rs`, paint `ChevronsLeft` for the expand
  control with the same props. Delete `ChevronsRight`, which then has no caller, and keep the
  module comment's icon count correct (design D4). Verify: `cargo clippy -p mbv-gui --all-targets
  -- -D warnings` is clean and `rg -n "ChevronsRight|chevrons-right" crates/mbv-gui` finds nothing.

## 4. Docs

- [x] 4.1 In `docs/architecture/gui-client.md`, state the column order where the file places the
  panels: the Library panel column at the left window edge and the Queue column at the right
  window edge (near line 673, "a column beside the Queue panel"); the Wide list at the left edge of
  the Library panel with the Hero pane to its right; the collapsed rail at the right edge. Change
  the Settings text near line 1282 ("the sidebar sits in the Queue column's place") to say the
  sidebar sits at the left window edge. Verify: `rg -n -i "Queue column's place" docs/architecture/gui-client.md`
  finds nothing.
- [x] 4.2 In `crates/mbv-gui/src/window/components/panel_divider.rs`, check that the module and
  `DividerSide` comments name no specific column on a specific side; if a comment says which
  column sits on which side, update it to the new order. Verify: `cargo fmt --all -- --check` and
  `cargo nextest run -p mbv-gui` pass.

## 5. Integration

- [x] 5.1 Run `cargo fmt`, `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo nextest run -p mbv-gui`, and `openspec validate gui-swap-queue-and-library-sides --strict`.
  Verify: all pass.

## Workflow follow-up

- After the user approves the PR for merge, sync the delta specs into `openspec/specs/` and archive
  the change as the last commit before the merge.
