# Baseline

## Production files

The exact governed production file list is:

```text
src/app/components/media_list/{mod,wide,carrier,anchor,selection,grouping}.rs src/app/components/music_tree{,_model,_view,_selection,_label}.rs
```

Pre-change line count command:

```sh
wc -l src/app/components/media_list/{mod,wide,carrier,anchor,selection,grouping}.rs src/app/components/music_tree{,_model,_view,_selection,_label}.rs
```

Pre-change output:

```text
   898 src/app/components/media_list/mod.rs
   349 src/app/components/media_list/wide.rs
   213 src/app/components/media_list/carrier.rs
    53 src/app/components/media_list/anchor.rs
   244 src/app/components/media_list/selection.rs
   178 src/app/components/media_list/grouping.rs
   428 src/app/components/music_tree.rs
   479 src/app/components/music_tree_model.rs
   484 src/app/components/music_tree_view.rs
   460 src/app/components/music_tree_selection.rs
   225 src/app/components/music_tree_label.rs
  4011 total
```

## Pre-change line coverage

Coverage command:

```sh
cargo llvm-cov -p mbv --json --summary-only --output-path target/extract-shared-list-components-before.json
```

The command completed successfully. The per-file covered-line numerator and
count-line denominator for the exact production file list were:

| File | Covered lines | Count lines |
| --- | ---: | ---: |
| `src/app/components/media_list/anchor.rs` | 19 | 20 |
| `src/app/components/media_list/carrier.rs` | 132 | 138 |
| `src/app/components/media_list/grouping.rs` | 117 | 117 |
| `src/app/components/media_list/mod.rs` | 428 | 449 |
| `src/app/components/media_list/selection.rs` | 188 | 196 |
| `src/app/components/media_list/wide.rs` | 193 | 210 |
| `src/app/components/music_tree.rs` | 207 | 210 |
| `src/app/components/music_tree_label.rs` | 140 | 140 |
| `src/app/components/music_tree_model.rs` | 266 | 273 |
| `src/app/components/music_tree_selection.rs` | 312 | 317 |
| `src/app/components/music_tree_view.rs` | 330 | 333 |
| **Sum** | **2332** | **2403** |

## After production file list (task 4.4)

The exact governed production file list after the extraction is the 1.1 list as it
now exists (plus `media_list/types.rs`, created by splitting `media_list/mod.rs`)
together with every new production file under `src/app/components/list/`. Test
files are excluded.

After line count command:

```sh
wc -l src/app/components/media_list/{mod,wide,carrier,anchor,selection,grouping,types}.rs src/app/components/music_tree{,_model,_view,_selection,_label}.rs src/app/components/list/{mod,row_flow,cursor,viewport,paint,marks,expandable}.rs
```

After output:

```text
   496 src/app/components/media_list/mod.rs
   407 src/app/components/media_list/wide.rs
   213 src/app/components/media_list/carrier.rs
    53 src/app/components/media_list/anchor.rs
   267 src/app/components/media_list/selection.rs
   178 src/app/components/media_list/grouping.rs
   447 src/app/components/media_list/types.rs
   506 src/app/components/music_tree.rs
   488 src/app/components/music_tree_model.rs
   569 src/app/components/music_tree_view.rs
   649 src/app/components/music_tree_selection.rs
   224 src/app/components/music_tree_label.rs
    89 src/app/components/list/mod.rs
   137 src/app/components/list/row_flow.rs
   200 src/app/components/list/cursor.rs
   379 src/app/components/list/viewport.rs
   275 src/app/components/list/paint.rs
   220 src/app/components/list/marks.rs
   214 src/app/components/list/expandable.rs
  6011 total
```

Per-file before/after line counts (before `0` = new file; `types.rs` is the
split-out descendant of the pre-change `media_list/mod.rs`):

| File | Before | After | Delta |
| --- | ---: | ---: | ---: |
| `src/app/components/media_list/mod.rs` | 898 | 496 | -402 |
| `src/app/components/media_list/types.rs` | 0 | 447 | +447 |
| `src/app/components/media_list/wide.rs` | 349 | 407 | +58 |
| `src/app/components/media_list/carrier.rs` | 213 | 213 | 0 |
| `src/app/components/media_list/anchor.rs` | 53 | 53 | 0 |
| `src/app/components/media_list/selection.rs` | 244 | 267 | +23 |
| `src/app/components/media_list/grouping.rs` | 178 | 178 | 0 |
| `src/app/components/music_tree.rs` | 428 | 506 | +78 |
| `src/app/components/music_tree_model.rs` | 479 | 488 | +9 |
| `src/app/components/music_tree_view.rs` | 484 | 569 | +85 |
| `src/app/components/music_tree_selection.rs` | 460 | 649 | +189 |
| `src/app/components/music_tree_label.rs` | 225 | 224 | -1 |
| `src/app/components/list/mod.rs` | 0 | 89 | +89 |
| `src/app/components/list/row_flow.rs` | 0 | 137 | +137 |
| `src/app/components/list/cursor.rs` | 0 | 200 | +200 |
| `src/app/components/list/viewport.rs` | 0 | 379 | +379 |
| `src/app/components/list/paint.rs` | 0 | 275 | +275 |
| `src/app/components/list/marks.rs` | 0 | 220 | +220 |
| `src/app/components/list/expandable.rs` | 0 | 214 | +214 |
| **Sum** | **4011** | **6011** | **+2000** |

### 4.4 follow-up analysis

The after total is **not lower**: 4011 -> 6011 (**+2000**). This is diagnostic,
not a threshold; the breakdown and the direct duplication evidence follow.

