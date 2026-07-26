> Suggested implementer tier per pass (OpenAI model tier; substitute your own
> naming as needed): 5.4-mini = mechanical/low-reasoning work, 5.4 = standard
> implementation, 5.6 luna = architecture/replanning judgment calls.

**Fixtures** (reuse these, do not invent new ones):
- `make_power_music_group_app` — `src/app/render/tests.rs:1097`
- `make_power_music_album_list_app(count, cursor)` — `src/app/input_power_music_track_focus_tests.rs:109`
- `buffer_to_string` — `src/app/render/tests.rs:11`

## 1. Remove the duplicated artist row (Pass 0)

Suggested implementer tier: 5.4-mini (small, closed-form deletions and two test-expectation fixes; no design judgment required).

This is issue #371's literal fix and cannot regress independently of the rest of the change.

- [x] 1.1 Delete `GroupedAlbumDisplayRow::AlbumArtist` (`src/app/render/album_plan.rs`) and its three producer sites (currently `album_plan.rs:258`, `:292`, `:322`). **Each site is a pair of statements**: `rows.push(GroupedAlbumDisplayRow::AlbumArtist(idx))` immediately followed by `rows.extend(std::iter::repeat_n(GroupedAlbumDisplayRow::AlbumWrappedContinuation, selected_artist_lines(idx).saturating_sub(1)))`. Delete **both** statements at each of the three sites — deleting only the `push` compiles cleanly but silently leaves blank continuation rows in the block. Once all three sites are gone, `selected_artist_lines` and `album_artist_labels` have no callers; delete them here. Leave `App::album_artist_label` for task 1.2 (it loses its callers there). Keep `resolve_group_album_artist`, `album_artist_cache`, and `AlbumArtistFetched` — grouping still depends on them.
- [x] 1.2 Delete the `AlbumArtist(idx)` render match arm in `src/app/render/album.rs` and its `left_row_map` arm. Delete `App::album_artist_label` once this removes its last callers. Simplify `selected_art_abs_rows`'s `title_offset` calculation and the `Album` row's `has_block` first-line calculation now that no artist row is emitted inside the selected block.
- [x] 1.3 Fix the now-stale row-count expectations and comments in the PageUp/PageDown tests in `src/app/input_power_music_track_focus_tests.rs` (they currently count the row the `AlbumArtist` variant occupied). `cargo test` will surface exactly which assertions shift by one row.
- [x] 1.4 Add a regression test that plain/search album selections retain per-album framing without a duplicated artist-name row.
- [x] 1.5 Verify: `cargo test -p mbv --bin mbv`, `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, and `rg 'GroupedAlbumDisplayRow::AlbumArtist|AlbumArtist\(' src` returns no matches.

## 2. Artist-scoped selection frame and inline discography window (Pass 1)

Suggested implementer tier: 5.4 (multi-file planner/renderer change against an already-detailed spec; standard implementation, no open design questions left).

- [x] 2.1 In `src/app/render/album_plan.rs`, merge the artist-header-focus and album-focus selected-block producers into one path: for the selected artist group, emit one artist row, one pinned target-sensitive hint row, then the complete group when it has at most 12 albums or a derived 12-album window containing the focus, then optional track-detail rows. `selected_block_bounds` should have one producer in music-group view. Album order within a group is unchanged: `order` sorts by artist key only (`natural_sort_key(strip_article(artist))`, `album_plan.rs:97-98`, a stable sort), so albums keep their existing relative order inside a group — do not add a secondary sort (e.g. by year).
- [x] 2.2 In `src/app/render/album.rs`, render a fixed two-column marker gutter; apply the AQUA `▌` marker and bold blue artist title or bold white album title only to the current target; keep unfocused album text alignment unchanged. Hint text is header-actions vs. album-actions depending on target.
- [x] 2.3 Switch inline artwork between the artist collage (header target) and the focused album's cover (album/track target). Keep the existing 12-row art band and continuation-space filler — preserve the invariant `block_bottom >= art_top + INLINE_ALBUM_ART_ROWS` (`INLINE_ALBUM_ART_ROWS = 12`, `album_art.rs:9`). Use one constant narrowed width for the block (do not add row-aware per-row width measurement — deferred, see design.md).
- [x] 2.4 In `src/app/render/album.rs`, keep the outer offset block-stable using the same lower/upper bounds as other selected-block renderers. Do not scroll the outer viewport into album continuation rows; large groups shift the derived 12-album inline window instead.
- [x] 2.5 Add tests: header-focus and album-focus produce identical `selected_block_bounds` for the same group (pick a terminal width, e.g. 120 columns, wide enough that both the header hint `^P: Play | ^A: Enqueue | ^S: Shuffle` and the album hint `^P: Play | ^A: Enqueue | ^S: Shuffle | ENTER: Show tracks` fit on one line — `selected_hint_lines` wraps against `full_width - artwork_width - 1`, `album_plan.rs:145`, so a narrower width can make the two hints wrap differently and the bounds legitimately differ); track focus keeps the album (not artist) marked and its cover shown, not a collage.
- [x] 2.6 Keep `input_mouse.rs`, `layout.rs`, and action semantics unchanged. Adapt grouped cursor/page planning as needed so albums hidden outside the 12-album window remain reachable and header actions still operate on the full artist group.
- [x] 2.7 Run the app and look at the rendered block for a multi-album artist before considering this pass done. If no interactive terminal is available, render via `ratatui::backend::TestBackend` at 100x40 and dump the buffer with `buffer_to_string` instead — this is a rendering change, and a visual check (interactive or buffer-dumped) is cheaper and higher-signal than exhaustive assertions.
- [x] 2.8 Verify: `cargo test -p mbv --bin mbv`, `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `openspec validate artist-discography-selection-block --strict --no-interactive`.

Non-selected artist groups are unaffected by this change — they keep their existing `ArtistHeader`/`Album` rows unmodified.

## 3. Deferred — row-aware artwork only

Suggested implementer tier if ever scoped: 5.6 luna for the re-planning/design decision (whether it's still needed, how to scope it minimally), then 5.4 to implement.

Do not implement in this change. See proposal.md "Deferred" and design.md's "Deferred: row-aware artwork wrapping" note.

- Row-aware top-down artwork wrapping for text below the 12-row art band.
