# Plan: Wheel rebuilds the full row flow twice per notch (issue #926)

## Context

PR #925 removed the 30 ms wheel throttle, so every wheel event reaches the
Emby library list. Each notch builds a full `RowFlow` twice:

1. `MediaList::delegate_operation`, `MediaListOperation::Scroll` arm
   (`crates/mbv-components/src/media_list.rs`, Scroll arm) calls
   `self.row_flow()` before `scroll_viewport`.
2. `EmbyLibraryContent::wheel_reach_message` → `last_painted_reach_index`
   (`crates/mbv-components/src/emby_library_content/panel.rs`) calls
   `WideMediaList::resolve_viewport` → `MediaList::resolve_viewport`
   (`crates/mbv-components/src/media_list/wide.rs`,
   `crates/mbv-components/src/media_list.rs`), which calls `self.row_flow()`
   a second time.

`row_flow()` allocates one `Row` per row and clones every selectable target
`String`: O(rows) per notch, twice. Inferred from the code path, not
profiled. The issue asks to profile first, then pick between:

- (a) max offset from `rows.len()` / selectable count, no flow build.
- (b) cache the `RowFlow` in `MediaList`, rebuilt in `set_content`/`patch_row`.
- (c) reach path reuses the offset the scroll just produced.

## Findings so far

- Scroll (`Viewported::scroll_viewport` in
  `crates/mbv-components/src/list/viewport.rs`) needs only `flow.len()` for
  `max_viewport_offset = len - viewport_len`. It never reads targets. A
  length-only path removes build #1 with no new seam.
- After a scroll the anchor is `Free`, so `resolved_viewport_offset` takes
  the clamp-only branch (`offset.min(max)`) plus no selection pull. The
  leading-structural raise (`raise_over_leading_structural_rows`, opted in by
  `MediaList`) runs only under `FollowSelection`. So the reach-side resolve
  also needs only `len` in the post-scroll case — build #2 is removable the
  same way, or by reusing the scroll's offset (c).
- `last_painted_reach_index` additionally scans `rows[..=last]` backward for
  a selectable target: O(viewport height), fine as-is.
- Keyboard ops (`move_selection`, `select_first/last/index`,
  `context_intent`, `set_content`) genuinely need targets via `Cursored`;
  they stay on `row_flow()`. Wheel is the hot path, not cursor moves.
- Other wheel callers (`music_content/interaction.rs` track list and inline
  search) delegate `Scroll` with no reach report: single build only. The
  double build is specific to `EmbyLibraryContent::on_list_point_event`.
- `MediaList::rows()` and `WideMediaList::rows()` already expose the row
  slice; `resolve_viewport` is private on `MediaList`, public on
  `WideMediaList`. A length-based helper fits beside them.

## Approach (agreed 2026-10-11: implement directly, no profiling gate)

Implement (a)+(c) together and skip (b). Fix the shared `Scroll` arms so
EVERY wheel caller benefits, not just the Emby path:

- `MediaList::delegate_operation` `Scroll` arm: length-only scroll
  helper (max offset from `self.rows.len()`). Single change point covers
  Emby, book, TV series rail, playlists, search sidebar, music track list,
  and inline-search wheels — all funnel through this arm.
- `TreeBrowser::apply_operation` `Scroll` arm
  (`list/tree_browser/operations.rs`): same treatment via the existing
  `visible_len()` (no `current_flow()` target clones on the tree wheel
  path either).
- In `last_painted_reach_index`, compute the painted offset from the resting
  scroll the delegation just stored (`carrier.scroll()`), `rows.len()`, and
  the painted height — no second `resolve_viewport` call (this is (c)).
- Do NOT cache `RowFlow` (reject (b)): it adds an invalidation seam
  (`set_content`, `patch_row`, plus every future mutator) for a cost a
  length check already avoids.

## Files to modify

- `crates/mbv-components/src/media_list.rs` — length-only scroll + resolve
  helper; `Scroll` arm uses it.
- `crates/mbv-components/src/list/tree_browser/operations.rs` — `Scroll`
  arm uses `visible_len()` instead of `current_flow()`.
- `crates/mbv-components/src/emby_library_content/panel.rs` —
  `last_painted_reach_index` reuses the post-scroll offset instead of
  resolving again.
- Possibly `crates/mbv-components/src/media_list/wide.rs` — forward the
  helper if the reach path needs it through `WideMediaList`.
- Possibly `crates/mbv-components/src/list/viewport.rs` — a
  `max_viewport_offset_for_len(len, viewport_len)` free function so the seam
  stays in one place.

## Reuse

- `Viewported::max_viewport_offset`, `scroll_viewport`,
  `resolved_viewport_offset` (`crates/mbv-components/src/list/viewport.rs`).
- `MediaList::rows`, `WideMediaList::rows`, `carrier.scroll()`,
  `current_content_rect` (existing accessors).
- Existing wheel tests: `media_list/tests.rs` (viewport/scroll cases),
  `music_content/tests.rs` (wheel cases) — extend, do not inflate.

## Steps

- [x] Add length-only scroll path; `Scroll` arm uses it (removes build #1
      for all `MediaList` wheel callers).
- [x] Rework `last_painted_reach_index` to reuse the post-scroll offset
      (removes build #2).
- [x] Extend the owning tests with the double-build regression claim
      (scroll + reach at large N performs one pass, no target clones).
- [x] Run `cargo nextest run -p mbv-components`, `cargo clippy`, `cargo fmt`.

## Verification

- Bench before/after numbers at the chosen row counts.
- `cargo nextest run -p mbv-components` green; clippy with `-D warnings`
  clean; `cargo fmt --all -- --check` clean.
- Manual: wheel through a large Emby library, confirm smooth scroll and
  correct paging reach (pagination still triggers at the bottom).
