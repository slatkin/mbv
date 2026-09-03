# Tasks 4.2-4.5 — consolidated slice acceptance evidence

This record consolidates the one-painter source trace, the source-level absence
proofs, the test inventory, and the automated gate results for the Home/Feeds
canonical-media-list slice, scoped against the in-tree `173bdba1`/`400c0b59`
wiring this slice reworked. Live Wide/Narrow acceptance (4.6) is the human's and
is not covered here.

Slice commit range: `45b43f63..b5fccf68`

- `83e35de2` feat(home): compose canonical media-list controls (2.1-2.3)
- `003730d9` fix(home): preserve legacy Home row content in canonical projection
- `c4e25ca0` feat(feeds): compose canonical media-list controls (3.1-3.4)
- plus docs commits `f793d669`, `b0603657`, and this evidence commit.

HEAD at gate run: `b5fccf68`.

## One-painter evidence (4.2)

Exactly one list painter runs per destination, selected XOR by breakpoint.

### Home

`HomeComponent::view()` no longer calls either list painter directly:

```text
$ rtk grep -n "render_wide_media_list\|render_inline_media_browser" src/app/components/home.rs
(exit 1 — no matches)
```

The only caller is `render_home_content` in the render seam, once each, on
mutually exclusive branches of the `two_column` breakpoint:

```text
$ rtk grep -n "render_wide_media_list\|render_inline_media_browser" src/app/render/components/home.rs
460:        super::media_list::render_wide_media_list(
474:        let result = super::media_list::render_inline_media_browser(
```

`src/app/render/components/home.rs:455-473` is a single
`if control_empty { placeholder } else if two_column { render_wide_media_list }
else { render_inline_media_browser }` chain — one painter per frame.

### Feeds

`FeedsComponent::view()` no longer calls either list painter directly:

```text
$ rtk grep -n "render_wide_media_list\|render_inline_media_browser" src/app/components/feeds.rs
(exit 1 — no matches)
```

The only caller is `render_feeds_content`, once each, on the mutually exclusive
`wide` branch (`src/app/render/components/feeds.rs:220-247`):

```text
$ rtk grep -n "render_wide_media_list\|render_inline_media_browser" src/app/render/components/feeds.rs
20:use super::media_list::{render_inline_media_browser, render_wide_media_list};
221:        let offset = render_wide_media_list(
240:        let result = render_inline_media_browser(
```

## Absence proofs (4.2)

Scoped to `src/app/components/{home,feeds}.rs` and
`src/app/render/components/{home,feeds}.rs`.

### No parent underpaint beneath the control

The legacy Home list-row painters and the Feeds entry-row loop are deleted, not
merely bypassed. Deleted files: `src/app/render/components/home_list_rows.rs`
(`render_home_list_rows` + `DisplayRow`), `src/app/render/components/feed_row.rs`
(`render_feed_entry_cell`). Deleted symbols also include
`render_home_emby_row`, `render_home_latest_row`, `home_title_color`,
`home_panel_scroll`, `arrangements::home::inline_hero_area`,
`widgets::content_width`, `pack_feed_rows`/`PackedFeedRow`.

```text
$ rtk grep -rn "render_home_list_rows\|render_home_emby_row\|render_home_latest_row\|home_title_color\|home_panel_scroll\|fn content_width\|fn inline_hero_area" src/
(exit 1 — no matches)

$ rtk grep -rn "render_feed_entry_cell\|pack_feed_rows\|PackedFeedRow" src/
(exit 1 — no matches)

$ ls src/app/render/components/home_list_rows.rs src/app/render/components/feed_row.rs
ls: cannot access 'src/app/render/components/home_list_rows.rs': No such file or directory
ls: cannot access 'src/app/render/components/feed_row.rs': No such file or directory
```

(`src/app/render/components/home_latest_row.rs` survives — it supplies hero
detail text (`home_latest_detail_text`), not list rows.)

### No `set_scroll(` render-offset write-back from the render seam

```text
$ rtk grep -rn "set_scroll" src/app/render/ src/app/components/home.rs src/app/components/feeds.rs
(exit 1 — no matches)
```

