# Task Brief: Album Plan — Suppress Inline Expansion (Tasks 3.1-3.4)

## Project Context

mbv is a Rust TUI client for Emby. The music library is being routed through
`render_power_list` to inherit the hero area, two-column layout, and shared
list infrastructure. Batch 1 added the album hero sizing helper and
`power_selected_album_item`; Batch 2 added the album branch to the hero content
painting in `list.rs` — when the hero panel is present, it renders the selected
album's detail (art + track list + metadata) into the fixed hero rect.

**This batch makes the grouped-album display plan stop producing the inline
expansion rows when the hero handles the detail**, so the list below the hero
does not render the detail a second time.

The plan builder is `App::build_grouped_album_display_plan` in
`src/app/render/album_plan.rs` (currently lines 117-478). It emits an
"expanded block" for the selected album: `AlbumDetailRule` framing rows,
`AlbumActionHint` / `ArtistActionHint` hint rows, an
`AlbumDetailStart`/`AlbumDetailContinuation` track block (or `AlbumLoading`
while tracks fetch), album-art reservation rows, and `selected_block_bounds` /
`track_detail_bounds` for the renderer. When the hero handles the detail, all
of that chrome must be suppressed from the plan, and the bounds set to `None`.

## Tasks

### Task 3.1: Add `hero_handles_detail: bool` parameter

In `src/app/render/album_plan.rs`, add a `hero_handles_detail: bool` parameter
to `build_grouped_album_display_plan`, positioned **after `expand_selected` and
before `wrap_widths`**:

```rust
pub(super) fn build_grouped_album_display_plan(
    &mut self,
    albums: &[mbv_core::api::MediaItem],
    album_info: &[(String, String, String)],
    order: &[usize],
    cursor: usize,
    fetch_missing_tracks: bool,
    selectable_headers: bool,
    selected_artist_header: Option<&ArtistHeaderSelection>,
    expand_selected: bool,
    hero_handles_detail: bool,
    wrap_widths: Option<(u16, u16)>,
) -> GroupedAlbumDisplayPlan
```

### Task 3.2: Suppress detail rows when `hero_handles_detail` is true

When `hero_handles_detail` is true, the plan must **not** contain any of:
`AlbumDetailStart`, `AlbumDetailContinuation`, `AlbumDetailRule`,
`AlbumLoading`, or `AlbumActionHint` rows. (`ArtistActionHint` is intentionally
**not** in the suppression list — it stays: artist headers remain selectable in
the list area below the hero, and their hint is a list feature.)

Suppression sites inside `build_grouped_album_display_plan`:

**Selected-group branch** (the `if selected_group { ... }` block, currently
lines 250-350):

- Gate the two leading `AlbumDetailRule` pushes (the top padding + top rule)
  and the `let top_idx = rows.len();` on `!hero_handles_detail`.
- Keep the `ArtistHeader` push unconditional.
- When `header_selected`: keep the `ArtistActionHint` push and its
  `AlbumWrappedContinuation` hint continuations unconditional.
- When **not** `header_selected`: gate the `AlbumActionHint` push AND its
  `AlbumWrappedContinuation` hint continuations on `!hero_handles_detail`
  (the continuations carry the wrapped hint text; leaving them would emit
  blank rows).
- Keep `ArtistGroupSpacer` and the per-album `Album(idx)` + title
  `AlbumWrappedContinuation` rows unconditional.
- Gate the track-detail block (`AlbumDetailContinuation` +
  `AlbumDetailStart(idx)` + continuations, or the `AlbumLoading` block with its
  continuations and art-reservation rows) on `!hero_handles_detail`. This means
  the plan builder does **not** call `fetch_album_tracks` in the hero case —
  the hero painting in `list.rs` already triggers the fetch.
- Gate the trailing art-reservation `AlbumDetailContinuation` rows and the two
  trailing `AlbumDetailRule` pushes (bottom rule + padding) on
  `!hero_handles_detail`, and only set `selected_block_bounds` inside that
  gate.

**Non-selected branch** (the `else { ... }` block, currently lines 351-439):

