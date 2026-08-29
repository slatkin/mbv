## 1. Pin the defect and enumerate the real re-anchor sites

- [ ] 1.1 Enumerate every production site that writes `level.cursor` for a music
      group view (start from `src/app/music_grouping.rs:296-309` and
      `src/app/music_actions.rs:51-109,196,254`) and record, in `design.md` D1,
      which of them are genuine shell-owned re-anchors versus write-throughs of
      a value the component already resolved. Verify: the list is derived from
      `rtk grep -n "\.cursor" src/app/music_grouping.rs src/app/music_actions.rs`,
      not from this proposal's summary.
- [ ] 1.2 Write a test proving the "real change lost" case: push a cursor,
      move the component's cursor locally, then push a *different* shell-owned
      cursor, and assert the component currently ignores it
      (`music_workspace.rs:95-100`). Verify: the test passes against current
      behavior and is inverted in task 3.
- [ ] 1.3 Write a characterization test for saved-position restore on entering a
      music library, asserting the component's painted album cursor matches the
      restored position. Verify: `rtk cargo nextest run -p mbv` passes — this is
      the regression guard for D1's third re-anchor site.

## 2. Give the component sole ownership (D1)

- [ ] 2.1 Add an explicit re-anchor entry point on `MusicWorkspaceComponent`
      that sets `album_cursor`/`album_scroll` from a value the shell supplies,
      separate from `set_content`. Verify: `rtk cargo check -p mbv`.
- [ ] 2.2 Call it from each site task 1.1 classified as a genuine re-anchor
      (expected: group switch, recursive-album activation, saved-position
      restore). Verify: task 1.3's test still passes.
- [ ] 2.3 Remove the cursor/scroll/track adoption from
      `MusicWorkspaceComponent::set_content` (`music_workspace.rs:79-120`),
      including the `!self.initialized` branch — first-mount anchoring is now
      2.1's job, called once after mount. Verify: `rtk cargo check -p mbv`.

## 3. Delete the echo detection (D2)

- [ ] 3.1 Delete `last_mirrored_cursor` and `last_mirrored_scroll` and every
      read/write of them. Verify: `rtk grep -c "last_mirrored" src/` returns 0.
- [ ] 3.2 Invert task 1.2's test: the component now adopts a genuine shell
      re-anchor regardless of whether the user moved first, and ignores plain
      content pushes entirely. Verify: `rtk cargo nextest run -p mbv` passes.
- [ ] 3.3 Confirm `last_album_id` and its track-focus reset are untouched (D4).
      Verify: `git diff` shows no change to the `album_changed` branch beyond
      the removal of `context.track_cursor` adoption.

## 4. Shed the unread context fields (D3)

- [ ] 4.1 Check whether any other consumer reads `MusicWideRenderCtx::track_cursor`,
      `list.cursor`, or `list.scroll`. Remove only the fields with no remaining
      reader; leave the shared `LibraryListRenderCtx` shape alone if anything
      else uses it. Verify: `rtk cargo clippy --workspace --all-targets` is
      clean and reports no dead field.

## 5. Close out

- [ ] 5.1 Confirm no `BrowseLevel` field changed (that is
      `split-browse-state-interaction-fields`'s scope). Verify:
      `git diff` touches no `BrowseLevel` field declaration.
- [ ] 5.2 Update the Music workspace row in
      `docs/architecture/interactive-surface-ledger.md`.
- [ ] 5.3 Verify: `rtk cargo check -p mbv`, `rtk cargo nextest run -p mbv`,
      `rtk cargo clippy --workspace --all-targets`, `rtk ast-grep scan`,
      `rtk cargo fmt`, and `rtk make check-code-file-lines` all pass.
