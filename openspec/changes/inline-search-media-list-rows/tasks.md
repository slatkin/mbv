# Tasks: inline-search-media-list-rows

Reference: `design.md` decisions D1–D7; spec deltas in
`specs/inline-library-search/` and `specs/canonical-media-lists/`.
Run `cargo nextest run -p mbv` after each task; keep it green.

## 1. Control: embed the canonical carrier (D1, D2, D4-keyboard, D5)

- [x] 1.1 Add `results: MediaListCarrier<String>` to `InlineSearch`; delete
  `cursor`, `scroll`, `layout`, `mouse_gestures`, `left_press`,
  `move_cursor`, `move_cursor_by`, `page_size`, `select_row_at`,
  `select_row_at_point`, `handle_mouse`, `InlineSearchMouse`.
  (Accepted: `9394dafa`, from `1132488b`; `nextest -p mbv` 1532/1532.)
- [x] 1.2 Build `MediaListRow::Item` rows from the scored order in `set_pool` and
  the debounce re-score (labels/folder suffixes/year per D2; album
  display-label substitution via the pool). Re-score (query changed) ⇒
  `set_content` + `select_first()`; pool refresh (query unchanged) ⇒
  `set_content` alone (carrier preserves the stable target; delete the
  manual selected-item preservation). Empty query/order ⇒ zero rows.
  (Accepted: `9394dafa`; reset also parks the resting viewport at the top
  and both directions are asserted.)
- [x] 1.3 Route `handle_key` movement (Up/Down/PageUp/PageDown/Home/End) through
  `results.delegate_operation(MediaListSurfaceInput::…)` conversions;
  keep query editing, Enter/Esc/Backspace, debounce, `QueryStarted`.
  (Accepted: `9394dafa`; PageUp/PageDown take the carrier's canonical ×5
  step, the painted-layout height having died with the `layout` field.)
