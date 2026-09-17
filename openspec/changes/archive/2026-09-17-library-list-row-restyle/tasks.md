## 1. Theme role and split-row painter

- [x] 1.1 Add `SPLIT_ROW_TITLE_FG = primitives::IRIS` to `src/app/render/theme/mod.rs` with its role comment (the item's own name in a split media-list row; distinct from the playback strip's `PLAYBACK_TITLE_FG` aqua).
- [x] 1.2 Resolve the split-row secondary title through `palette::SPLIT_ROW_TITLE_FG` in `src/app/render/components/media_list/row.rs` (the `parts` vec and the truncation branch), leaving the `▶` marker's `ACCENT` untouched.

## 2. Drop the duration from library owners

- [x] 2.1 `home_content.rs`: project `duration: None`; drop the dead `fmt_duration_short` use/import.
- [x] 2.2 `podcast_content.rs`: project `duration: None`; drop the dead `list_duration_secs` use/import.
- [x] 2.3 `tv_content/mod.rs`: project `duration: None`; drop the dead `list_duration_secs` use/import.
- [x] 2.4 `music_content.rs`: project `duration: None` in `build_track_rows`; drop the dead `list_duration_secs` use/import.
- [x] 2.5 `book_content.rs`: project `duration: None` in `chapter_rows`; drop the dead `list_duration_secs` use/import.
- [x] 2.6 `feeds_content.rs`: project `duration: None`; drop the dead `feed_duration_text` use/import (check `render/screens/feeds_model.rs` for a surviving caller before deleting the helper).

## 3. Tests

- [x] 3.1 Update library-owner row tests that pin a duration (home, podcast, tv, music, book, feeds) to assert no duration slot.
- [x] 3.2 Update the row-painter split-row palette pin from `PLAYBACK_TITLE_FG` to `SPLIT_ROW_TITLE_FG`, keeping the painter's queue-duration test as the surviving duration-slot owner.
- [x] 3.3 Update any mounted tick/buffer test that locates or asserts a library row's duration column.

## 4. Gates

- [x] 4.1 `cargo nextest run -p mbv -p mbv-core`, `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `openspec validate library-list-row-restyle`.
