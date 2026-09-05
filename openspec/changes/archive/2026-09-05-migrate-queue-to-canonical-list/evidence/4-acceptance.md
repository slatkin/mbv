# Umbrella rows 4.2 / 4.5 — Queue acceptance evidence

Evidence-only record for `compose-canonical-media-lists` rows 4.2 (Queue never
enters Hero-on-left or selected-row replacement; fixed-row presentation proven
directly, not inferred from shared types) and 4.5 (implementation, tests, gates,
review, acceptance, recorded manual evidence). Added because the accepted Queue
slice recorded its one-painter and geometry evidence only in `tasks.md` prose,
which the umbrella row could not audit against current source.

HEAD at every rerun below: `f9fa56a2` (branch `feat/migrate-tui-to-tuirealm`).
Queue slice commit range: `a956e642..d426e057`; Queue-own commits are `a956e642`
(2.1-2.4 composition), `6c0931df` / `ae2653d9` / `0e4993b9` (shared canonical-row
fixes incl. the Queue watch-% badge), `40b51101` (3.2 fixtures), `d426e057`
(acceptance state). The range also contains unrelated TV/media-list fixes made
while the slice was in review.

Every command below is a **direct rerun at `f9fa56a2`** unless a section is
explicitly labelled ARCHIVED. `rtk` is unavailable in this session; commands are
shown as actually run.

## 1. Sole Queue body painter (4.2, direct)

`rg -n "render_wide_media_list|render_inline_media_browser" src/` returns exactly
one Queue-relevant call site — the mounted parent, never the render seam:

```text
src/app/components/queue.rs:18:    render_queue_title_content, render_wide_media_list, QueueRenderGeometry, QueueTitleModel,
src/app/components/queue.rs:434:        render_wide_media_list(
```

