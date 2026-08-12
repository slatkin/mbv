## 0. Reference material

Old (pre-modal) code is at commit `1c608c7`, the parent of `03a725b`. Read it with
`git show 1c608c7:<path>`, never `git revert` — the old code carries two bugs this
change must not reintroduce (see `design.md` Decisions 5 and 6).

- `git show 1c608c7:src/app/types_browse.rs` — `LibSearch` struct
- `git show 1c608c7:src/app/input_lib_power_keys.rs` — `/` handler, `handle_lib_search_key`
- `git show 1c608c7:src/app/library_load_actions.rs` — `update_lib_search` (line ~331)
- `git show 1c608c7:src/app/library_search_actions.rs` — `open_recursive_album_search`,
  `sync_recursive_album_search`, `recursive_album_search_entry`, `activate_recursive_album`
- `git show 1c608c7:src/app/library_browse_actions.rs` — `spawn_search_items_load` (line ~577)
- `git show 1c608c7:src/app/render/list.rs` — search input box, `show_grouped` guard
- `git show ce1daa5:src/app/input.rs` — `handle_lib_search_key` **with** its navigation
  keys (line ~241). This is the version to restore, not `1c608c7`'s.

## 1. Remove the search modal

- [x] 1.1 Delete `src/app/render/overlays/search_modal.rs` and its `mod search_modal;`
      line in `src/app/render/overlays/mod.rs`.
- [x] 1.2 Delete `src/app/input_search_modal_keys.rs`, but first copy out
      `SEARCH_DEBOUNCE_MS`, `dispatch_search_modal_query_if_global`,
      `move_search_modal_cursor`, `cycle_search_modal_type_filter`,
      `search_modal_backspace`, `search_modal_append_char`, `dismiss_search_modal`,
      and `activate_search_result` — these are the basis for task 4.2.
- [x] 1.3 In `src/app/render/mod.rs`, delete the `if self.search_modal.is_some()`
      render call (lines ~234-236) and the `|| self.search_modal.is_some()` term in
      `any_dim_modal_open` (line ~250).
- [x] 1.4 In `src/app/input_resolver.rs`, delete the `search_modal` `ContextEntry`
      (lines ~191-192). Leave the gap; task 4.4 refills it.
- [x] 1.5 In `src/app/input_lib_power_keys.rs`, delete `handle_search_key` (the
      promotion dispatcher at the top of the file) and its `Instant` import if it
      becomes unused. Remove the `/` call site that reaches it.
- [x] 1.6 In `src/app/library_search_actions.rs`, delete `open_search_modal_fuzzy`,
      `open_search_modal_global`, and `fill_search_modal_corpus_from_album_index`.
      Keep `library_tabs_for_nav` — the sidebar still needs it.
- [x] 1.7 In `src/app/app_struct.rs`, delete the `search_modal`,
      `search_modal_prior_focus`, and `last_slash_at` fields (lines ~198-201). Before
      deleting `last_slash_at`, confirm with `rtk grep -rn last_slash_at src/` that
      the promotion gesture was its only consumer.
- [x] 1.8 Remove the corresponding initialisers in `src/app/construct.rs` and the
      `App` struct literal in `src/app/tests.rs` (~line 248). Delete the `SearchModal`
      / `SearchMode` re-exports in `src/app/mod.rs`.
- [x] 1.9 Verify: `rtk cargo check` reports no reference to `SearchModal`, `SearchMode`,
      `search_modal`, or `last_slash_at`. `src/app/search_modal.rs` still exists at
      this point — task 4.1 renames it.

## 2. Restore inline library search state and scoring

- [x] 2.1 Add `LibSearch` back to `src/app/types_browse.rs`, above `AlbumPathPart`,
      with fields `query: String`, `items: Vec<MediaItem>`, `results: Vec<usize>`,
      `cursor: usize`, `scroll: usize`, `loading: bool`. Copy from
      `git show 1c608c7:src/app/types_browse.rs`. Re-export it from `src/app/mod.rs`.
- [x] 2.2 Add `search: Option<LibSearch>` back to `LibraryTab` in
      `src/app/types_library_tab.rs` and initialise it to `None` at every
      `LibraryTab { .. }` construction site (`cargo check` will list them).