`render_wide_media_list` / `render_inline_media_browser` still *return* a resolved
offset, but neither render seam nor either component stores it back into the
control. Home discards it; `FeedsComponent` keeps it only as `painted_offset`
for observability (`scroll()` accessor, characterization tests) and never feeds
it back.

### No parent `cursor: usize` / `scroll: usize` mirror fields

```text
$ rtk grep -n "cursor: usize\|scroll: usize" src/app/components/home.rs src/app/components/feeds.rs
(exit 1 — no matches)
```

`HomeComponent::cursor()` derives the flat cursor from the active control's
selectable index (`src/app/components/home.rs:223-230`); `FeedsComponent::cursor()`
reads the active control directly (`src/app/components/feeds.rs:115-121`). Neither
struct holds a numeric cursor or scroll field.

### No per-frame child control construction

```text
$ rtk grep -n "WideMediaList::new()\|InlineMediaBrowser::new()" src/app/components/home.rs src/app/components/feeds.rs src/app/render/components/home.rs src/app/render/components/feeds.rs
src/app/components/home.rs:106:            canonical_list: WideMediaList::new(),
src/app/components/home.rs:107:            inline_list: InlineMediaBrowser::new(),
src/app/components/feeds.rs:69:            canonical_list: WideMediaList::new(),
src/app/components/feeds.rs:70:            inline_list: InlineMediaBrowser::new(),
```

All four constructions are in `HomeComponent::new()` / `FeedsComponent::new()`.
The controls are persistent `struct` fields; `view()` and the render seam borrow
them, never build them.

### No second router / no key handling added in the components

```text
$ git diff 45b43f63 b5fccf68 -- src/app/components/home.rs src/app/components/feeds.rs \
    | grep -E "^[+-] " | grep -iE "Application|EventListener|Sub::|Router|route\("
(no matches)
```

`handle_key` on both components is the pre-existing `173bdba1` local-navigation
interpreter: it moves the canonical control and emits typed `Msg::Shell(...)`
requests. This slice's only key-arm edits are mechanical (`self.cursor` ->
`self.cursor()`, `Left`/`Right` now delegate to `move_selection`). Destination-
independent chords stay in `router.rs` / `key_policy.rs`, untouched.

### No global hit map / no new mouse wiring

```text
$ rtk grep -n "MouseGestureState\|HitRegions<\|subscribe.*Mouse\|Sub::new" \
    src/app/components/home.rs src/app/components/feeds.rs \
    src/app/render/components/home.rs src/app/render/components/feeds.rs
(exit 1 — no matches)
```

The `home_hitmap` rebuild in `render/components/home.rs:573-588` and the
`rebuild_selectable_maps` rebuild in `render/components/feeds.rs:302-323`
repopulate the bespoke `*HitRegion` / `layout.left_row_map` /
`layout.left_item_rows` structures **from the control-exported `RowGeometry`**.
This is authorized pre-#638 mouse compatibility — `restore-mouse-support` (#638)
owns migrating it — not new mouse wiring.

### No callback/provider framework or authority leak into the controls

```text
$ rtk grep -n "App\b\|ServiceClient\|Player\|EmbyClient\|Application<\|Router" \
    src/app/render/components/home.rs src/app/render/components/feeds.rs \
    | grep -v "AppLayout\|HomeImagePaint\|//"
(exit 1 — no matches)
```

Both render seams take plain data (`&[QueueItem]`, `FeedsRenderModel<'_>`,
`&WideMediaList<String>`, `&InlineMediaBrowser<String>`) plus a `Frame`/`Rect`.
The image pixel paint is the only deferred effect and is returned as data
(`HomeImagePaint`) for the shell to apply — no `App`, Service-client, or
Player-authority handle crosses into the controls.

## Stateful / rendered / geometry tests (cross-ref 4.1)

### Home — `src/app/components/home_component_tests.rs` (3 added)

- `active_section_projects_item_rows_with_content_and_parallel_indices` —
  only the active section is projected as canonical `Item` rows; `primary` =
  `display_name()`, `duration` formatted, resume `%` in `trailing`,
  `semantic_state` stays `Ordinary`; selectable index == flat index (Home has no
  `Heading`/`Spacer`).
