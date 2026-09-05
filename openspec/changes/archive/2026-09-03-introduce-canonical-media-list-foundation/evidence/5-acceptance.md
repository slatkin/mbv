# Task 5.3 — consolidated slice acceptance evidence

This record consolidates the stateful, rendered, source-level one-painter, live,
file-size, and verification evidence for the corrected canonical-media-list
foundation slice, and states its reviewability/reversibility boundary. It builds
on the 5.1 focused review (PASS, commit `a9a78f7c`) and the 5.2 live acceptance
(PASS, user-performed).

## Stateful

Task 4.1 stateful coverage is carried by these tests:

- `src/app/components/browser_component_tests.rs`
  - `set_content_keeps_the_control_cursor_and_apply_position_moves_it` —
    proves 4.1(a) `BrowserContent` cannot express cursor/scroll (position is
    stripped by `from_render_ctx` before the control boundary), 4.1(b) repeated
    pushes at an unchanged browse identity (pagination, loading completion,
    refresh) preserve the control's own cursor/scroll after movement, and
    4.1(c) a changed browse identity (depth, parent, back-restore, letter
    reset, sort, saved restore, feed-group switch) re-seeds position only
    through the explicit `apply_position` push.
  - `browser_local_navigation_mirrors_legacy_flat_movement`,
    `browser_local_navigation_skips_letter_headers_and_ragged_rows`,
    `browser_local_navigation_strides_one_column_for_wide_movies` — movement
    delegated to the active persistent control matches legacy flat movement,
    header/ragged-row skipping, and one-column Wide striding.
  - `browser_control_transition_preserves_the_selected_viewport_offset` —
    proves 4.1(d): selected target plus selected-row offset survive the
    same-destination hero Browser Wide->Narrow->Wide transition and the TV
    Wide `TvWorkspaceComponent` <-> Narrow `BrowserComponent` round trip.

New regression test:

- `src/app/render/tests_library_characterization.rs::wide_letter_grouped_row_map_indexes_items_without_counting_headings`
  — locks the 5.1 correction: the Wide letter-grouped `left_row_map` indexes
  selectable items and does not advance its index on `Heading`/`Spacer` rows.

## Rendered

Task 4.2 rendered characterization is the suite in
`src/app/render/tests_library_characterization.rs`, rewritten against the
control-exported geometry of task 3.5b (`RowGeometry` / `InlinePaintResult`)
rather than retained `layout.left_item_rows` / `left_sorted_indices`
assertions:

- `library_buffer_characterization_covers_wide_unfocused_narrow_and_selected_states`
- `movies_plain_replacement_characterization_covers_bottom_scroll_fallback_and_targets`
- `tv_letter_grouped_replacement_characterization_covers_header_fit_and_marker_suppression`
- `wide_letter_grouped_row_map_indexes_items_without_counting_headings`

Companion rewrites live in `src/app/tests_narrow_browse_migration.rs`,
`src/app/render/test_helpers_fixtures.rs`, and
`src/app/render/tests_non_music.rs`, covering metadata, grouping, active
progress, focus, breakpoint, and image fixtures.

## Source-level one-painter

See `evidence/4.3-one-painter.md` for the full reachable call-chain record
covering every migrated destination (hero Movies, Emby homevideos/feed-group,
Emby podcast-channel browsing, narrow TV Series, Wide TV right rail): no
applicable Wide path reaches
`render_generic_movies_home_video_rows_with_ctx` (and therefore neither
`render_letter_grouped_rows` nor `render_plain_rows`), no per-frame Narrow
canonical-control construction, no duplicate replacement geometry, and exactly
one `LibraryListRenderCtx` construction site inside
`src/app/components/browser/`, fed by control-owned position.

The 5.1 correction (`a9a78f7c`) does not add a painter or a position channel.
`layout.left_row_map` remains the sole retained pre-#638 mouse-compat geometry
write; it is now populated from the `InlineMediaBrowser` / `WideMediaList`
`RowGeometry` stable targets exported in 3.5b, and the correction only fixes
that map to index selectable items instead of counting heading/spacer rows. It
stays wired and untouched, owned by `restore-mouse-support` (#638).

## Live

Task 5.2 Wide/Narrow acceptance was performed by the user and reported PASS
across: selection, movement, focus, scrolling, images enabled and disabled,
Inline replacement, and the TV rail — all matching the legacy reference.

## File size

`rtk make check-code-file-lines` exits non-zero on one pre-existing flag:

```text
code-file-lines: src/app/shell_home.rs has 804 lines (maximum 800)
```

`src/app/shell_home.rs` was already 804 lines at the slice base `49429afc`
and is not touched by this slice; it is out of scope here. Every source file
changed by this slice is ≤800 lines:

- `src/app/render/components/media_list.rs` — 488
- `src/app/render/tests_library_characterization.rs` — 253

## Verification

Gates run fresh at HEAD `44d7b201`:

| Gate | Result |
| --- | --- |
| `rtk cargo fmt --check` | PASS (no output) |
| `rtk cargo check -p mbv` | PASS — 0 errors, 3 warnings (pre-existing dead-code: `render_home_video_item`, `has_group_pills`) |
| `rtk cargo nextest run -p mbv` | PASS — 1254 passed (1 binary) |
| `rtk cargo clippy --workspace --all-targets` | PASS — 0 errors, 12 warnings (all pre-existing: test imports/`clone`-slice in `feeds`/`shell_music_workspace` test files, `home_hero` identical-if, `layout.rs` `movies_wide_area`, `home_video` dead fn); none in files changed by this slice |
| `rtk ast-grep scan` | PASS — exit 0, no findings |
| `rtk make check-code-file-lines` | FAIL — pre-existing `src/app/shell_home.rs` 804 lines only; out of slice (see File size) |
| `rtk openspec validate introduce-canonical-media-list-foundation --strict` | PASS — "Change ... is valid" |

## Reviewability and reversibility

The corrected-foundation slice is the commit range `49429afc..HEAD`:

- `a9a78f7c` fix(canonical-lists): map Wide `left_row_map` to selectable index, not source rows
- `64f14414` chore(openspec): mark canonical foundation 5.1 review complete
- `44d7b201` chore(openspec): mark canonical foundation 5.2 live acceptance passed

plus this evidence commit. Changes touch only `src/app/render/**` and
`openspec/changes/introduce-canonical-media-list-foundation/**`. No protocol,
provider, daemon, or persistence code is modified, so the slice is
independently reviewable and revertible.