**Where the seam added lines.** The new `src/app/components/list/` seam is 1514
lines across seven files: `viewport.rs` 379, `paint.rs` 275, `marks.rs` 220,
`expandable.rs` 214, `cursor.rs` 200, `row_flow.rs` 137, `mod.rs` 89. The
remaining +486 is growth in the descended files (4497 after vs 4011 before):
`media_list/mod.rs` lost 402 to the `types.rs` split while `types.rs` added 447,
and the shape files grew on their new shared-trait impl blocks, the target-surface
rename, and the seam adapters. The seam is additive code that the two shapes now
delegate to, not a replacement that compresses the shapes by more than it costs.

**Direct evidence the duplicated mechanics named in 4.1-4.3 are gone.**

- 4.1 (cursor / viewport / keep-visible arithmetic): every shape entry point now
  delegates to the seam and keeps only primitive adapters plus named paging
  policy. Delegating call sites: `media_list/mod.rs` `Cursored::move_by` (165),
  `Cursored::first` (173), `Cursored::last` (181),
  `Cursored::select_selectable_ordinal` (190),
  `Viewported::resolved_viewport_offset` (217), `Viewported::clamp_viewport` (450);
  `music_tree_view.rs` `Cursored::move_by` (109), `Viewported::page` with
  `PagingPolicy::VisibleViewport` (135), `Cursored::first` (146),
  `Cursored::last` (151); `music_tree_selection.rs` `Viewported::clamp_viewport`
  (474). `WideMediaList`'s public `move_selection` / `select_*` / `resolve_viewport`
  and `MediaListCarrier`'s wrappers are one-line forwards to the owner (no
  arithmetic bodies), and no shape-local `keep_visible` / `ensure_visible` /
  `clamp_offset` helper exists.
- 4.2 (paint carrier):
  `grep -RInE 'WidePaintResult|paint_complete' src/app/components/media_list src/app/components/music_tree*.rs`
  finds nothing (exit 1), as does a wider search for the removed
  `WidePaintGeometry` carrier. Both shapes call the single
  `PaintRetained::finish` (`media_list/wide.rs:125`, `music_tree_view.rs:480`)
  into one `PaintRetainedState` field (`media_list/wide.rs:64`,
  `music_tree.rs:180`); there is no second paint-result carrier or point resolution.
- 4.3 (ordered-mark carrier):
  `grep -RInE 'selection_order' src/app/components/media_list src/app/components/music_tree*.rs`
  finds nothing (exit 1). Both owners hold the seam's
  `MarkSelectionState` (`media_list/mod.rs` `multi_selection`,
  `music_tree.rs` `marks`), and `MarkSelection::action_targets` drives display
  order from it. The flat-only Visual range/anchor fields (`selection_anchor`,
  `frozen_selection`, `live_range`) remain local to `MediaList`.

**Conclusion.** The increase is justified as the cost of the shared seam; the
named duplicate mechanics are gone, so no duplication remains to remove for 4.4.

## After line coverage (task 4.5)

Coverage command:

```sh
cargo llvm-cov -p mbv --json --summary-only --output-path target/extract-shared-list-components-after.json
```

The command completed successfully (2116 tests passed). The per-file covered-line
numerator and count-line denominator for the exact after production file list are:

| File | Before covered | Before count | After covered | After count |
| --- | ---: | ---: | ---: | ---: |
| `src/app/components/media_list/anchor.rs` | 19 | 20 | 19 | 20 |
| `src/app/components/media_list/carrier.rs` | 132 | 138 | 132 | 138 |
| `src/app/components/media_list/grouping.rs` | 117 | 117 | 117 | 117 |
| `src/app/components/media_list/mod.rs` | 428 | 449 | 301 | 321 |
| `src/app/components/media_list/selection.rs` | 188 | 196 | 206 | 214 |
| `src/app/components/media_list/types.rs` | 0 | 0 | 146 | 154 |
| `src/app/components/media_list/wide.rs` | 193 | 210 | 191 | 236 |
| `src/app/components/music_tree.rs` | 207 | 210 | 237 | 245 |
| `src/app/components/music_tree_label.rs` | 140 | 140 | 140 | 140 |
| `src/app/components/music_tree_model.rs` | 266 | 273 | 272 | 279 |
| `src/app/components/music_tree_selection.rs` | 312 | 317 | 426 | 452 |
| `src/app/components/music_tree_view.rs` | 330 | 333 | 386 | 391 |
| `src/app/components/list/mod.rs` | 0 | 0 | 30 | 30 |
| `src/app/components/list/row_flow.rs` | 0 | 0 | 81 | 81 |
| `src/app/components/list/cursor.rs` | 0 | 0 | 101 | 103 |
| `src/app/components/list/viewport.rs` | 0 | 0 | 179 | 198 |
| `src/app/components/list/paint.rs` | 0 | 0 | 142 | 151 |
| `src/app/components/list/marks.rs` | 0 | 0 | 114 | 127 |
| `src/app/components/list/expandable.rs` | 0 | 0 | 111 | 124 |
| **Sum** | **2332** | **2403** | **3331** | **3521** |

Aggregate comparison: covered lines **2332 -> 3331** and count lines
**2403 -> 3521**. The governed subsystem's aggregate covered-line total is above
the 1.1 baseline (3331 >= 2332), so 4.5's gate is met; no shortfall tests were
needed. The covered-line rate moved from 97.1% (2332/2403) to 94.6% (3331/3521)
because the new seam and its trait impls add counted lines that the aggressive
movement/carrier tests do not fully exercise; the absolute covered total, which
is the recorded baseline comparison, rose by 999. Every `music_tree*` file's
covered-line count rose, so the U4b carried watch-items (cached-track `.level()`
depth and post-reset re-intern coverage) are covered at or above baseline and no
focused restoration was required.
