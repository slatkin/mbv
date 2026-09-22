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
