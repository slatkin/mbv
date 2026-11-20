# Tasks

Assumes this change merges before `gui-delineated-rows` (design D4). If
`gui-delineated-rows` is already on `main` when work starts, do task 5.1 in
place of the artist-row parts of 2.2 and the series-row parts of 3.2.

## 1. The shared hint part

- [x] 1.1 Add `crates/mbv-gui/src/window/components/tree_hint.rs` with `TreeHint { open: bool }` (design D3): one span, class `tree-hint`, mark `data-tree-hint`, muted canvas tone, 12 px, `flex-none`, no wrap, text `Double-click to expand` when closed and `Double-click to collapse` when open. Register it in `components.rs`. Verify: a `#[case]` test (closed, open) in `tree_hint/tests.rs` checks the text and the mark, and `cargo nextest run -p mbv-gui tree_hint` passes.
- [x] 1.2 Add the hover rule to `crates/mbv-gui/assets/app.css` (design D3): `.tree-hint` hidden, `.tree-parent:hover .tree-hint` shown, `.tree-parent:hover .tree-trailing` hidden, with a comment that names this change and design D3. Regenerate the committed `app.generated.css` with `make gui-css` (Tailwind CLI v4.3.5, as the Makefile names it). Verify: `make gui-css` succeeds, `rg -c "tree-hint" crates/mbv-gui/assets/app.generated.css` finds the rule, and `cargo check -p mbv-gui` passes.

## 2. Music list

- [x] 2.1 In `crates/mbv-gui/src/window/library/state/rows.rs`, make `press_library_row`'s artist and album arm call only `show_detail`, and add `toggle_library_node(&LibraryRow)` that calls `set_node_open(row, !open)` for an artist or album row and does nothing for other rows (design D1). Update the doc comments. Verify: `cargo check -p mbv-gui`.
- [x] 2.2 In `crates/mbv-gui/src/window/components/library_list.rs`, remove the chevron span and the `ChevronRight` import from the artist and album row. Bind `ondoubleclick` to `toggle_library_node`. Add class `tree-parent` to the row button, class `tree-trailing` to the album year span, and `TreeHint { open }` as the last child. Change `row_padding` to the design D2 insets (artist 8 px, album 26 px, track 44 px). Verify: `cargo check -p mbv-gui`.
- [x] 2.3 Update `crates/mbv-gui/src/window/components/library_list/tests.rs` (design D5): the album-open test opens through `toggle_library_node` and still checks the tracks request; add `a_press_on_an_album_picks_it_and_leaves_it_closed` (the Hero detail command is sent and `aria-expanded` stays `false`); the filter test toggles through `toggle_library_node`; the `open_beastie` and `open_licensed_to_ill` helpers open through the toggle. Verify: `cargo nextest run -p mbv-gui library_list` passes.

## 3. Shows list

- [x] 3.1 In `crates/mbv-gui/src/window/library/shows.rs`, make `press_shows_row` call only `select_shows_row` (design D1) and update its doc comment. Verify: `cargo check -p mbv-gui`.
- [x] 3.2 In `crates/mbv-gui/src/window/components/library_shows_list.rs`, remove `paint_chevron`, its `data-library-shows-chevron` mark, and the empty 18 px spacer. For an expandable series or season row, bind `ondoubleclick` to `toggle_shows_node`, add class `tree-parent`, and add `TreeHint { open }` as the last child; a row that is not expandable gets none of these. Change `row_inset` to the design D2 insets (series 8 px, season 26 px, episode 44 px). Verify: `cargo check -p mbv-gui`.
- [x] 3.3 Update `crates/mbv-gui/src/window/components/library_shows_list/tests.rs` (design D5): split `a_press_on_a_series_node_opens_it_and_selects_it` into a press that selects and leaves the node closed and a `toggle_shows_node` call that opens it; make `an_expandable_node_paints_its_open_state_as_aria_expanded` open through the toggle; remove any assertion on the chevron mark. Update `crates/mbv-gui/src/window/library/shows_keys/tests.rs` only where a test opened a node by a press. Verify: `cargo nextest run -p mbv-gui shows` passes.

## 4. Playlists screen

- [ ] 4.1 In `crates/mbv-gui/src/window/components/playlists_panel.rs`, add `fn toggle_command(is_open: bool, id: &str) -> GuiPlaylistsCommand` (`Collapse` when open, `Open { id }` when closed). Make the playlist row's `onclick` only set the pick, and its `ondoubleclick` send `toggle_command(..)` in place of `Play` (design D1). Remove the chevron span and the `ChevronDown` and `ChevronRight` imports. Add class `tree-parent` and `aria-expanded` to the row button, class `tree-trailing` to the item-count span, and `TreeHint { open: is_open }` as the last child. Update the module doc comment. Verify: `cargo check -p mbv-gui`.
- [ ] 4.2 Add a `#[case]` table to `crates/mbv-gui/src/window/components/playlists_panel/tests.rs` for `toggle_command` with two cases: closed sends `Open { id }`, open sends `Collapse`. Verify: `cargo nextest run -p mbv-gui playlists_panel` passes.

## 5. Merge order with `gui-delineated-rows`

- [x] 5.1 Only if `gui-delineated-rows` is on `main` (artist and series rows paint through `LibraryItemRow`): add `tree_open: Option<bool>` and `on_toggle: Option<Callback<()>>` props to `crates/mbv-gui/src/window/components/library_item_row.rs` (design D4). With `Some(open)`, the part adds class `tree-parent` and `aria-expanded`, marks its trailing text `tree-trailing`, paints `TreeHint { open }`, and its double-click calls `on_toggle` in place of `on_play`. Pass them from the Music artist row and the Shows series row. If `gui-delineated-rows` is not on `main`, mark this task done with the note "not needed: this change merged first". Verify: `cargo nextest run -p mbv-gui library` passes.

## 6. Docs and gates

- [ ] 6.1 Update `docs/architecture/gui-client.md` where it describes the tree rows' press, chevron, or double-click, so it states design D1 to D3; keep the Right and Left key text as it is. Verify: `rg -n -i "chevron" docs/architecture/gui-client.md` names no tree-row chevron.
- [ ] 6.2 Run `cargo fmt`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo nextest run -p mbv-gui`. Verify: all three pass with no new warning.

## Workflow follow-up

- Sync the `gui-client` delta into `openspec/specs/` and archive the change only when the user approves the PR for merge, as the last commit before the merge.
