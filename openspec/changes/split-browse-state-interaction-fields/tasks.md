## 1. Authoritative reader inventory (D3)

- [x] 1.1 Produce a type-aware inventory of every non-test reader and writer of
      `BrowseLevel::cursor` and `BrowseLevel::scroll` using `rtk ast-grep`
      (field access on the `BrowseLevel` type), not text search. Record it in
      `design.md` as a table: file:line, live-cursor or resting-position, and
      the D2 outcome assigned. Verify: the count is stated explicitly and
      compared against #618's scout figure of ~37; a large divergence is
      reported before proceeding.
- [x] 1.2 Same inventory for `AudiobookshelfBrowseState`'s `selected_id`,
      `episode_selection`, `scroll` and `AudiobookshelfBookBrowseState`'s
      `selected_id`, `chapter_selection`, `selected_bucket`. Verify: recorded
      in `design.md`.
- [x] 1.3 Flag every reader that fits none of D2's three outcomes and stop for
      a decision rather than inventing a fourth path. Verify: the flagged list
      is empty, or the change pauses here with it reported.

## 2. Audiobookshelf book struct (D4, first)

- [x] 2.1 Write characterization tests for book position save and restore across
      a tab switch away and back, and for bucket selection surviving a content
      refresh. Verify: `rtk cargo nextest run -p mbv` passes pre-change.
- [x] 2.2 Split `AudiobookshelfBookBrowseState` into a content struct and a
      component-owned interaction struct. The component holds its own
      interaction state and receives only content. Verify:
      `rtk cargo check -p mbv` surfaces every call site (compiler-forced).
- [x] 2.3 Re-point each reader from 1.2 per its assigned outcome. Verify:
      `rtk cargo check -p mbv`.
- [x] 2.4 Delete `AudiobookshelfBookComponent::set_content`'s clobber-then-restore
      block (`audiobookshelf_book.rs:60-77`) — with content and interaction
      separated it has nothing to restore. Verify: 2.1's tests still pass.
- [x] 2.5 Verify: `rtk cargo nextest run -p mbv`, `rtk cargo clippy --workspace
      --all-targets`, `rtk ast-grep scan` all pass.

## 3. Audiobookshelf podcast struct (D4, second)

- [x] 3.1 Characterization tests for show position save/restore, episode-mode
      entry and exit, and episode filter surviving a refresh. Verify: passes
      pre-change.
- [x] 3.2 Split `AudiobookshelfBrowseState` the same way, applying whatever the
      book split taught. Preserve `select()`'s existing side effects — the
      `episode_selection = None` reset and the filter reset on identity change
      (`types_audiobookshelf_browse.rs:108-116`) — on the interaction struct.
      Verify: `rtk cargo check -p mbv`.
- [x] 3.3 Re-point each reader from 1.2. Verify: `rtk cargo check -p mbv`.
- [x] 3.4 Delete the podcast `set_content` restore block
      (`audiobookshelf_podcast.rs:61-85`). Verify: 3.1's tests still pass.
- [x] 3.5 Verify: full gate as in 2.5.

## 4. Split live cursor from resting position in `BrowseLevel` (D1)

- [x] 4.1 Characterization tests for: position restore on entering a library,
      `go_back`'s parent-cursor re-anchor (`actions_navigation.rs:239-278`),
      and prefetch triggering at the cursor threshold
      (`library_search_actions.rs:240`). Verify: passes pre-change.
- [ ] 4.2 Introduce the resting-position type and move `BrowseLevel`'s
      persistence-facing uses onto it, leaving `cursor`/`scroll` in place for
      now. Verify: `rtk cargo check -p mbv`; 4.1's tests still pass.
- [ ] 4.3 Re-point every outcome-1 reader from 1.1 to take the resolved value as
      a parameter. Verify: `rtk cargo check -p mbv` after each group; commit in
      reviewable units rather than one sweep.
- [ ] 4.4 Re-point every outcome-3 reader to the component accessor. Verify:
      `rtk cargo check -p mbv`.
- [ ] 4.5 Delete `BrowseLevel::cursor` and `BrowseLevel::scroll`. Verify:
      `rtk cargo check -p mbv` is clean with no transitional accessor left
      behind (D5).

## 5. Retire what the split makes unreachable

- [ ] 5.1 Delete `App::apply_lib_cursor_index` (`lib_cursor_actions.rs:241`) and
      route `ShellRequest::BrowserCursorIndex` to the resting-position writer
      and effect tail directly. Verify: `rtk cargo check -p mbv`.
- [ ] 5.2 Delete `App::move_lib_cursor_rows` and `App::jump_lib_cursor` — both
      already have no live caller — and `App::move_lib_cursor`, whose only
      non-test caller is `mouse_gestures.rs:83`. Delete the mouse call sites
      with them; do not repair mouse behaviour (D16). Verify:
      `rtk cargo clippy --workspace --all-targets` reports no dead code.
- [ ] 5.3 Re-check `mouse_gestures.rs` for remaining writes to the deleted
      fields (`:122`, `:219`, `:231`) and delete those paths. Verify:
      `rtk cargo check -p mbv`.

## 6. Retire the conventions the types now enforce

- [ ] 6.1 Review `rules/interactive-component-boundary/` and remove only the
      clauses the types now make unrepresentable; keep every clause still
      guarding a real boundary. Verify: `rtk ast-grep test` fixtures pass and
      `rtk ast-grep scan` is clean.
- [ ] 6.2 Delete the warning comments that documented the old rule (for example
      `input_browse_dispatch.rs:22`, `context_menu_actions.rs:305`), since the
      thing they warned about no longer compiles. Verify: `rtk grep -n
      "mirror" src/app/` returns only historical references in archived docs.

## 7. Close out

- [ ] 7.1 Update `docs/architecture/interactive-surface-ledger.md`, ADR 0022,
      and `openspec/specs/interactive-component-framework/spec.md` so all three
      describe the same completion state (#614's criterion).
- [ ] 7.2 Split any file pushed over 800 lines by this change. Verify:
      `rtk make check-code-file-lines` passes.
- [ ] 7.3 Verify the full gate: `rtk cargo check -p mbv`,
      `rtk cargo nextest run -p mbv`, `rtk cargo clippy --workspace
      --all-targets`, `rtk ast-grep scan`, `rtk cargo fmt`,
      `rtk make check-code-file-lines`.
- [ ] 7.4 Confirm #607's acceptance criterion "component-local interaction state
      has one owner" now holds literally: no `App` field stores a live cursor,
      scroll, or selection for a mounted component. Verify: stated against the
      task 1 inventory, every row resolved.
