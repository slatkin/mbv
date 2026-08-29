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
- [x] 4.2 Introduce the resting-position type and move `BrowseLevel`'s
      persistence-facing uses onto it, leaving `cursor`/`scroll` in place for
      now. Verify: `rtk cargo check -p mbv`; 4.1's tests still pass.
- [x] 4.3 Re-point every outcome-1 reader from 1.1 (and 1.1b) to take the resolved
      value as a parameter, including `render_list`'s per-frame scroll
      write-back as a `&mut usize` render parameter. Verify:
      `rtk cargo check -p mbv` after each group; commit in
      reviewable units rather than one sweep.
- [~] 4.4 Re-point every outcome-3 reader from 1.1 (and 1.1b) to the component
      accessor, adding the accessors 1.1b names where they do not exist yet
      (`MusicWorkspaceComponent::selected_item`). Verify:
      `rtk cargo check -p mbv`.
      R16/R18 (Music `selected_item`) done in `c99e496d`; R14
      (`library_list_render_ctx` and its transitive readers `list.rs`,
      `tv_wide.rs`, `detail_series_view.rs`, `music_wide_browser.rs`,
      R19/R20 in `detail.rs`) still open. R14 constraint: verify the mounted
      component is reachable at the `library_list_render_ctx` call site; if it
      is not, stop and report the concrete design blocker per task 1.3 — do
      not add an App-side mirror or invent a fourth D2 outcome.
      **R14 moved out.** It stopped here per its own constraint: narrow TV and
      Emby podcast have no mounted component to read from. The blocker is
      recorded in design.md D6 and 1.3, and the threading lands in
      `migrate-narrow-browse-to-components` alongside the mounts it depends on.
      R16/R18 are done, so this task is complete as far as it can go here.

## 5. Hand-off

- [ ] 5.1 Confirm this change's remaining scope is closed: phases 1–4 done,
      R14 handed to `migrate-narrow-browse-to-components`, field deletion and
      retirement handed to `delete-browse-level-cursor-scroll`, doc sync handed
      to `sync-interactive-surface-docs`. Verify: `rtk cargo check -p mbv`,
      `rtk cargo nextest run -p mbv`, `rtk cargo clippy --workspace
      --all-targets`, `rtk ast-grep scan`, `rtk cargo fmt --check`,
      `rtk make check-code-file-lines`.