- [x] 1.4 `selected_item()` = `results.selected_target()` → pool lookup.
  (Accepted: `9394dafa`; `item_for_target(&str)` added for the pointer
  paths' `item_type` lookup.)
- [x] 1.5 `restore_query` re-scores + selects first; `restore_target` ⇒
  `results.select_target(id)` + `set_scroll(row_offset)`.
  (Accepted: `9394dafa`; `restore_target` narrowed to `Option<String>`,
  its old callers gone.)
- [x] 1.6 Update/extend control unit tests: re-score reset-to-first, pool-refresh
  target preservation, empty-query zero rows, movement through the
  carrier.
  (Accepted: `9394dafa`, from `1132488b`; four new control tests.)

## 2. Panel: paint the session through the PanelList surface (D3)

- [x] 2.1 Implement the panel's `PanelList` trait for `InlineSearch` by forwarding
  to the embedded carrier (object-safe, 7 one-line methods).
  (Accepted: `1132488b` — `library_panel/panel_list.rs`; no second policy
  site and no second painter.)
- [x] 2.2 Rewrite the `ListSlot::Search` arm of `paint_browser_pane`
  (`src/app/components/library_panel/wide.rs`, shared by Wide and Narrow):
  `render_search_box` bar; zero rows ⇒ `render_placeholder`
  (`" Loading…"` / `" (empty)"`); otherwise the `ListSlot::Media` driving
  sequence (`sync_viewport`, `set_paint_policy(Wide { focused })`,
  `set_geometry`, `view`, `selected_row_rect` for anchor geometry).
  (Accepted: `1132488b`; the placeholder literals still duplicate
  `ListSlot::Empty` and the zero-row path skips `set_geometry` — recorded
  note, no resolvable points exist with zero rows.)
- [x] 2.3 Update `library_panel/wide_tests.rs::active_search_takes_the_selector_row_and_the_list_box`
  for the canonical row painting; add rendered evidence: focused search
  row paints `palette::SELECTED_ROW_BG` across the row, unfocused paints
  no bar, zebra stripes, placeholder strings, zero-row rect handling.
  (Accepted: `1132488b`; three new rendered-evidence tests. Unfocused
  "no bar" is asserted on the selected row's own line because row 0 never
  stripes and `SELECTED_ROW_BG` shares the resting fill's RGB.)

## 3. Owners: pointer intents through the carrier (D4-pointer)

- [x] 3.1 Collapse `handle_search_pointer` in `emby_library_content.rs`,
  `tv_content/interaction.rs`, and `music_content.rs`/
  `music_interaction.rs` to `delegate_operation` over the search's carrier
  plus intent translation: right-click ⇒ `RowIntent::Context(target)` →
  the same `RowContextMenu` message each owner emits today
  (`Browser(vec![id])` vs `Emby(vec![item])`); double-click ⇒
  select + `InlineSearchActivate`; wheel ⇒ `MouseClaimed`; plain click ⇒
  select only.
  (Accepted: `9394dafa`, from `1132488b`; the delegated transition's
  `external_intent` is the authority for the resolved row, and modifier
  clicks delegate a plain select — the search session must never populate
  the carrier's `multi_selection` (D4 non-goal).)
- [x] 3.2 Delete `InlineSearchMouse` and the per-owner gesture translations.
  (Accepted: `1132488b`.)
- [x] 3.3 Update `emby_library_inline_search_tests.rs` (pointer test seeds the
  carrier instead of `layout_mut`) and
  `tv_content_component_tests_search.rs` (pool seeding via
  `restore_query`/typing stays; right-click test drives the intent path).
  (Accepted: `1132488b`.)

## 4. Context cleanup: TV grouping gate + Music ctx decoration (D6)

- [x] 4.1 `TvContent::set_content` grouped computation reads
  `self.inline_search.is_active()` instead of
  `context.list.is_search_active()`.
  (Accepted: `ad55488a`, from `e819e5ba`; the replaced ctx field was
  constant-false for TV, so this made the gate live for the first time —
  the new `tv_grouped_rows_flatten_while_search_is_open_and_restore_after_close`
  pins both directions: grouped rows while the session is closed, flat
  `Item`-only rows while it is open, grouped shape back after Esc. The
  post-close regroup rides the per-frame sync pass
  (`shell_run.rs:614` → `sync_tv_content` → `push_tv_workspace_content` →
  `set_content`), which recomputes rows before the next draw.)
- [x] 4.2 Delete the Music push's `context.list.with_search(…)` decoration
  (`shell_music_workspace.rs`).
  (Accepted: `e819e5ba`; behaviour-neutral — the deleted round trip pushed
  the owner's own session state back to it, and the owner opens/closes
  locally.)
- [x] 4.3 Delete `LibraryListRenderCtx::{search_query, search_loading, with_search,
  is_search_active}`.
  (Accepted: `e819e5ba`; no ctx search field or `library_search_active`
  remains in `src/`.)
- [x] 4.4 Confirm TV/Music/Emby tick-integration search tests pass unchanged
  (they drive real keys; only internal-seed lines may change).
  (Accepted: `e819e5ba`; all test-file hunks are seed-arity only —
  `from_items(.., 0, 0)` → `(.., 0)` — zero flow or assertion edits.)

## 5. Deletions + one-painter evidence (D7)

- [x] 5.1 Delete `src/app/render/components/list.rs`,
  `src/app/render/components/list_letter_groups.rs`,
  `src/app/render/components/media_list/plain_rows.rs`,
  `src/app/render/components/inline_search.rs` (the
  `render_inline_search` painter); trim re-exports in
  `src/app/render/mod.rs`; keep `render_search_box`
  (`render/components/hero.rs`) and
  `render/components/media_list/{row,wide}.rs` (live canonical painters).
  (Accepted: `e819e5ba`; `src/app/library_column_width.rs` was deleted too —
  both its consumers died in this unit and nothing took over its math.)
- [x] 5.2 Delete the now-unused `LibraryListRenderCtx` plumbing pieces surfaced by
  the deletions (`rows()`/`ListRenderCtx` consumers — audit; move what is
  genuinely shared rather than deleting what `list_letter_groups` still
  uniquely provided if other renderers need it).
  (Accepted: `fea7d173`; deleted `rows()`/`ListRenderCtx`,
  `FixedRowPlan`/`DisplayRow`, the span builders, `draw_column_selection_bleed`
  and the painter-only `scroll` field. Nothing `list_letter_groups` uniquely
  provided needed moving — `letter_bucket`/`effective_sort_str`/`LetterFilter`
  already live in `screens/sort_filter.rs`; `LibraryListRenderCtx` itself
  survives for TV/Music wide and detail.)
- [x] 5.3 Compile-level proof: no remaining callers of
  `render_generic_movies_home_video_rows_with_ctx`/`render_plain_rows`/
  `render_letter_grouped_rows`/`render_inline_search`.
  (Accepted: `fea7d173`; grep over `src/` returns no code matches. The one
  deleted test was the removed painter's own geometry test, whose claim the
  canonical-media-lists delta now forbids.)

Recorded note (accepted, not fixed): in the new TV gate test the flat-shape
assertion `rows.iter().all(Item)` would also hold for zero rows; the same test's
baseline push pins the fixture's 3 items, so the vacuity is not reachable within
this test.

## 6. Gates + integration evidence

- [ ] 6.1 `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D
  warnings`, `cargo nextest run -p mbv` and `-p mbv-core`, `openspec
  validate --all`.
- [ ] 6.2 Rerun the search-path tick-integration tests (TV wide/narrow Series
  activation, Music ctrl+A/album activation, Movies pool push) through
  real `Application::tick()`; adjust only seed calls, not the flows.
- [ ] 6.3 Buffer-coverage check per AGENTS.md TUI rules: Wide and Narrow each
  paint one bar + one result list; absent panels unmounted with empty
  areas; hit geometry follows the carrier's retained rects.
