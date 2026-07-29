## Context

The right panel of the media browser renders several list variants (home, letter-grouped, plain, music/album, home-video), each implemented as a separate function. All variants receive a `focused: bool` parameter from `render_power_library` in `power_widgets.rs`. Four of the five variants correctly dim text from `WHITE` → `SUBTLE` when unfocused; two do not:

1. **`album.rs` — `render_power_grouped_album_rows`** (603 lines): The non-grouped album path (lines 371–384) always renders titles as `WHITE` regardless of focus. Both paths render year labels as `AQUA` and separators as `YELLOW` regardless of focus. The grouped-block path (lines 256–274) correctly dims titles but not years/separators.

2. **`home_video.rs` — `render_home_video_item`** (lines 111–117): Uses `palette::TEXT` (RGB 230,230,230) for the unfocused fallback instead of `palette::SUBTLE` (RGB 158,158,158), making unfocused items noticeably brighter than in other list variants.

The root cause is that `album.rs` builds its spans entirely inline without using the shared `build_list_row_spans` utility from `list_rows.rs`, and `home_video.rs` has its own inline color logic. The focus-dimming pattern (`if focused { WHITE } else { SUBTLE }`) is repeated independently in `list_plain.rs`, `list_letter_groups.rs`, `home.rs`, and partially in `album.rs` — a clear case for extraction.

## Goals / Non-Goals

**Goals:**
- Fix the music library focus dimming bug so album titles, years, and separators dim consistently when the panel loses focus.
- Fix the home video `TEXT` → `SUBTLE` inconsistency.
- Extract focus-aware color selection into named utility functions in `list_rows.rs` to prevent future drift.
- Decompose the 600-line `render_power_grouped_album_rows` into smaller, named helper functions for maintainability.

**Non-Goals:**
- Unifying `list_plain.rs` and `list_letter_groups.rs` (deferred to a separate change).
- Migrating `album.rs` to use `build_list_row_spans` (the album display model with artist headers, wrapped titles, and inline art is structurally different enough that forcing it through the shared row builder would add complexity without clear benefit).
- Changing the color palette or introducing new colors.
- Adding focus dimming to elements that intentionally don't dim (e.g., selected+focused items, action hints).

## Decisions

### 1. Extract color helpers as free functions in `list_rows.rs`

**Choice**: Three small `pub(super)` functions: `focused_or_subtle(focused) -> Color`, `focused_or_muted(focused) -> Color`, `focused_aqua_or_muted(focused) -> Color`.

**Rationale**: These capture the three focus-dimming transitions currently scattered across files:
- `WHITE ↔ SUBTLE` — primary text (used in 4 files)
- `YELLOW ↔ MUTED` — accent text like separators
- `AQUA ↔ MUTED` — secondary accent like year labels

**Alternative considered**: A `FocusPalette` struct with all color pairs. Rejected — over-engineered for three transitions, and the free functions compose naturally at call sites.

### 2. Fix album.rs inline, don't migrate to build_list_row_spans

**Choice**: Apply the new color utilities directly in `album.rs`'s existing inline span construction.

**Rationale**: `album.rs` has a fundamentally different display model — artist header rows, multi-line wrapped album titles, grouped blocks with borders, inline album art — that doesn't map to the flat `DisplayRow` enum used by `build_list_row_spans`. Forcing a migration would require significant restructuring for no user-visible benefit beyond what targeted color fixes achieve.

### 3. Decompose album.rs by display-row variant

**Choice**: Extract each `GroupedAlbumDisplayRow` match arm into its own function (e.g., `render_artist_header_row`, `render_album_row`, `render_album_action_hint`).

**Rationale**: The 600-line function is hard to reason about. Each variant is self-contained (reads different fields, produces different spans). Extracting by variant keeps each function under ~60 lines and makes the focus-dimming logic locally visible. The main function becomes a dispatch loop.

**Alternative considered**: Extract by concern (all color logic, all layout logic, etc.). Rejected — would scatter a single variant's logic across multiple functions, making it harder to understand any one row type end-to-end.

### 4. Fix home_video.rs with a one-line change

**Choice**: Replace the `palette::TEXT` fallback at line 116 with `focused_or_subtle(focused)` (returns `WHITE` when focused, `SUBTLE` when unfocused).

**Rationale**: Using the shared utility promotes consistency with other list renderers. The slight color shift from `TEXT` to `WHITE` when focused is acceptable and matches brightness conventions used elsewhere.

## Risks / Trade-offs

- **[Visual regression in album.rs]** → The decomposition is a pure refactor, but the focus-dimming changes alter visible behavior. Mitigation: manual visual verification with the panel focused and unfocused across all album display modes (grouped, non-grouped, with/without album art).

- **[Color utility adoption drift]** → New list renderers could still inline colors instead of using the utilities. Mitigation: the utilities are `pub(super)` within the render module, making them the path of least resistance. No enforcement mechanism beyond convention.

- **[home_video.rs focused-but-not-selected brightness]** → Changing to `SUBTLE` when unfocused may look too dim if the surrounding UI expects `TEXT` brightness for focused items. Mitigation: verify visually; if `TEXT` is preferred for focused state, the utility call `focused_or_subtle(focused)` already handles this correctly (returns `WHITE` when focused, which is close to `TEXT`).
