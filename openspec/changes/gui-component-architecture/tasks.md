# Tasks

Rules for every task. Read `crates/mbv-gui/AGENTS.md`, `proposal.md`, and `design.md` first.
Paths are under `crates/mbv-gui/` unless a path starts at the repository root. Run `make format`
after each Rust edit. Run `make checks PKG=mbv-gui` and `make test PKG=mbv-gui` in the
foreground and wait for them; never run them in the background, never sleep, never poll. A task
may leave the build or tests red only when its text says so and names the task that turns them
green. Add a test only where the task names its contract (`design.md` D13). Remove a
per-destination painter test when its painter goes; do not port it. Before a task deletes a painter test, compare the painted structure the test
guarded with the new base's output, and record in the commit message that the contract is
covered. No test asserts a pixel
position. Use no lint suppression and no `Element` prop on a base. Keep every file under 800
lines. The look does not change: every look value goes through a token (`design.md` D10).
Commit each task with a message that names the task number.

## 1. Neutral bridge vocabulary

- [ ] 1.1 In `src/bridge/frame/list.rs` (new), add `GuiListId`, `GuiRowKey`, `GuiRowRole`
  (`Root`, `Branch`, `Leaf`, `Heading`, `Field`), `GuiRowCellsFrame`, `GuiRowActionsFrame`, and
  `GuiListRowFrame` as `design.md` D6 states, with rows behind `Arc<[_]>`. Verify:
  `make checks PKG=mbv-gui` passes.
- [ ] 1.2 In the same module, add `GuiListFrame`, `GuiListHeaderFrame`, `GuiChoiceFrame`,
  `GuiLettersFrame`, and `GuiListNoticeFrame` (D6). Verify: `make checks PKG=mbv-gui` passes.
- [ ] 1.3 In `src/bridge/frame/hero.rs` (new), add `GuiHeroFrame`, `GuiHeroActionFrame`, `GuiHeroFavouriteFrame`, and
  `GuiHeroBodyFrame::{None, Workspace(GuiListFrame)}` (D5). Verify: `make checks PKG=mbv-gui`
  passes.
- [ ] 1.4 In `src/bridge/intent.rs` (new), add `GuiListIntent`, `GuiListAct`,
  `GuiListHeaderAct`, `GuiHeroIntent`, `GuiHeroAct` (with `SetFavourite(bool)`), and `GuiPointerAct` over the existing
  `RegionTarget` (D7, D8). Route each intent type through the window-to-core channel beside
  `GuiLibraryCommand`; the core answers with a documented no-op for now. Verify:
  `make checks PKG=mbv-gui` passes and `rg -n '_ =>' src/core_thread.rs` shows no wildcard arm
  over the new types.
- [ ] 1.5 Add a neutral Library section to the frame that carries the shown list's
  `GuiListFrame` and the shown `GuiHeroFrame`, with its own revision, beside the old
  per-destination sections. Verify: `make test PKG=mbv-gui` passes.
- [ ] 1.6 In `CONTEXT.md` (repository root), add the GUI terms *List base*, *Library list*,
  *Flat list*, *Tree list*, *Queue list*, *Workspace list*, *Hero base*, *Control base*, *Field
  base*, *Popup base*, *Bar base*, *Settings row*, *Panel base*, *List intent*, with *Avoid*
  lines. Verify: `rg -n 'List base|Hero base|List intent' CONTEXT.md` prints each term.

## 2. Projections into neutral frames

Each task maps every fact of the old per-destination frame to a neutral field first, and lists
that map in the projection module's doc comment. Each task adds projection tests in
`gui_core` for its destination's rows and Hero frame: one test per row role it emits and one
for its Hero body.

- [ ] 2.1 Music list: project the artist, album, and track rows and the letters into a Tree
  `GuiListFrame`. Verify: the new Music projection tests pass.
- [ ] 2.2 Music Hero: project the album Hero (Workspace body) and the artist page (Workspace body
  with `Heading` rows per album) into `GuiHeroFrame`. Verify: the new tests pass.