- Add `&& !hero_handles_detail` to both cursor-block conditions:
  `if idx == cursor && !selectable_headers && !expand_selected` and
  `else if idx == cursor && !selectable_headers`. When the hero handles the
  detail, the cursor album falls through to the plain `rows.push(Album(idx))`.

### Task 3.3: Bounds to None when `hero_handles_detail` is true

In the final `GroupedAlbumDisplayPlan` construction (currently lines 469-477),
force the bounds off:

```rust
selected_block_bounds: if hero_handles_detail {
    None
} else {
    selected_block_bounds
},
track_detail_bounds: if hero_handles_detail {
    None
} else {
    track_detail_bounds
},
```

(Defensive on top of the emission gating in 3.2 — makes the invariant explicit.)

### Task 3.4: Update callers

**`src/app/render/album.rs`** — `render_power_grouped_album_rows` (line 26):
add a `hero_handles_detail: bool` parameter **after `focused`, before
`layout`**, and pass it through to `build_grouped_album_display_plan` (the call
at line 78) in the new position (after `expand_selected`, before `wrap_widths`).

**`src/app/render/list.rs`** — the call to `render_power_grouped_album_rows` at
line 509 (inside `render_power_list`'s `show_grouped` branch): pass
`selected_album_item.is_some()` for the new parameter, with a comment: the hero
panel handles the album detail exactly when `selected_album_item` is present
(a music-group view at the album-browsing level). `selected_album_item` is in
scope (computed at line 164). Do NOT pass a literal `true` — plain
(non-music-group) album-folder browsing reaches this branch too, has no hero,
and must keep its inline expansion.

**`src/app/render/music.rs`** — the call at line 167 (the legacy
`render_power_music_group_view` path): pass `false` — this view has no hero
panel, so the inline expansion must stay (this path is deleted in a later
batch).

**`src/app/render/album_cursor.rs`** — both calls (lines 120 and 353) in
`music_group_navigation` and `artist_header_album_items_for_selection`: pass
`false`. These are navigation/member-resolution paths, not rendering; the
navigation targets derive only from `ArtistHeader` and `Album` rows, which are
emitted identically either way.

**`src/app/render/tests_music_groups.rs`** — both calls (lines 72 and 87): pass
`false` (the `hero_handles_detail: true` variants are task 7.1 in a later
batch).

## Files to Modify

- `src/app/render/album_plan.rs` — parameter + suppression + bounds override
- `src/app/render/album.rs` — thread parameter through
  `render_power_grouped_album_rows`
- `src/app/render/list.rs` — pass `selected_album_item.is_some()`
- `src/app/render/music.rs` — pass `false`
- `src/app/render/album_cursor.rs` — pass `false` at both call sites
- `src/app/render/tests_music_groups.rs` — pass `false` at both call sites
- `openspec/changes/music-library-hero-and-columns/tasks.md` — mark 3.1-3.4 as `[x]`

## Constraints

- Do NOT modify any rendering logic in `album.rs` beyond threading the new
  parameter. Do NOT modify `render_power_album_detail`, `render_inline_album_art`,
  or the hero painting in `list.rs`.
- Do NOT touch the cursor-movement column logic (`album_cursor.rs` movement
  functions) — that is batch 5.
- Keep the `ArtistActionHint` row in the selected-group branch when
  `hero_handles_detail` is true.
- All files stay under the 800-line cap (pre-commit enforced).
- Verify with (all prefixed with `rtk`, run in background):
  - `rtk cargo check -p mbv` (changes live in the `mbv` binary crate)
  - `rtk cargo check -p mbv-core`
  - `rtk cargo clippy --workspace --all-targets`
  - `rtk cargo test -p mbv render` (exercises `tests_music_groups.rs`,
    `list_tests.rs`, `tests_album_*.rs`)
  - `rtk make check-code-file-lines`
- Commit all changes as a single commit with message:
  `feat: suppress inline album expansion when hero handles detail`

## Report

Write your report to `.superpowers/sdd/music-library-hero-and-columns/task-3-report.md`
Include:
- Status: DONE, DONE_WITH_CONCERNS, NEEDS_CONTEXT, or BLOCKED
- What you changed (file paths and summary)
- Verification output (check/clippy/test results)
- Any concerns or deviations from the brief