- [x] 2.3 Restore `update_lib_search` in `src/app/library_load_actions.rs` verbatim
      from `1c608c7` (line ~331). It scores with `SkimMatcherV2` against
      `AlbumSearchEntry::search_text` when the library has a ready album index, and
      against `item.display_name()` otherwise, sorting by descending score.
- [x] 2.4 Restore `spawn_search_items_load` in `src/app/library_browse_actions.rs`
      from `1c608c7` (line ~577) — the full-library unfiltered fetch that fires when
      search opens over a partly-paged or letter-filtered level.
- [x] 2.5 Restore `open_recursive_album_search`, `sync_recursive_album_search`,
      `recursive_album_search_entry`, and `activate_recursive_album` in
      `src/app/library_search_actions.rs` from `1c608c7`, plus the
      `if refresh { self.sync_recursive_album_search(lib_idx) }` line that
      `start_album_index` lost.
- [x] 2.6 Restore the `search.is_none()` guards in `src/app/lib_cursor_actions.rs`
      (`current_library_columns`, `move_lib_cursor_rows`, `move_lib_cursor`) and the
      `search` branch in `current_lib_item` / `lib.search = None` reset in
      `src/app/actions.rs`, from `1c608c7`. These are what make the cursor and
      activation resolve through the result list.
- [x] 2.7 Restore the `AlbumIndexBuilt` arm in `handle_lib_event` and the
      `SearchItemsLoaded`-equivalent `LibEvent` variant in `src/app/types_events.rs`
      if `03a725b` removed one (`git show 03a725b -- src/app/types_events.rs`).
- [x] 2.8 Verify: `rtk cargo check` passes. No rendering or key handling yet.

## 3. Restore inline search rendering and keys, with the two fixes

- [x] 3.1 In `src/app/render/list.rs`, restore the search-results branch that picks
      `(items, cursor, scroll, total)` from `lib.search` when present, mapping each
      result index through `recursive_album_display_item` (from `1c608c7` line ~111).
- [x] 3.2 Restore the three-row bordered search input box drawn at the top of the
      content area when `focused && library_tab > 0 && search.is_some() &&
      content_area.height >= 3`, shrinking the list area by 3 rows and assigning
      `layout.left_area` to the shrunk area. Show `"{query}█ [loading…]"` while
      `search.loading`, `"{query}█"` otherwise. From `1c608c7` line ~215.
- [x] 3.3 Restore the `"Indexing music library..."` empty-state message for
      `recursive_album_search_enabled && search.loading`, and the `search.scroll =
      final_offset` write-back at the end of the function (from `1c608c7` line ~463).
- [x] 3.4 **Grouped-music fix.** Add the missing guard to `show_grouped`
      (`render/list.rs:328-332`) so it is false while search is active:
      `self.is_viewing_album_folders(lib_idx) && self.libs[lib_idx].search.is_none()`.
      See `design.md` Decision 6 for why — the `GroupedAlbumCatalog` indexes the
      unfiltered vector.
- [x] 3.5 Restore the `&& self.libs[lib_idx].search.is_none()` clause in
      `use_letter_groups` (`render/list.rs:358-364`).
- [x] 3.6 Restore the `/` handler in `src/app/input_lib_power_keys.rs`'s
      `handle_lib_key` from `1c608c7` line ~273: try `open_recursive_album_search`
      first, else build the corpus from `nav_stack.last()` (`all_items` or `items`),
      set `needs_full_load` when `all_items.is_none() && (letter_filter.is_some() ||
      items.len() < total_count)`, construct `LibSearch`, call
      `spawn_search_items_load` when loading, then `update_lib_search`. Keep the
      letter-filter comment — it explains a non-obvious condition.
- [x] 3.7 Restore `handle_key_power_lib_search` from `1c608c7` line ~39 (guards:
      no ALT/CTRL, no context menu, `PanelFocus::Library`, `library_tab != 0`;
      falls through for `Tab`/`BackTab` and for `Enter` when a series item is
      selected).
