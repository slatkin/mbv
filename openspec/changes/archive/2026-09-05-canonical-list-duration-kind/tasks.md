## 1. Core model + painter

- [x] 1.1 Add `MediaKind { Collection, Media }` field on `MediaListRow::Item` and `list_duration_secs(i64) -> Option<String>` helper over `fmt_duration_short`; verify `cargo check -p mbv` passes
- [x] 1.2 Painter suppresses `duration` for `Collection` rows + buffer test (Collection with duration projects nothing, Media paints `4:32` right-aligned green); verify new buffer test passes

## 2. Migrate projections

- [x] 2.1 TV episodes + music tracks + book chapters/files from `fmt_duration_approx` to helper with `Media` kind; verify `cargo nextest run -p mbv` passes and updated characterization tests show `M:SS`
- [x] 2.2 Browser/Movies/generic, podcast shows/episodes, book titles, albums, TV series rows get explicit kind (`Collection` for containers, `Media` for playable leaves), `duration: None` unchanged; verify `cargo check` + existing buffer tests pass unchanged
- [x] 2.3 Fold `fmt_duration_mmss` (selection modal) and `feeds_model::format_duration` into helper, delete both; verify `rg fmt_duration_mmss|format_duration` shows no production callers and full test suite passes

## 3. Gates

- [x] 3.1 Run `cargo clippy --workspace --all-targets`, `ast-grep scan`, `cargo fmt --all -- --check`; verify all clean
