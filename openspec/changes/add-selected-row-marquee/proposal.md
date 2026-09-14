## Why

Every canonical media-list row ellipsis-truncates a title that doesn't fit, including the selected row, so a long title is permanently unreadable in full. The player strip already solves this for the Now Playing / idle-feed title with a bounded back-and-forth marquee instead of truncating. Extending that same treatment to the selected row of a canonical media list lets the one row the user is actually looking at reveal its full title over time, while every other row keeps truncating as it does today.

## What Changes

- The canonical media-list row painter (currently `wide_media_row` in `src/app/render/components/media_list/wide_row.rs`; expected to be renamed `media_list_row` by a preceding, unrelated change — this proposal targets whichever name that function carries at implementation time) marquees the title of the row that is both selected and focused when its title overflows its slot, instead of ellipsis-truncating it.
- Every other row — unselected, selected-but-unfocused, or not overflowing — keeps today's `trunc_str` ellipsis truncation unchanged.
- The marquee animation (hold / scroll / hold / scroll-back, ~200ms per column, ~1200ms holds) reuses the existing cadence from `chrome_player.rs`'s Now Playing / idle-feed marquee rather than inventing a second timing model; the shared column-advance math is extracted out of `chrome_player.rs` so both call sites use one implementation.
- Marquee phase/clock is owned per list (alongside the existing cursor/scroll owned by `MediaList<Target>`), not the shell-global marquee clock the player strip uses — each list's selected-row marquee runs independently and resets to the start whenever the marqueed text changes (new selection or new content).
- Scope is limited to the canonical `MediaList<Target>` row framework (`WideMediaList`, `InlineMediaBrowser`, and the Queue panel, which all paint through the one shared row painter). The legacy grid/letter-group painters (`plain_rows.rs`, `list_rows.rs`, `list_letter_groups.rs` — Home, search results, artists A-Z) are unchanged; the grid painter already has its own, different overflow treatment (showcase the full title in a detail block instead of truncating) and is out of scope here.

## Capabilities

### New Capabilities
(none)

### Modified Capabilities
- `canonical-media-lists`: the shared row painter's selected-row title behavior changes from always-truncate to marquee-when-selected-and-focused-and-overflowing.

## Impact

- `src/app/render/components/media_list/wide_row.rs` (or its post-rename equivalent `media_list_row.rs`/fn) — title rendering for the selected+focused row.
- `src/app/render/components/chrome_player.rs` — marquee column-advance/window logic extracted to a shared location instead of staying private here.
- `src/app/components/media_list/mod.rs` (`MediaList<Target>`) — gains per-list marquee clock state (text + start instant), mirroring the existing cursor/scroll ownership.
- `src/app/components/media_list/wide.rs`, `inline.rs` — thread the marquee clock through to the row painter for the selected row only.
- No change to `plain_rows.rs`, `list_rows.rs`, `list_letter_groups.rs`, or the legacy grid showcase behavior.