- [ ] 2.3 Shows: project the All view (series, season, episode, Tree, with `expandable: false`
  seasons) and the New and Upcoming views (Flat). Verify: the new tests pass.
- [ ] 2.4 Shows Hero: project the series and episode Hero, its fact rows, cast, and episodes
  Workspace body. Verify: the new tests pass.
- [ ] 2.5 Home: project the Home rows (Continue watching time left included) and the Home Hero.
  Verify: the new tests pass.
- [ ] 2.6 Emby plain libraries and Favourites: project the rows (the shared Emby play state in
  plain and feed-group mode) and the Emby Hero. Verify: the new tests pass.
- [ ] 2.7 Audiobooks: project the rows and the Hero with its chapters Workspace body. Verify:
  the new tests pass.
- [ ] 2.8 Podcasts: project the rows and the Podcasts Hero. Verify: the new tests pass.
- [ ] 2.9 Feeds: project the rows and the Feeds Hero with its entries Workspace body. Verify:
  the new tests pass.
- [ ] 2.10 Playlists: project the playlists and items as a Tree `GuiListFrame` (playlist `Root`,
  item `Leaf`, the rename draft as a `Field` row under the open playlist). Verify: the new tests
  pass.
- [ ] 2.11 Search: project the inline Search results into a Flat `GuiListFrame`. Verify: the new
  tests pass.
- [ ] 2.12 Fill the neutral Library section of task 1.5 from tasks 2.1 to 2.11 for the shown
  destination. Verify: one core test per destination shows that switching to it sends its list
  and Hero frames.

## 3. Intent resolution in the core

Each task adds a `resolve` function to the destination's projection module that maps a
`GuiListAct` or `GuiHeroAct` to the command the destination runs today, and one test per act
the destination accepts that asserts the resulting command.

- [ ] 3.1 Music: press, activate, toggle, menu, row actions, header acts. Verify: the new tests
  pass.
- [ ] 3.2 Shows (All, New, Upcoming; an Upcoming row resolves no act). Verify: the new tests
  pass.
- [ ] 3.3 Home and Emby (Favourites included). Verify: the new tests pass.
- [ ] 3.4 Audiobooks, Podcasts, and Feeds. Verify: the new tests pass.
- [ ] 3.5 Playlists (open, toggle, rename, delete, row actions, item actions). Verify: the new
  tests pass.
- [ ] 3.6 Search, and the Hero acts of every destination. Verify: the new tests pass.
- [ ] 3.7 Replace the no-op arms of task 1.4 with the resolvers. Verify: `make test PKG=mbv-gui`
  passes.

## 4. Tokens, the drag gesture, and the small bases

- [ ] 4.1 Rewrite `docs/invariants/38-repeated-gui-shapes-have-one-mechanics-owner.md`
  (repository root): the whole-GUI scope, the level rule, the sealed-extension rule, the
  data-only rule (no `Element` prop on a base), no rule 3 exception, and a "Where it currently
  fails" list with one line per surface that tasks 5 to 11 have not moved yet. Each later task
  that moves a surface removes its line. Verify: the file names D1's levels and the list holds
  one line per un-moved surface.
- [ ] 4.2 Add the token names of D10 to `assets/app.css` with today's values, one name per
  mechanic, and the level-root overrides that keep the Queue's and Workspace's current values.
  Run `make gui-css`. Verify: `make test PKG=mbv-gui` passes and the window's stylesheet tests
  find each token.
- [ ] 4.3 Add `src/window/components/gesture.rs` (D9) and the root `onpointermove`,
  `onpointerup`, and `onpointerleave` handlers in the app root. Add contract tests: motion past
  the threshold starts a drag, a release drops it, a new press cancels it, a leave cancels it, Esc cancels it and is consumed.
  Verify: the new tests pass.
- [ ] 4.4 Add CONTROL BASE (`control.rs`, `control/`) with button, icon button, text button,
  tab, pill, switch, checkbox, pick button, and hover action variants, and their contract tests
  (press intent, disabled, hover look hook). Verify: the new tests pass.
