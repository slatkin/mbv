# Tasks

## 1. Delete the duplicated tests

- [x] 1.1 Delete `crates/mbv-components/src/tv_content/tests/wheel_tests.rs`
      and the `mod wheel_tests;` line in
      `crates/mbv-components/src/tv_content/tests.rs`. Verify:
      `rg -n "wheel_tests" crates/mbv-components/src/tv_content/` is empty.
- [x] 1.2 Delete `book_wheel_scrolls_viewport_without_a_selection_report`
      from `crates/mbv-components/src/book_content/tests.rs`, including its
      in-fn `use` lines if they orphan nothing else. Verify:
      `rg -n "book_wheel" crates/mbv-components/src/book_content/` is empty.

## 2. Re-point the ledger rows

- [x] 2.1 Wide TV row (`docs/architecture/interactive-surface-ledger.md`,
      Wide TV wheel row): drop the two `tv_content::tests::wheel_tests::…`
      citations; keep
      `list::tree_browser::tests::structure::a_wheel_scroll_moves_the_viewport_and_keeps_the_selected_node`;
      add the delegation prose.
- [x] 2.2 Audiobookshelf books row: replace
      `book_content::tests::book_wheel_scrolls_viewport_without_a_selection_report`
      with the shared
      `media_list::tests::a_wheel_scroll_moves_the_viewport_and_leaves_the_selection_untouched`
      citation plus the Podcast-style sentence. Verify:
      `rg -n "wheel_tests|book_wheel" docs crates/mbv-components` finds no
      dangling test names.

## 3. Group gate

- [x] 3.1 `cargo nextest run -p mbv-components` passes with a net −3 tests;
      `cargo clippy --workspace --all-targets -- -D warnings` is clean;
      `cargo fmt --all -- --check` is clean. Fix any `dead_code` by deleting
      the item, never with `allow`/`expect`.
- [x] 3.2 `make check-code-file-lines` passes (deletions only shrink files).
- [x] 3.3 Commit.