`src/app/render/components/queue.rs` contains **no** painter call: it now holds
only the Queue title painter (`render_queue_title_content`), which paints the
title/scope pills, not slot rows. All other painter call sites belong to other
destinations (`browser/paint.rs`, `home.rs`, `feeds.rs`, `list_narrow.rs`,
`audiobookshelf_book.rs`, `audiobookshelf_podcast.rs`, `music_wide*.rs`,
`tv_wide.rs`, plus the painter's own definition file).

## 2. Zero Queue `InlineMediaBrowser` / Hero references (4.2, direct)

`rg -ln "InlineMediaBrowser" src/` → 23 files. **No Queue file is among them:**
`audiobookshelf_book.rs`, `audiobookshelf_podcast.rs`,
`browser_component_tests.rs`, `browser/mod.rs`, `feeds.rs`, `home.rs`,
`media_list/{inline,mod,tests}.rs`, `music_workspace.rs`, the matching
`render/components/*`, `render/tests_{audiobookshelf_books,audiobookshelf_podcasts,hero_left_pane_characterization,music_narrow,music_wide_reanchor_characterization}.rs`,
`tests_tick_integration_mouse.rs`.

```text
$ rg -n -i "hero" src/app/components/queue.rs src/app/render/components/queue.rs src/app/components/queue_component_tests.rs
(exit 1 — no matches)
```

Selected-row replacement is likewise absent; the only hits for
`replace|selected_row` in the two Queue source files are the parent's own
context-menu anchor accessor and a comment:

```text
src/app/components/queue.rs:56:    pub(crate) fn selected_row_rect(&self) -> Option<Rect> {
src/app/components/queue.rs:431:        // only for `selected_row_rect` and tests.
```

`selected_row_rect` forwards the child's row rect for the Queue context-menu
anchor; it is not Inline's hero-substituting row.

## 3. Non-hero two-column browsers retain their policy (4.2, direct)

The carve-out is still live in source, so Queue's use of the Wide control did not
collapse the two-column path:

```text
src/app/render/components/list.rs:29:            super::list_letter_groups::render_letter_grouped_rows(
src/app/render/components/list.rs:37:            super::media_list::render_plain_rows(f, row_ctx, layout)
src/app/render/components/list_narrow.rs:176:            super::list_letter_groups::render_letter_grouped_rows(
src/app/render/components/list_narrow.rs:184:            super::media_list::render_plain_rows(f, row_ctx, layout)
src/app/render/components/music_wide.rs:606:            output.final_scroll = super::media_list::render_plain_rows(
```

`render_letter_grouped_rows` (`src/app/render/components/list_letter_groups.rs:20`)
and `render_plain_rows` (`src/app/render/components/media_list.rs:40`) remain the
two-column Browser painters. Queue calls neither; they do not call Queue.

## 4. One-painter and child-rect trace (4.2, direct source trace)

`QueueComponent::view()` (`src/app/components/queue.rs:398-457`) is linear and
executes at most one body painter per frame:

1. `self.area = area; self.geometry = QueueRenderGeometry::default()` — never
   reuses last frame's area.
2. Optional `render_queue_title_content(frame, title_area, …)` — title row only.
3. `if area.height < 1 { return }` — child rect guard.
4. `if self.list.is_empty()` → `Paragraph` empty text, `return`.
5. The single body painter (`queue.rs:434`):
   `render_wide_media_list(frame, area, area, &mut self.list, self.focused, palette::SURFACE_FOCUSED)`
   — `paint_area == content_area == area`, i.e. the child rect **is** the
   shell-published `queue_area`; non-emptiness is guarded at step 3 and asserted
   by the conformance test in §5.
6. Row geometry is *derived from the child*, not recomputed:
   `self.list.resolve_viewport(area.height as usize)` then one
   `Rect { x: area.x, y: area.y + line, width: area.width, height: 1 }` per
   visible selectable row (`queue.rs:443-457`). Fixed rows: height 1, full panel
   width, one per viewport line.

Inside the painter (`src/app/render/components/media_list.rs:263-351`) the
resolved offset is stored back via `list.set_scroll(offset)` at line 345, so the
child owns scroll and the parent keeps no mirror.

**One-painter counter limitation (stated, not invented).** The test-only
execution counters `WIDE_MEDIA_LIST_PAINTS` / `INLINE_MEDIA_BROWSER_PAINTS` /
`PLAIN_ROWS_PAINTS` (`media_list.rs:25`) are asserted only by
`render/tests_audiobookshelf_books.rs`, `tests_audiobookshelf_podcasts.rs`, and
`tests_music_wide.rs`. There is **no Queue-slice-own counter test**; the Queue
one-painter proof is source-level (§1, §4) plus the legacy-base-frame execution
test (§5). Adding a Queue counter assertion would require a Rust edit, which is
out of scope for this evidence-only correction.

## 5. Fixed-row geometry / scroll / progress tests (4.5, direct rerun)

```text
$ cargo nextest run -p mbv 'queue_component_' 'queue_projection_' 'queue_movement_' \
    'now_playing_queue_row_' 'queue_right_click_' 'queue_scope_mouse_pills_' \
    'queue_legacy_base_frame_' 'queue_refresh_retains_'
Summary [   0.072s] 17 tests run: 17 passed, 1394 skipped
```

All 17 PASS. The eight that carry the row/scroll/progress contract:

- `queue_legacy_base_frame_reserves_geometry_but_paints_no_slot_rows`
  (`render/tests_conformance_matrix.rs:705`) — for `[(60, 20), (140, 30)]`:
  asserts `app.layout.main.queue_area.width > 0` (child rect non-empty) and that
  the base frame paints no `"Item 0"`/`"Item 1"` slot rows. This is the
  one-painter **execution** proof and the non-empty child-rect proof. It does
  **not** cover 120x40 — unlike the Podcast/Book matrix rows, Queue's pair is
  60x20/140x30.
- `queue_projection_clamps_active_progress_to_presentation_bounds` — bounded
  `progress_percent`: `position_ticks` 0 → no percent, 250/100 → `Some(100)`.
- `now_playing_queue_row_shows_elapsed_next_to_duration` — buffer contains the
  rendered active-row metadata `"0:30 / 2:00"`.
- `queue_movement_uses_single_row_stride_and_follows_focus` — `PageDown` advances
  the cursor by exactly one row (0 → 1 → 2), i.e. fixed-row stride.
- `queue_refresh_retains_selected_target_and_scrolls_to_it` — `QueueCursorUpdate::Preserve`
  keeps cursor 20 and `test_selected_target() == slots[20].slot_id` across a
  refresh, with `test_scroll() > 0`.
- `queue_component_upward_scrolling_reaches_top` /
  `queue_component_page_up_from_bottom_reaches_top` — 30 slots in an 8-row
  viewport clamp back to cursor 0 / scroll 0.
- `queue_component_instances_isolate_viewport_state` — two Queue instances keep
  independent scroll (scrolled vs 0).
- `queue_right_click_uses_the_rendered_slot_target` — a mouse event at
  `component.test_rows()[1]` resolves `slot_id == slots[1].slot_id`, proving the
  parent's published hit rects are the child's painted rows (§4 step 6).

Also in the 17: `queue_component_renders_a_snapshot_without_app_state`,
`queue_component_emits_typed_keyboard_intents`,
`queue_set_content_follow_the_playhead_moves_cursor_when_slots_persist`,
`queue_activation_uses_slot_id_after_snapshot_reorder`,
`queue_scope_switch_resets_component_scroll`,
`queue_scope_mouse_pills_reset_component_scroll_from_nonzero`,
`shell_frame_publishes_queue_geometry_to_queue_component_and_layout`,
`shell_frame_uses_queue_component_geometry_for_keyboard_context_menu_anchor`.

## 6. Gates (4.5, direct rerun at `f9fa56a2`)

| Gate | Result |
| --- | --- |
| `cargo fmt --all -- --check` | PASS — no output, exit 0 |
| `openspec validate migrate-queue-to-canonical-list --strict` | PASS — `Change 'migrate-queue-to-canonical-list' is valid` |
| `cargo nextest run -p mbv queue` | PASS — `Summary [   5.558s] 248 tests run: 248 passed, 1163 skipped` |
| focused 17-test Queue set (§5) | PASS — 17 run, 0 failed |
| `cargo check --workspace --all-targets` | NOT RUN — the nextest builds compiled all `mbv` test targets in this session (`Finished test profile`, 5 warnings: the pre-existing `meta_rows` / `has_group_pills` dead-code family); the `mbv-core`/`mbvd` crates were not separately checked and are claimed as unverified |
| `make check-code-file-lines` | NOT RUN this session — Queue's changed files are `src/app/components/queue.rs` and `src/app/render/components/queue.rs`; the umbrella records the campaign-wide gate at task 5.2 |

## 7. Manual end-to-end evidence (4.5, ARCHIVED cross-slice record)

**These are archived records, not reruns.** The Queue slice's own task 4.1
acceptance is recorded as prose in `openspec/changes/migrate-queue-to-canonical-list/tasks.md`
with no width capture attached, so the only width-bearing live record covering
Queue is the cleanup slice:

`openspec/changes/archive/2026-09-04-remove-bespoke-media-list-loops/tasks.md:54`
(task 4.5, `[x]`):

> Result: user ran live acceptance at 60x20 / 120x40 / 140x30 across Home,
> Movies, TV, Music, Podcast, Book, Feeds, Queue — all five checks pass, no
> defects.

Its checks (same tasks.md, and `specs/media-list-cleanup/spec.md:66-72`) include
Queue fixed rows, no underpaint, stable geometry, the two-column carve-out, and
both feed surfaces. The cleanup slice lands *after* all four destination slices,
so its Queue observation exercised the canonical Queue composition, not a
pre-slice painter.

**Queue-slice-own live-width limitation.** No live capture names Queue at a
specific width from inside the Queue slice, and no human rerun at 60x20 / 120x40
/ 140x30 was performed for this evidence file (a live TUI session cannot be
driven from this environment). The 120x40 Queue observation rests solely on the
cleanup-slice record; Queue's automated fixed-geometry coverage is 60x20 and
140x30 plus a 40x8 component buffer (§5). Row 4.5's manual requirement is met by
the cross-slice record, and that substitution is disclosed here rather than
restated as Queue-slice evidence.

## 8. Review / acceptance state

Slice tasks 1.1-4.1 are `[x]` in `migrate-queue-to-canonical-list/tasks.md`;
acceptance state was committed at `d426e057` (`docs(openspec): accept queue
canonical list slice`), and the umbrella records 3.4 accepted (task 3.4 `[x]`,
"Queue's `QueueHitRegion` path is left wired and untouched here"). Mouse work is
`restore-mouse-support` (#638), archived at `0cbe7508` — not a Queue-slice
contribution. This file adds no task-state change; rows 4.2 and 4.5 stay
unchecked for the parent to close.
