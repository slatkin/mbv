## 1. Pin current behaviour before moving anything (D17 discovery)

- [x] 1.1 Write a characterization test proving the podcast show cursor after a
      `PageUp`/`PageDown` key is the value `App::lib_page_size()` produces, not
      the component's `page_size()` — drive the component with a painted
      geometry whose row count differs from `lib_page_size()` and record which
      stride wins. Verify: `rtk cargo nextest run -p mbv` passes and the test
      documents the *current* stride. If the two already agree in every
      reachable layout, record that in the test name and note it in `design.md`
      D1 — the risk is then closed, not ignored.
- [x] 1.2 Same characterization for the book list
      (`AudiobookshelfBookMove::PreviousBookPage`/`NextBookPage`, which read
      `App::lib_page_size()` at `shell_audiobookshelf_book.rs:34-40` while
      `AudiobookshelfBookComponent` carries its own shell-provided `page_size`).
      Verify: `rtk cargo nextest run -p mbv` passes.
- [x] 1.3 Write a characterization test proving the stale-adoption hole is real:
      push a podcast snapshot whose `shows` no longer contain the component's
      `selected_id`, with the snapshot's `episode_selection` set to `Some(n)`,
      and assert the component currently ends up with that App-sourced value.
      Verify: test passes against pre-change behavior; it will be inverted in
      task 5.

## 2. Podcast: resolved index replaces the eight deltas (D1, D2)

- [x] 2.1 Replace `PodcastShowMove` in `src/app/components/msg/intents.rs` with a
      resolved-index payload on the show-move request in
      `src/app/components/msg/shell.rs`. Verify: `rtk cargo check -p mbv`
      surfaces every call site (compiler-forced).
- [x] 2.2 In `AudiobookshelfPodcastComponent::handle_key`
      (`src/app/components/audiobookshelf_podcast.rs:150-205`), keep the local
      `move_cursor`/`select` calls and the existing
      `episode_selection.is_none()` match guards, then emit the request with
      the index the component landed on. Verify: `rtk cargo check -p mbv`.
- [x] 2.3 In `src/app/shell_messages.rs:217-250`, replace the eight-arm `match`
      with a single call to `App::select_audiobookshelf_show(index)`, keeping
      the existing post-move position-save tail. Verify: `rtk cargo check -p mbv`.
- [x] 2.4 Delete `App::move_audiobookshelf_show_cursor`,
      `move_audiobookshelf_show_rows`, and `jump_audiobookshelf_show_cursor`
      (`src/app/audiobookshelf_browse_actions.rs:171-203`). Do **not** copy their
      `episode_selection.is_some()` guard into `select_audiobookshelf_show` —
      per D2 it now lives only in the component. Verify: `rtk cargo check -p mbv`
      and `rtk cargo clippy --workspace --all-targets` report no unused-code
      warnings; update `shell_audiobookshelf_podcast_tests.rs:50` which calls
      `move_audiobookshelf_show_rows` directly.

## 3. Book: three resolved-value requests replace the twelve deltas (D1, D3)

- [x] 3.1 Replace `AudiobookshelfBookMove`'s 12 variants with three resolved-value
      requests — book index, bucket position, and chapter selection
      (`Option<usize>`) — in `src/app/components/msg/`. Verify:
      `rtk cargo check -p mbv` surfaces every call site.
- [x] 3.2 In `AudiobookshelfBookComponent::handle_key`
      (`src/app/components/audiobookshelf_book.rs:150-230`), keep the existing
      local `state.select` / bucket arithmetic and the `chapters_visible`
      geometry gate, and emit the resolved value. Verify: `rtk cargo check -p mbv`.
- [x] 3.3 Add one `App` entry point taking the resolved chapter selection
      (`Option<usize>`) in `src/app/audiobookshelf_browse_actions.rs`, replacing
      `focus_audiobookshelf_book_chapters`, `focus_audiobookshelf_book_browser`,
      and `move_audiobookshelf_book_row` (D3). Preserve the
      `selected_id.is_some()` precondition; the `chapter_selection.is_none()`
      half is a transition guard the component now owns. Verify:
      `rtk cargo nextest run -p mbv` — the existing book chapter-focus tests
      must pass unchanged.
- [x] 3.4 Rewrite `Model::handle_audiobookshelf_book_request`
      (`src/app/shell_audiobookshelf_book.rs:13-46`) to route the three requests
      to `App::select_audiobookshelf_book`,
      `App::select_audiobookshelf_book_bucket`, and the 3.3 entry point.
      Verify: `rtk cargo check -p mbv`.
- [x] 3.5 Delete `App::move_audiobookshelf_book_cursor`,
      `jump_audiobookshelf_book_cursor`, `cycle_audiobookshelf_book_bucket`, and
      `move_audiobookshelf_book_row`. Verify: `rtk cargo clippy --workspace
      --all-targets` is clean; any remaining caller is a mouse or test caller —
      update tests, and if a live non-mouse caller exists, stop and report it
      rather than keeping the function alive silently.

## 4. One page-size source (D1)

- [x] 4.1 With both shells no longer paging, confirm `App::lib_page_size()` has no
      remaining caller on either Audiobookshelf path and that the component's
      own stride is the only one. Verify: `rtk grep -n "lib_page_size"
      src/app/shell_audiobookshelf_book.rs src/app/shell_messages.rs` returns
      nothing for these two surfaces; task 1.1/1.2's characterization tests are
      updated to assert the component stride and still pass.

## 5. Close the stale-adoption hole (D4)

- [x] 5.1 In `AudiobookshelfPodcastComponent::set_content`
      (`audiobookshelf_podcast.rs:55-89`), make the component's own values win
      unconditionally: when the saved `selected_id` is absent from the new
      snapshot, reset `episode_selection`, `scroll`, and `episode_filter` to
      the component's defaults instead of leaving the snapshot's values in
      place. Clamp `scroll` against the new `shows.len()`. Verify: task 1.3's
      test is inverted to assert the component-default outcome and passes.
- [x] 5.2 Same in `AudiobookshelfBookComponent::set_content`
      (`audiobookshelf_book.rs:54-80`) for `chapter_selection`,
      `browser_offset`, and `selected_bucket`. Keep the existing
      `selected_bucket.min(buckets.len() - 1)` clamp and extend it to the
      reset path. Verify: `rtk cargo nextest run -p mbv` passes.
- [x] 5.3 Add a shell-level test for each surface driving a real content push
      that drops the selected item, asserting no App-sourced interaction value
      survives into the component. Verify: `rtk cargo nextest run -p mbv`.

## 6. Close out

- [x] 6.1 Confirm no `AudiobookshelfBrowseState` or
      `AudiobookshelfBookBrowseState` field was added or removed by this change
      (D5). Verify: `git diff src/app/types_audiobookshelf_browse.rs` shows no
      field-list change.
- [x] 6.2 Update the two Audiobookshelf rows in
      `docs/architecture/interactive-surface-ledger.md` to record the round
      trip's removal and the remaining shared-struct dependency on
      `split-browse-state-interaction-fields`.
- [x] 6.3 Verify: `rtk cargo check -p mbv`, `rtk cargo nextest run -p mbv`,
      `rtk cargo clippy --workspace --all-targets`, `rtk ast-grep scan`,
      `rtk cargo fmt`, and `rtk make check-code-file-lines` all pass.