- `ordinary_refresh_preserves_target_and_locally_clamps` — repeated `set_content`
  at an unchanged section keeps the control's selected target by id and clamps
  locally, with no parent cursor/scroll input.
- `breakpoint_transition_hands_off_one_viewport_anchor` — a Wide<->Narrow flip
  performs exactly one `ViewportAnchor` handoff carrying target + row offset.

### Feeds — `src/app/components/feeds_component_tests.rs` (2 added)

- `structural_rows_are_non_selectable_and_cursor_movement_skips_them` —
  `FeedAgeGroup` labels project to `Heading`, separators to `Spacer`; both are
  excluded from the selectable index, so cursor movement skips them (display
  index vs selectable index diverge as expected).
- `breakpoint_flip_carries_one_viewport_anchor` — one `ViewportAnchor` handoff on
  a real Wide<->Narrow flip.

### Retention test updated — `src/app/shell_home.rs`

- `shell_sync_keeps_home_component_cursor_local` — now uses
  `make_items(2)` so the two Continue-Watching rows have distinct ids,
  exercising id-based selection retention across a shell sync.

### Rendered characterization rewritten — `src/app/render/tests_feeds.rs` (2)

- `wide_feeds_use_a_left_detail_and_right_entry_workspace` — rewritten to
  characterize the canonical painter's left-detail / right-entry split and
  aligned entry rows. (Was failing pre-slice; now passes.)
- `wide_feeds_reserve_borders_at_the_scrolled_bottom_boundary` — rewritten to
  assert the reserved rail borders survive at the scrolled bottom boundary.
  (Was failing pre-slice; now passes.)

## File-size gate (4.3)

```text
$ rtk make check-code-file-lines
./scripts/check-code-file-lines.sh
code-file-lines: src/app/shell_home.rs has 801 lines (maximum 800)
make: *** [Makefile:21: check-code-file-lines] Error 1
```

The single flag is **pre-existing and out-of-campaign**. `src/app/shell_home.rs`
was **804 lines** at the slice base `45b43f63`; this slice removed 3 lines from
it (mechanical test-fixture change in `003730d9`), leaving it at **801** — the
slice moved the file *toward* the limit's compliance, not over it. Splitting it
is tracked outside this slice.

Every other source file changed by this slice is <= 800:

| File | Lines |
| --- | --- |
| `src/app/components/feeds.rs` | 451 |
| `src/app/components/feeds_component_tests.rs` | 643 |
| `src/app/components/home.rs` | 641 |
| `src/app/components/home_component_tests.rs` | 456 |
| `src/app/render/components/feeds.rs` | 323 |
| `src/app/render/components/home.rs` | 588 |
| `src/app/render/components/home_latest_row.rs` | 426 |
| `src/app/render/components/widgets.rs` | 606 |
| `src/app/render/mod.rs` | 237 |
| `src/app/render/screens/feeds_model.rs` | 232 |
| `src/app/render/tests_feeds.rs` | 301 |

## Strict validation (4.4)

```text
$ rtk openspec validate migrate-home-feeds-to-canonical-lists --strict
Change 'migrate-home-feeds-to-canonical-lists' is valid
```

## Workspace gates (4.5)

| Gate | Result |
| --- | --- |
| `rtk cargo fmt --all -- --check` | PASS — no output, exit 0 |
| `rtk cargo check --workspace --all-targets` | PASS — 0 errors; only the 3 pre-existing dead-code warnings (`movies_wide_area`, `render_home_video_item`, `has_group_pills`) |
| `rtk cargo nextest run -p mbv` | PASS — `Summary [7.775s] 1252 tests run: 1252 passed, 0 skipped` |

`mbv-core` was not touched by this slice (no `crates/mbv-core/` file in the slice
diff), so its suite was not re-run for this gate.

0 test failures. The 2 `wide_feeds_*` characterization tests that failed on the
pre-slice baseline were rewritten against the canonical painter output and now
pass.
