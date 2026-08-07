# Task 4 Brief: Two-column packing for grouped album rows

## Tasks
- 4.1 Add `cols` parameter to `render_power_grouped_album_rows` in `src/app/render/album.rs:26`
- 4.2 Batch consecutive `Album` rows into column pairs - album `i` within an artist group occupies column `i % cols`, each pair shares a terminal row
- 4.3 Render `ArtistHeader` and `ArtistGroupSpacer` rows at full width, starting a fresh row
- 4.4 Each artist group packs independently - a row never mixes albums from two groups; a trailing odd album in a group leaves the partner cell empty
- 4.5 Pass `cols` from `render_power_list` in `src/app/render/list.rs` (already computed at line ~170 as `cols`)

## Key Context
- The rendering loop is at `src/app/render/album.rs:178-294`
- `GroupedAlbumDisplayRow::Album(idx)` rows need to be packed into columns
- Look at `src/app/render/list_letter_groups.rs` for the letter header pattern (full-width headers with grouped content)
- `cols` is already computed in `list.rs` around line 170-174

## Implementation Notes
The current rendering iterates through visible rows. For two-column packing:
1. Add `cols: u16` parameter to `render_power_grouped_album_rows`
2. Track when we're in an album group (between ArtistHeader/ArtistGroupSpacer boundaries)
3. When rendering Album rows, check if we should start a new row or continue in the next column
4. Use `Rect` with adjusted `x` and `width` for column positioning
5. Pass `cols` from `list.rs` when calling `render_power_grouped_album_rows`

## Verification
After implementing:
1. Run: `rtk cargo check -p mbv`
2. If it compiles, commit with: `feat: two-column packing for grouped album rows`
3. Write report to `.superpowers/sdd/music-library-hero-and-columns/task-4-report.md`
