## 1. Pin the defect and enumerate the real re-anchor sites

- [x] 1.1 Enumerate every production site that writes `level.cursor` for a music
      group view (start from `src/app/music_grouping.rs:296-309` and
      `src/app/music_actions.rs:51-109,196,254`) and record, in `design.md` D1,
      which of them are genuine shell-owned re-anchors versus write-throughs of
      a value the component already resolved. Verify: the list is derived from
      `rtk grep -n "\.cursor" src/app/music_grouping.rs src/app/music_actions.rs`,
      not from this proposal's summary.
      *Done: table added to design.md D1. `switch_music_group` is dead code; the
      catalog-commit anchor is not a component re-anchor (the component's
      `album_cursor` is an `album_index`, stable across grouping settle).*
- [x] 1.2 Write a test proving the "real change lost" case: push a cursor,
      move the component's cursor locally, then push a *different* shell-owned
      cursor, and assert the component currently ignores it
      (`music_workspace.rs:95-100`). Verify: the test passes against current
      behavior and is inverted in task 3.
      *Done as part of 3.2 (fix landed directly): the "ignores a plain push"
      assertion is `music_workspace_ordinary_push_leaves_album_cursor_alone`
      (component) and `music_workspace_ordinary_push_does_not_touch_album_cursor`
      (Model); the paired "re-anchor lands anyway" assertion is
      `music_workspace_re_anchor_overrides_prior_local_move` /
      `music_workspace_reanchor_lands_regardless_of_prior_local_move`.*
- [x] 1.3 Write a characterization test for saved-position restore on entering a
      music library, asserting the component's painted album cursor matches the
      restored position. Verify: `rtk cargo nextest run -p mbv` passes — this is
      the regression guard for D1's third re-anchor site.
      *Done: `music_workspace_first_mount_adopts_restored_album_cursor`
      (`shell_music_workspace_tests.rs`).*

## 2. Give the component sole ownership (D1)

- [x] 2.1 Add an explicit re-anchor entry point on `MusicWorkspaceComponent`
      that sets `album_cursor`/`album_scroll` from a value the shell supplies,
      separate from `set_content`. Verify: `rtk cargo check -p mbv`.
      *Done: `MusicWorkspaceComponent::re_anchor(cursor, scroll)`.*
- [x] 2.2 Call it from each site task 1.1 classified as a genuine re-anchor
      (expected: group switch, recursive-album activation, saved-position
      restore). Verify: task 1.3's test still passes.
      *Done via the one-shot `Model::music_workspace_reanchor` flag, consumed in
      `push_music_workspace_content`. Set at: first mount (`sync_music_workspace`,
      `!mounted` branch only), `RecursiveAlbumActivated` and
      `RestoreLibraryPosition` (`shell_run.rs`), and the `BrowserClick::SelectorTab`
      arm (`shell_messages.rs`, group-pill switch — mouse).*
- [x] 2.3 Remove the cursor/scroll/track adoption from
      `MusicWorkspaceComponent::set_content` (`music_workspace.rs:79-120`),
      including the `!self.initialized` branch — first-mount anchoring is now
      2.1's job, called once after mount. Verify: `rtk cargo check -p mbv`.
      *Done: `initialized` field removed; `set_content` reads no `list.cursor`,
      `list.scroll`, or `track_cursor`.*

## 3. Delete the echo detection (D2)

- [x] 3.1 Delete `last_mirrored_cursor` and `last_mirrored_scroll` and every
      read/write of them. Verify: `rtk grep -c "last_mirrored" src/` returns 0.
      *`rtk grep -rn "last_mirrored" src/` → no matches.*
- [x] 3.2 Invert task 1.2's test: the component now adopts a genuine shell
      re-anchor regardless of whether the user moved first, and ignores plain
      content pushes entirely. Verify: `rtk cargo nextest run -p mbv` passes.
- [x] 3.3 Confirm `last_album_id` and its track-focus reset are untouched (D4).
      Verify: `git diff` shows no change to the `album_changed` branch beyond
      the removal of `context.track_cursor` adoption.
      *`album_changed → track_cursor = None` and `last_album_id` tracking are
      both retained. The branch moved out of the deleted `!initialized`/`else`
      split (now unconditional); behaviour is identical (first push:
      `last_album_id` was `None`, so the reset ran there before too).*

## 4. Shed the unread context fields (D3)

- [x] 4.1 Check whether any other consumer reads `MusicWideRenderCtx::track_cursor`,
      `list.cursor`, or `list.scroll`. Remove only the fields with no remaining
      reader; leave the shared `LibraryListRenderCtx` shape alone if anything
      else uses it. Verify: `rtk cargo clippy --workspace --all-targets` is
      clean and reports no dead field.
      *No field removed. `MusicWideRenderCtx::track_cursor` is still read by the
      wide-music renderer (`music_wide.rs:233,260`) via `with_local_state`;
      `list.cursor`/`list.scroll` belong to the shared `LibraryListRenderCtx`.
      The `new()` `track_cursor` param is always `None` from
      `wide_music_render_ctx`, but removing it only ripples ~8 test-helper call
      sites for no runtime benefit — out of D3's conservative scope. Clippy
      reports no new dead field.*

## 5. Close out

- [x] 5.1 Confirm no `BrowseLevel` field changed (that is
      `split-browse-state-interaction-fields`'s scope). Verify:
      `git diff` touches no `BrowseLevel` field declaration.
- [x] 5.2 Update the Music workspace row in
      `docs/architecture/interactive-surface-ledger.md`.
- [x] 5.3 Verify: `rtk cargo check -p mbv`, `rtk cargo nextest run -p mbv`,
      `rtk cargo clippy --workspace --all-targets`, `rtk ast-grep scan`,
      `rtk cargo fmt`, and `rtk make check-code-file-lines` all pass.
      *check: clean. nextest: green except one pre-existing unrelated failure
      (`browser_component_tests::browser_local_navigation_mirrors_legacy_flat_movement`,
      confirmed failing on the pre-change baseline). clippy: no new warnings
      (repo carries pre-existing dead-code warnings from
      `remove-legacy-keyboard-endpoint`). ast-grep: 66 pre-existing
      screen-boundary diagnostics, none new, none in touched files. fmt: clean.
      check-code-file-lines: all files ≤ 800 (the `shell_music_workspace.rs`
      test module was split into `shell_music_workspace_tests.rs` via `#[path]`,
      matching the existing `shell_tv_workspace_tests.rs` pattern).*
