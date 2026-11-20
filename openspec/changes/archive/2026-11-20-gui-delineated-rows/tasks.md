# Tasks

## 1. Queue row

- [x] 1.1 Set the queue row geometry to design D3 in `queue_panel/geometry.rs` (`ROW_HEIGHT_PX` 40, `ROW_GAP_PX` 0, ordinal column 28, right column 96, padding 10) and the row's 1 px divider and square corners (now-playing radius 4) in `queue_panel.rs`; verify `cargo nextest run -p mbv-gui queue_panel` passes with the capacity and ghost-offset tests updated to the new stride.
- [x] 1.2 Replace the in-row progress bar in `queue_panel/row_cells.rs` with the progress ring after the title (design D3: SVG, `stroke-dasharray`, track `Bg2`, fill `Orange`, now playing `Green`), driven by the unchanged `row_progress`; verify a render test asserts the ring on a partial row and on the now-playing row and no ring on a new or played row.
- [x] 1.3 Paint the frame's `duration` in the right column for every row state, and delete the start mark (`StartMark`, `start_mark`, its `Play` glyph) from the row; remove `GuiQueue::start`, `GuiQueue::playing`, `GuiQueue::start_slot_reported`, `QueueStartProjection`, `gui_queue_start_projection`, and the tests that cite the retired start-mark scenarios (design D3); verify a render test asserts the duration on new, partial, played, and now-playing rows and no start mark on a stopped queue's start slot.
- [x] 1.4 Update the `queue_panel.rs` module docs and `docs/architecture/gui-client.md` queue-row text to the new row; verify no doc still names the in-row progress bar or the start mark.

## 2. Row art model and Service requests

- [x] 2.1 Add `ImageRendition { Hero, Row }` to `ImageFetchReq` (all current callers pass `Hero`); verify `cargo check --workspace` passes and existing image-fetch tests pass unchanged.
- [x] 2.2 Build the Emby `Row` URL (design D6: first declared tagged type with `tag=`, untagged chain fallback, `maxWidth=128` or `maxWidth=72&maxHeight=72` by declared arm, `quality=80`) and its `row:` cache key; verify unit tests cover the tagged URL, the untagged fallback, and the square size.
- [x] 2.3 Parse the Audiobookshelf library item `updatedAt` into the catalog entry for books and podcasts in `crates/mbv-audiobookshelf/src/catalog.rs`; verify a catalog test reads it from a fixture and defaults when it is absent.
- [x] 2.4 Build the Audiobookshelf `Row` cover URL (`width=128` book, `width=72` podcast, `format=jpeg`) and its `row:abs:` key holding `updated_at`; verify unit tests cover both widths and the key change on a new `updated_at`.
- [x] 2.5 Add the row LRU (256 entries, keys starting `row:`) and the row pending queue (replaced on each set, drained after the Hero/card queue, at most 4 of `MAX_IMAGE_FETCHES` slots) to `crates/mbv-images/src/cache.rs`; verify tests: a row insert never evicts a non-row key, a replaced set drops an unstarted row request and clears its loading mark (design D7), and a waiting Hero request is taken before a waiting row request.
- [x] 2.6 Add `docs/invariants/` entry for the row cache key rule (a row key holds the Service's change value — Emby tag, Audiobookshelf `updatedAt` — so changed art never reuses old bytes); verify the file exists with the next free invariant number.

## 3. Row art in the core

- [x] 3.1 Add `GuiRowArt` and `GuiRowMediaKind` (design D4) to the app-side Library projection types and their bridge frames; verify `cargo check -p mbv -p mbv-gui` passes.
- [x] 3.2 Add the row art resolver in `src/app/gui_library/artwork.rs`: source from the artwork policy per design D5, `NoSource` with no request, cache read for `Image`/`Loading`/`Missing` with `square` from `painted_shape_for_pixels`; verify unit tests for a Feeds row (`NoSource`, no fetch), a cached square cover (`square: true`), and an empty fetch (`Missing`).
- [x] 3.3 Add `GuiRowArtScope` and `GuiLibraryCommand::RowsInView { scope, ids }` in the bridge and the app (design D7), with an exhaustive dispatch arm that stores the latest set per scope, stamps the navigation-idle clock, and queues `Row` fetches for the in-view rows once idle; verify a test that a set sent during navigation fetches nothing until idle, then fetches only those rows.
- [x] 3.3a Queue an interrupted in-view row key again when its retry time passes, through the art retry deadline (design D7, invariant 27); verify a test with an injected interrupted fetch and a passed retry time (no sleep) that the key is queued again only while it is in the stored set.
- [x] 3.4 Fill `art` and `media` on the Home rows through the resolver (Audiobookshelf rows: their destination's ABS cover, design D5) and delete `gui_home_row_artwork` and its at-once fetch; verify the Home projection tests assert row art states and that no Home row fetches before `RowsInView`.
- [x] 3.5 Fill `art` and `media` on the Emby list rows and the Shows series, New, and Upcoming rows (Upcoming: the series image, design D5); verify projection tests assert one Emby row's source and media kind and one Upcoming row's series source.
- [x] 3.6 Fill `art` and `media` on the Music artist rows (`ArtistKey::Service` → artist `Primary`, `Fallback` → `NoSource`); verify a projection test for both keys.
- [x] 3.7 Fill `art` and `media` on the Audiobooks rows and on `GuiEntryRowFrame` (Podcasts: show cover; Feeds: `NoSource`, media from `FeedKind`); verify projection tests for one book, one podcast episode, and one audio and one video feed entry.
- [x] 3.8 Make the Home Continue rows' trailing text the time left (design D9) in `src/app/gui_library/home.rs`; verify a table test for under one hour, exactly one hour, and a rounded-up minute.

## 4. Library top-level row in the window

- [x] 4.1 Add the thumb to `LibraryItemRow` (design D1, D2: box, cover or centred-square image, kind icon for `Missing`/`NoSource`, thumb progress track and fill, the new `--canvas-thumb-track` token in `assets/app.css`), set the top-level row look (56 px, divider, title weight 400), and delete the under-row run; verify render tests for each art state, the square fit, the progress bar, and a title-only row.
- [x] 4.2 Send `RowsInView` from `library_virtual_list` with the painted ids plus 4 rows of overscan each time the set changes; verify a window test that a scroll sends the new set and an unchanged set sends nothing.
- [x] 4.3 Paint Home item rows through `LibraryItemRow`, delete `paint_home_item_row`'s own row look, and set the Home section heading look (design D2) and the Home row height constant; verify the Home list tests pass with the shared part's hooks.
- [x] 4.4 Paint the Emby, Podcasts, Audiobooks, and Feeds list rows with their frame's `art` and `media`, and update their row-height constants; verify each list's existing render test asserts the thumb hook.
- [x] 4.5 Paint the Music artist rows and the Shows series rows through `LibraryItemRow` (nested album, track, season, and episode rows unchanged) and update the top-level row heights in their height tables; verify render tests that the top-level row has a thumb and a nested row has none, and the letter-rail jump test passes with the new heights.
- [x] 4.6 Paint the Shows New and Upcoming rows (`paint_shows_flat`) through `LibraryItemRow` with their art; verify the Shows New and Upcoming render tests assert the thumb hook.
- [x] 4.7 Update `docs/architecture/gui-client.md` (Library row part, row art flow, `RowsInView`) and the component module docs touched in this group; verify no doc still names the under-row progress run.

## 5. Integration

- [x] 5.1 Run `cargo fmt`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo nextest run --workspace`; verify all pass.
- [x] 5.2 Run `openspec validate gui-delineated-rows --strict`; verify it passes.
