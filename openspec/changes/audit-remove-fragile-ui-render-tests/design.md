## Context

The `src/app/render/` directory contains 13 test files totaling 7,371 lines with ~110 test functions. These tests render TUI output using ratatui's `TestBackend` and assert on the resulting character buffer. Many tests assert on cosmetic details that change frequently during UI development: specific Unicode glyphs (`▌`, `▕`, `▁`, `▔`), exact color palette values at pixel coordinates, hardcoded indentation strings, font style modifiers, and art rect dimensions.

Additionally, every test file duplicates the same ~100 lines of helper functions (`buffer_to_string`, `render_power_library_to_terminal`, `render_power_scrollbar_column`, `make_power_movie_app`, `make_power_music_group_app`, etc.), resulting in ~1,300 lines of duplicated code.

The design language is not yet stabilized. Tests asserting on visual details break on nearly every UI change, creating maintenance friction without catching real regressions.

## Goals / Non-Goals

**Goals:**

- Remove tests that assert on cosmetic/visual details that are not set in stone.
- Retain tests that verify behavioral correctness (data logic, state transitions, geometry, scroll behavior, layout math).
- Consolidate duplicated test helpers into a shared module to reduce maintenance burden.
- Produce a clear classification of every test as "cosmetic" (remove) or "behavioral" (keep).

**Non-Goals:**

- Changing any production rendering code.
- Adding new tests or improving coverage.
- Stabilizing the design language (that's a separate effort).
- Removing tests in other directories (`crates/`, `overlays/`, `card.rs`) — scope is `src/app/render/` test files only.

## Decisions

**1. Classification criteria: cosmetic vs behavioral**

A test is **cosmetic** (remove) if its primary assertions check:
- Specific Unicode glyph characters at specific positions (e.g., `▌`, `▕`, `▁`, `▔`, `♥`, `🖧`, `🖭`)
- Exact color palette values at pixel coordinates (e.g., `assert_eq!(buffer[(x, y)].fg, palette::AQUA)`)
- Hardcoded visual indentation strings (e.g., `"        ▌2. Focused Track"`)
- Font style modifiers (e.g., `Modifier::BOLD` on specific text spans)
- Art/image rect dimensions that reflect design choices (e.g., `(30, 15)`)
- Exact string matches on rendered output that encode visual layout (e.g., `"|| X >> Title"`, `"RES 1080p  AUD en  SUB off"`)

A test is **behavioral** (keep) if its primary assertions check:
- Data resolution logic (e.g., which item is selected, which host label is used)
- State transitions (e.g., toast expiry clears state, session state after switching)
- Geometry/hitbox correctness (e.g., hitbox covers the right area, layout areas don't overlap)
- Scroll behavior (e.g., scrolling reveals the correct row, scroll bounds)
- Layout math (e.g., `content_rows()` returns correct value)
- String composition logic independent of visual position (e.g., remote status label text)

**Edge cases**: Some tests mix cosmetic and behavioral assertions. For these:
- If the behavioral assertion can be extracted without the cosmetic scaffolding, keep only the behavioral part.
- If the test is primarily cosmetic with a minor behavioral check, remove it — the behavioral check is incidental.
- When in doubt, keep the test.

**2. Helper consolidation strategy**

Extract shared helpers into `src/app/render/test_helpers.rs` (included via `include!` like the existing test files):

| Helper | Currently duplicated in | Purpose |
|--------|------------------------|---------|
| `buffer_to_string` | All 13 files | Convert terminal buffer to string |
| `render_power_library_to_terminal` | 8 files | Render library panel to terminal |
| `render_power_library_to_terminal_focused` | 8 files | Render with focus control |
| `render_power_library_to_string` | 8 files | Render library to string |
| `render_power_view_to_terminal` | 8 files | Render main view |
| `render_power_view` | 8 files | Render main view, return layout |
| `render_sidebar_scrollbar_column` | 10 files | Render scrollbar for testing |
| `render_power_scrollbar_column` | 10 files | Render power scrollbar |
| `render_power_scrollbar_column_with_viewport` | 10 files | Render scrollbar with viewport |
| `render_pill_bar_hitboxes` | 8 files | Render pill bar, return hitboxes |
| `make_power_movie_app` | 8 files | Create test app with movie library |
| `make_power_queue_app` | 8 files | Create test app with queue |
| `make_power_remote_queue_app` | 8 files | Create test app with remote queue |
| `make_power_music_group_app` | 8 files | Create test app with music groups |
| `make_power_home_video_app` | 8 files | Create test app with home videos |
| `make_power_large_movie_library_app` | 8 files | Create test app with large library |

File-specific helpers (e.g., `render_power_list_to_string` in `list_tests.rs`, `render_power_compact_detail_to_string` in `detail_tests.rs`) stay in their respective files.

**3. Include mechanism**

Use the existing `include!` pattern. Each test file already uses `include!` via `#[cfg(test)] mod tests` in `render/mod.rs`. Add `include!("test_helpers.rs")` before the test file includes so helpers are available to all.

**Alternative considered**: Converting to a `test_helpers` module with `use super::test_helpers::*`. Rejected because the existing `include!` pattern puts everything in the same scope, avoiding import complexity.

## Risks / Trade-offs

- **[Accidental removal of behavioral test]** → Mitigated by per-test classification with the decision tree above. When in doubt, keep.
- **[Helper extraction breaks test isolation]** → All helpers are pure functions with no shared mutable state. Extracting them is safe.
- **[Large diff from helper dedup]** → The diff will be large (~1,300 deletions, ~150 additions) but mechanically verifiable with `cargo test`.
- **[Future tests re-introduce cosmetic assertions]** → No enforcement mechanism. This is a cultural norm, not a lint. Accept this trade-off.