- [x] 3.8 **Navigation fix.** Write `handle_lib_search_key` with the key set from
      `ce1daa5` (`git show ce1daa5:src/app/input.rs`, line ~241), NOT from `1c608c7`:
      `Esc` closes; `Backspace` pops or closes on empty; `Up`/`Down` →
      `move_lib_cursor(∓1)`; `PageUp`/`PageDown` → `move_lib_cursor` by
      `lib_page_size()`; `Home`/`End` → `jump_lib_cursor(false/true)`; `Enter` →
      `select()`; `Char(c)` appends and calls `update_lib_search`; everything else
      swallowed. Do not carry over `ce1daa5`'s `tab_idx` save/restore — that field
      no longer exists.
- [x] 3.9 Add the `lib_search` `ContextEntry` to `CONTEXT_STACK` in
      `src/app/input_resolver.rs`, positioned as at `1c608c7` (above
      `panel_mode_cycle_x` / `sidebar_toggle_x`, below the modal contexts).
- [ ] 3.10 Verify: `rtk cargo build`, then manually — `/` on a library tab opens the
      box, typing filters, Up/Down move the selection, Enter opens the item, Esc
      restores the list. `/` on the home tab does nothing.
- [ ] 3.11 Verify the fix from 3.4 specifically: `/` in a music library at the
      album-folder level shows a flat, correctly-labelled result list with no artist
      headers, and each row is the album that matched.

## 4. Build the global search sidebar

- [x] 4.1 `git mv src/app/search_modal.rs src/app/search_sidebar.rs`. Rename
      `SearchModal` → `SearchSidebar` and `SearchModalDrainOutcome` →
      `SearchDrainOutcome`. Delete the `SearchMode` enum, the `mode` and `corpus`
      fields, `score_corpus_against` and its `fuzzy_matcher` imports, the fuzzy branch
      of `on_query_changed`, and the mode guard at the top of `apply_drain`. Keep
      `is_navigable_type`, `available_types`, `filtered_results`, `filtered_count`,
      `type_sort_key`, and `apply_drain`'s stale-query guard unchanged. Update the
      `#[cfg(test)]` block at the bottom to match.
- [x] 4.2 Create `src/app/input_search_sidebar_keys.rs` with
      `handle_key_search_sidebar`: returns `None` when `search_sidebar.is_none()`,
      otherwise `Some(false)` for every key. Bindings: `Esc` → dismiss; `Enter` →
      `activate_search_result`; `Up`/`Down` → cursor; `Tab`/`BackTab` →
      `cycle_search_sidebar_type_filter(±1)`; `Backspace` → pop or dismiss on empty;
      `Char(c)` → append. Port the bodies from the copies taken in task 1.2, dropping
      every mode check and the whole `/` promotion arm. Keep `SEARCH_DEBOUNCE_MS = 300`
      and the `query.len() < 2` gate in `dispatch_search_sidebar_query`.
- [x] 4.3 Add `search_sidebar: Option<SearchSidebar>` to `src/app/app_struct.rs`
      (replacing the fields removed in 1.7), initialised to `None` in
      `src/app/construct.rs` and `src/app/tests.rs`. Add
      `open_search_sidebar` / `dismiss_search_sidebar` to
      `src/app/library_search_actions.rs`. Do **not** add a `PanelFocus` variant and do
      not save prior focus — see `design.md` Decision 4.
- [x] 4.4 In `src/app/input_resolver.rs`, add
      `ContextEntry { name: "search_sidebar", handler: App::handle_key_search_sidebar }`
      in the slot vacated in task 1.4 (between `queue_column_width` and
      `panel_mode_cycle_x`).
- [x] 4.5 In `src/app/input.rs`, extend `handle_key_global_overlay_open` to open the
      sidebar on `Ctrl+/`, matching **both** `KeyCode::Char('/')` and
      `KeyCode::Char('_')` with `KeyModifiers::CONTROL` (see `design.md` Decision 9).
      Guard the re-press: return `Some(false)` without reopening when
      `self.search_sidebar.is_some()`.
- [x] 4.6 Rename `spawn_search_modal_query` → `spawn_search_sidebar_query`,
      `drain_search_modal_results` → `drain_search_sidebar_results`, and update
      `maybe_flush_search_debounce` (`src/app/run_loop_drains.rs:84`) and
      `drain_search_results` to call them. The debounce fields, the run-loop call site
      at `src/app/mod.rs:345`, and the `search_tx`/`search_rx` channel are unchanged.
- [x] 4.7 Verify: `rtk cargo check`. Sidebar has no renderer yet, so it is invisible;
      confirm `Ctrl+/` at least stops reaching the view beneath (add a temporary
      `flash_status` if useful, then remove it).