- [ ] 4.5 Move the Library and Hero buttons onto CONTROL BASE (`header_icon_button`,
  `library_hero_actions`, `library_sort_control`, `library_top_bar/tabs`, `prefix_badge`,
  `clear_field_button`). Delete the old components. Verify: `make test PKG=mbv-gui` passes.
- [ ] 4.6 Move the Queue and player buttons onto CONTROL BASE (`queue_target_control`,
  `track_chips` chips, the transport and fold controls). Verify: `make test PKG=mbv-gui` passes.
- [ ] 4.7 Move the Settings controls onto CONTROL BASE (`settings_button`, `settings_switch`,
  `checkbox_box`, `settings_state_pill`, `settings_pick_button`). Verify: `make test
  PKG=mbv-gui` passes. The crate-wide `button {` check holds only after task 4.14 and runs as
  task 12.1's gate; here, `rg -l '^\s*button \{' src/window/components --glob 'settings*'
  --glob '!**/tests*'` prints only files that tasks 4.13 moves (`settings_entry_row`,
  `settings_checkbox_grid`, `settings_sidebar`, `settings_feeds`).
- [ ] 4.8 Add FIELD BASE (`field.rs`, `field/`) with its contract tests (draft, clear, commit,
  cancel, focus while open), and move `library_filter_field`, `library_search_control`,
  `settings_field`, and the `library_dropdown` filter onto it. Verify: `make test PKG=mbv-gui`
  passes and `rg -l '^\s*input \{' src/window --glob '!**/field/**' --glob '!**/tests*'` prints
  nothing except `playlists_panel.rs`, which task 7.4 moves.
- [ ] 4.9 Add POPUP BASE with MENU and DIALOG (`popup.rs`, `popup/`) and their contract tests
  (anchor, outside press closes, the topmost entry takes the press, entry intent). Verify: the
  new tests pass.
- [ ] 4.10 Move `queue_row_menu`, `chooser_menu`, `queue_chooser_menu`, `menu_card`,
  `library_dropdown`, `library_collection_dropdown`, `library_view_dropdown`, and the
  `track_chips` popover onto MENU. Delete the old components. Verify: `make test PKG=mbv-gui`
  passes.
- [ ] 4.11 Move `dialog_frame`, `confirm_overlay`, `daemon_lost_overlay`, and `attach_screen`
  onto DIALOG. Verify: `make test PKG=mbv-gui` passes.
- [ ] 4.12 Add BAR BASE (`bar.rs`, `bar/`) with its contract test (lead, title, trail slots as
  data), and move `library_top_bar`, `library_list_header`, `queue_header`,
  `library_status_bar`, `settings_status_bar`, and `queue_footer` onto it. Verify: `make test
  PKG=mbv-gui` passes.
- [ ] 4.13 Add SETTINGS ROW (`settings_row.rs`) with its contract test, and move `settings_row`,
  `settings_rows`, `settings_entry_row`, `settings_card`, and `settings_checkbox_grid` onto it;
  `settings_feeds`, `settings_services`, `settings_sessions`, `settings_keys`,
  `settings_library`, `settings_sidebar`, and `settings_screen` become composites. Verify:
  `make test PKG=mbv-gui` passes.
- [ ] 4.14 Add PANEL BASE (`panel.rs`, `panel/`) and move `panel`, `panel_divider`,
  `collapsed_rail`, and `compact_strip` onto it. Verify: `make test PKG=mbv-gui` passes.

## 5. LIST BASE

- [ ] 5.1 Add `list.rs` and `list/` with the sealed `ListSource` and `RowExtra` traits (D2) and
  `ListCaps`. Verify: `make checks PKG=mbv-gui` passes.
- [ ] 5.2 Move the viewport into LIST BASE: the virtual window, spacers, measure, offset, and
  rows-in-view reports (from `library_virtual_list`, `library_list_column`,
  `window/library/list/geometry.rs`), with `GeometryRow` heights keyed by role. Contract tests:
  the window holds the rows in view plus the overscan, the spacers hold the range, a resize
  keeps the selected row in view. Verify: the new tests pass.
- [ ] 5.3 Move the row host, roles, cell layout, play marks, fills, and divider into LIST BASE
  (from `list_row`, `list_row/flat`, `list_row/tree`, `list_row/thumb`, `tree_row`,
  `tree_guide`). Contract tests: one per role. Verify: the new tests pass.
- [ ] 5.4 Move hover, the hover-actions slot (`row_hover_actions`), the menu intent, the
  keyboard reveal (`cursor_reveal`), and the notices (`list_frame_notices`,
  `library_list_notice`) into LIST BASE. Contract tests: hover actions paint on the hovered
  row, a context press sends `Menu`, a notice replaces the rows. Verify: the new tests pass.
- [ ] 5.5 Add the `reorder` and `active_row` capabilities over the gesture of task 4.3.
  Contract tests: a drop sends `Reorder(key, anchor)`, the ghost and insertion line paint only
  during a drag, the active row paints its fill. Verify: the new tests pass.

## 6. LIBRARY LIST and FLAT

- [ ] 6.1 Add LIBRARY LIST (`list/library.rs`): `LibraryListSource` over `GuiListFrame`, the art
  scope, the filter and sort hookup, the list header BAR, and the `letters` capability (letter
  headings and A-Z rail from `library_letter_heading` and `library_letter_rail`). Contract
  tests: letters paint under a name sort only, a rail press scrolls to the letter. Verify: the
  new tests pass.
- [ ] 6.2 Replace `window/library/state.rs` and its per-destination parts with one `ListStates`
  map keyed by `GuiListId` (scroll, hover, open nodes, filter text, rename draft) (D11).
  The build may stay red until task 6.4. Verify: `make checks PKG=mbv-gui` shows only errors at
  the old destination call sites.
- [ ] 6.3 Replace `window/library/keys.rs`, `keys_shared.rs`, and the five `*_keys.rs` files with
  one LIBRARY LIST key map that emits `GuiListAct` for the pointer row (D7). The
  `PointerList` destination variants stay until task 9.1, because the old painters of tasks
  6.4, 7.x, and 8.3 still write them. Contract test: one per key act. Verify: the new tests
  pass.
- [ ] 6.4 Add FLAT (`list/library/flat.rs`) and mount it for Home, Emby (Favourites included),
  Audiobooks, Podcasts, Feeds, Search, and Shows New and Upcoming from the neutral section.
  Delete `library_lettered_list`, `library_home_list`, `library_favourites_list`, `window/library/lettered*`,
  `window/library/home*`, and the matching per-destination window files. Remove their lines from
  invariant 38's failure list. Verify: `make test PKG=mbv-gui` passes.

## 7. TREE

- [ ] 7.1 Add TREE (`list/library/tree.rs`): expand and collapse, Expand all and Collapse all,
  the rails and guides, child loads on open (a `Toggle` intent), and the `Field` row over FIELD
  BASE. Contract tests: a toggle opens and sends `Toggle`, Collapse all closes every level, the
  `Field` row commits `Rename`. Verify: the new tests pass.
- [ ] 7.2 Mount TREE for Music. Delete `library_list.rs` and the Music parts of
  `window/library/`. Verify: `make test PKG=mbv-gui` passes.
- [ ] 7.3 Mount TREE for the Shows All view. Delete `library_shows_list.rs`,
  `window/library/shows*`, and `window/library/episode.rs`. Verify: `make test PKG=mbv-gui`
  passes.
- [ ] 7.4 Mount TREE for Playlists inside a Playlists screen composite (PANEL + BAR + TREE).
  Delete `playlists_panel.rs` and its directory. Remove the three tree lines from invariant 38's
  failure list. Verify: `make test PKG=mbv-gui` passes and `rg -n 'TreeRow|FlatRow'
  src/window --glob '!**/list/**'` prints nothing.

## 8. WORKSPACE and HERO BASE

- [ ] 8.1 Add WORKSPACE (`list/workspace.rs`): `WorkspaceSource`, the column grid, the header
  row, the playing bars in the number cell, and `Heading` rows. Contract tests: the playing row
  paints the bars in place of its number, a press on the header row does nothing, a press on a data row sends `Press`, a double-click
  activates, a heading takes no press. Verify: the new tests pass.
- [ ] 8.2 Add HERO BASE (`hero.rs`, `hero/`) over `GuiHeroFrame`, built from
  `library_hero_frame`, `library_hero_art`, `library_hero_cast`, `library_hero_detail`,
  `library_hero_fact_rows`, `library_hero_section`, and `library_hero_pills`, with actions on
  CONTROL BASE and the body on WORKSPACE. Contract tests: no body paints no list band, a titled
  body paints its section header, an action press sends its `GuiHeroAct`. Verify: the new tests
  pass.
- [ ] 8.3 Mount HERO BASE for every destination. Delete `library_hero`, `library_home_hero`,
  `library_shows_hero`, `library_hero_heart`, `library_emby_hero`, `library_favourites_hero`, `library_audiobooks_hero`,
  `library_podcasts_hero`, `library_feeds_hero`, `library_workspace_table`,
  `library_tracklist`, `library_artist_tracklist`, and `window/library/*_workspace.rs` and
  `window/library/hero.rs`. Remove the Hero lines from invariant 38's failure list. Verify:
  `make test PKG=mbv-gui` passes.

## 9. LIBRARY composite and the neutral cut-over

- [ ] 9.1 Make `library.rs` and `library/` (D1) the LIBRARY composite (BAR + LIST + HERO + BAR)
  that mounts one list and one Hero from the neutral section, with no match on the destination;
  `library_panel.rs` moves into it. Now that no old painter writes a `PointerList` variant,
  replace the destination variants of `PointerList` in `window/pointer_target.rs` with a
  `GuiListId` token (D8). Verify: `rg -n 'PointerList::(Music|Home|Shows|Emby|Favourites|Audiobooks|Podcasts|Feeds|Tracklist|Workspace)' src` and `rg -n 'Music|Shows|Emby|Favourites|Audiobooks|Podcasts|Feeds' src/window/pointer_target.rs` print nothing, and `rg -n 'GuiLibraryView::|Destination'
  src/window/components/library.rs src/window/components/library` prints no destination match.
- [ ] 9.2 Delete the per-destination frames in `src/bridge/frame/library/` and
  `src/bridge/frame/playlists.rs` that the neutral frames replace, and their sections
  (`window/sections.rs` `playlists` included). Verify:
  `make test PKG=mbv-gui` passes.
- [ ] 9.3 Reduce `GuiLibraryCommand` and `GuiPlaylistsCommand` to the acts that are not list or
  Hero acts (D7), and delete the window functions that built the removed variants. Verify:
  `make test PKG=mbv-gui` passes and `rg -n 'press_shows_row|toggle_library_node|play_library_track'
  src` prints nothing.
- [ ] 9.4 Verify that the Library path is destination-free: `rg -n -i
  'music|shows|emby|favourite|audiobook|podcast|feeds|playlist' src/window/components/library.rs
  src/window/components/library src/window/components/list src/window/components/hero
  src/window/library src/window/pointer_target.rs src/bridge/frame --glob '!**/tests*'` prints
  only token names, icon names, and comments. Settings, Sessions, brand icons, and the Queue
  output choosers are outside this check. Verify: the command's output holds no type, function,
  or match arm that names a destination.

## 10. QUEUE and the single input system

- [ ] 10.1 Add QUEUE (`list/queue.rs`): `QueueSource` with `QueueSlotId` keys, the ordinal, the
  progress ring, the now-playing and marked fills, `reorder`, and `active_row`. Contract tests:
  the drop sends the move act with the slot and the insertion point, the hover Play control
  replaces the duration. Verify: the new tests pass.
- [ ] 10.2 Mount QUEUE in the Queue panel and the folded queue. `GuiQueue.rows` becomes the full list and the core stops cropping rows to its viewport.
  The window reports the Queue's
  rows in view, and the core uses that report for page keys and the wheel-follow rule. The
  build may stay red until task 10.4. Delete `queue_panel/row.rs`, `row_cells.rs`, and
  `geometry.rs`. Verify: `make checks PKG=mbv-gui` shows only errors at the region-set sites.
- [ ] 10.3 Move the scrubber seek onto the gesture of task 4.3. Contract tests: a press seeks, a drag
  over the bar seeks, a drag off the bar seeks no more, a leave ends the drag. Verify: the new test passes.
- [ ] 10.4 Give every component that reports a region set a Dioxus handler that sends
  `GuiPointerAct` with its painted `RegionTarget` (D8): the player panel, the Status bar, the
  dialogs, the attach screen, the compact strip, the collapsed rail, and the track chips. In the
  core, route `GuiPointerAct` to the per-target handling the hit map reaches today: rewrite
  `QueueLeaf`, `PlaybackLeaf`, and `StatusBarLeaf` in `src/leaves.rs` to take the act's
  `RegionTarget` (and the scrubber's `Seek(f32)`) instead of a point, and rewrite
  `attach.rs`'s `mismatch_pointer` to map the `Attach(Restart | Quit)` target to its
  `PromptOutcome` (D8). Contract tests: each leaf emits today's `Msg` for its target, and a
  mismatch-prompt target maps to Restart or Quit. Verify: `make test PKG=mbv-gui` passes.
- [ ] 10.5 Delete `window/layout/region_set.rs`, every region-set report, the hit-map path in
  `core_thread/pointer.rs`, `bridge/layout.rs`'s `hit_map`, the `HitMap` and
  `PointerGestureState` fields of `attach.rs`, and the GUI's use of `HitMap`,
  `PointerGestureState`, and the `QueueSurface` viewport. Remove `hover`, `drag`, and `scroll` from `GuiQueue`. Remove the input
  lines from invariant 38's failure list. Verify: `make test PKG=mbv-gui` passes and
  `rg -n 'region_set|RegionSet|HitMap|PointerGestureState' src` prints nothing.
- [ ] 10.6 Verify that `mbv-surfaces` and the TUI did not change: `git diff --stat origin/main --
  crates/mbv-surfaces crates/mbv-tui crates/mbv-client-core` (repository root) prints nothing.

## 11. Docs

- [ ] 11.1 Update `docs/invariants/30-shared-ui-is-never-bent-to-fit-a-consumer.md` (repository
  root) to name the bases of D1 and the data-only rule. Verify: the file names each base.
- [ ] 11.2 Finish invariant 38: its failure list is empty, and "How the code upholds it today"
  names each level and its module. Verify: the file names every D1 module and lists no failure.
- [ ] 11.3 Update `docs/architecture/gui-client.md` and `crates/mbv-gui/AGENTS.md` (repository
  root) for the level tree, the neutral frames and intents, the single input system, and the
  list state map. Verify: neither file names a removed component, region set, or
  per-destination frame (`rg -n 'region set|LibraryShowsList|LibraryHeroFrame' docs/architecture/gui-client.md crates/mbv-gui/AGENTS.md`
  prints nothing).

## 12. Integration

- [ ] 12.1 Run the structural checks of tasks 4.7, 4.8, 7.4, 9.4, and 10.5 again, and run
  `rg -n 'Element' src/window/components --glob '!**/tests*'` and confirm that no base's props
  struct holds an `Element` field. Verify: each check prints nothing to fix.
- [ ] 12.2 Run `make format-check`, `make checks`, `make test`, and `make check-code-file-lines`
  (repository root). Verify: all four pass.

## Workflow follow-up

- Before this change starts, PR 1075 (`emby-favourites`) merges.
- `add-gui-pin-window` and `gui-tree-row-roles` stay on hold until this change merges, then
  rebase onto the bases.
- Sync the delta into `openspec/specs/gui-client/spec.md` and archive the change only when the
  user approves the PR for merge, as the last commit before the merge.
