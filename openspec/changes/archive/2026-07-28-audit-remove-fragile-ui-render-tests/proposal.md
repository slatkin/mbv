## Why

UI rendering tests in `src/app/render/` assert on cosmetic details — exact glyph characters (`▌`, `▕`, `▁`, `▔`, `♥`, `🖧`), pixel-level color values at specific coordinates, hardcoded indentation strings, and precise art rect dimensions. These visual properties change constantly during UI development, causing tests to break on every tweak. A test that lives two days is worthless until the design language stabilizes. Recent example: 3 tests deleted that asserted on selection marker gutters (`▌`), focus indicators, and inline art positioning — all cosmetic details that had already changed since the tests were written.

## What Changes

- **Audit all ~110 render tests** across 13 test files (7,371 lines total) in `src/app/render/` to classify each as cosmetic (asserts on visual appearance) or behavioral (asserts on logic, state, or geometry).
- **Remove cosmetic tests** that assert on non-functional visual output: glyph characters, color palette values at specific pixels, exact indentation strings, font style modifiers, and art rect dimensions.
- **Keep behavioral tests** that verify logic independent of visual styling: data resolution, state transitions, hitbox geometry, scroll behavior, layout math, and string composition.
- **Consolidate duplicated test helpers**: The same ~100 lines of helper functions (`buffer_to_string`, `render_power_library_to_terminal`, `make_power_movie_app`, etc.) are copy-pasted across all 13 test files. Extract shared helpers into a single module.
- **No production code changes** — test-only modifications.

## Capabilities

### New Capabilities

(none — this is test removal and helper consolidation, not a feature change)

### Modified Capabilities

(none — no spec-level behavior changes)

## Impact

- **Files modified**: Up to 13 test files in `src/app/render/` (`tests.rs`, `tests_album_focus.rs`, `tests_album_listing.rs`, `tests_album_detail.rs`, `tests_music_groups.rs`, `tests_music_detail.rs`, `tests_non_music.rs`, `tests_panel.rs`, `tests_queue.rs`, `tests_scroll_pills.rs`, `home_tests.rs`, `detail_tests.rs`, `list_tests.rs`), plus a new shared test helpers module.
- **Tests removed**: Estimated 40–60 cosmetic tests deleted.
- **Tests retained**: Estimated 50–70 behavioral tests preserved.
- **Helper deduplication**: ~1,300 lines of duplicated helpers across 13 files reduced to a single shared module (~150 lines).
- **CI**: `cargo test` continues to pass; fewer tests means faster feedback.
- **Risk**: Low — test-only changes. Risk of accidentally removing a behavioral test mitigated by per-test classification before deletion.