## 5. Render the sidebar

- [x] 5.1 Create `src/app/render/search_sidebar.rs` with `render_search_sidebar`,
      taking `power_area: Option<Rect>` and following the
      `src/app/render/overlays/playlists.rs:34-38` shape: `render_panel_shell_at` when
      the slot exists, `render_panel_shell` against `f.area()` at a fixed
      `SEARCH_PANEL_W` when it does not. Add `const SEARCH_PANEL_W: u16 = 40;` beside
      `PLAYLISTS_PANEL_W` in `src/app/mod.rs`. Title `"SEARCH"`, hints
      `"[↑↓]select [⇥]type [↵]open [Esc]close"`.
- [x] 5.2 Draw the query input on the first content row, with a cursor block and a
      loading indicator while `loading`.
- [x] 5.3 Draw the type-filter chip row below it: `All` plus one chip per
      `available_types()`, with the chip at `type_filter` highlighted.
- [x] 5.4 Draw the result list in the remaining rows: one row per
      `filtered_results()` entry, each carrying a type badge and a name truncated
      with `trunc_str`, the row at `cursor` highlighted, scrolled by `scroll`. No hero
      block, no second row, no image, no image fetch. Port the badge and truncation
      helpers from the deleted `render/overlays/search_modal.rs` (read it from
      `git show HEAD:src/app/render/overlays/search_modal.rs` if already deleted).
- [x] 5.5 Draw the empty and error states: `last_drain_error` when set, otherwise a
      no-results line when the query is non-empty and results are empty, otherwise a
      prompt line.
- [x] 5.6 Wire it in `src/app/render/mod.rs` next to the other panel calls (~line
      202-207): `if self.search_sidebar.is_some() { self.render_search_sidebar(f,
      power_panel_area); }`. Add `mod search_sidebar;` to `src/app/render/mod.rs`.
      Do **not** add it to `any_dim_modal_open`.
- [x] 5.7 If `src/app/render/search_sidebar.rs` exceeds 800 lines, split the chip row
      and result-row builders into `src/app/render/search_sidebar_rows.rs`. Do not
      compress to fit. Run `make check-code-file-lines`.
- [x] 5.8 Verify: `rtk cargo build`, then manually — `Ctrl+/` from home and from a
      library tab opens the panel in the queue column; typing two characters fires one
      query after a pause; Up/Down move; Tab cycles chips; Enter switches tab and
      selects the item and closes; Esc closes without navigating; the library list
      beneath ignores keys while it is open.

## 6. Tests and cleanup

- [x] 6.1 Update `src/app/input_resolver_handle_key_tests.rs` for the removed
      `search_modal` context and the added `lib_search` / `search_sidebar` contexts.
- [x] 6.2 Restore the `LibSearch`-dependent assertions that `03a725b` stripped from
      `src/app/actions_tests.rs`, `actions_tests_letter.rs`, `tests_library_position*.rs`,
      `tests_music_grouping.rs` and `render/tests_album_listing.rs`
      (`git show 03a725b -- <path>` shows what was cut). Skip any that simulate a full
      app or network flow.
- [x] 6.3 Add a unit test that `show_grouped` is false while `search.is_some()` on a
      music library at the album-folder level — the regression gate for task 3.4.
- [x] 6.4 Add a unit test for `dispatch_search_sidebar_query`: no debounce is armed
      below two characters, and one is armed at two.
- [x] 6.5 Run `rtk cargo test -p mbv-core`, `rtk cargo test`, `rtk cargo clippy
      --workspace --all-targets`, and `rtk make check-code-file-lines`. Fix warnings
      this change introduced; leave pre-existing ones alone.
- [x] 6.6 Update any docs that name the modal (`rtk grep -rn "search modal" -i docs/
      AGENTS.md CONTEXT.md`), including `CONTEXT.md` vocabulary. Do not hand-edit
      `openspec/specs/` — `openspec archive` applies the deltas. After archiving,
      check that `openspec/specs/search-modal/` is gone (its every requirement is
      REMOVED) and that `dimmed-backdrop-images`, `inline-library-search` and
      `global-search-sidebar` each landed with a real Purpose, not a `TBD` placeholder.
