## 1. Selection modal component

- [ ] 1.1 Create `src/app/render/components/selection_modal.rs` reusing `modal_frame.rs` for frame/backdrop/centering. The modal renders a scrollable list of items (name + metadata) with existing movement-key navigation. Verify the module compiles and `cargo check -p mbv` passes.
- [ ] 1.2 Add app state for the selection modal: which surface opened it, the constituent items, cursor, and filter state (for podcasts). Verify `cargo check -p mbv` passes.
- [ ] 1.3 Wire Enter handling: when a surface with constituent items is selected in the inline-hero presentation, Enter opens the modal; Enter on a modal item selects/activates it; Esc/Backspace closes the modal and returns focus to the library browser. Verify with a buffer test that Enter opens the modal and Esc closes it, returning to the same scroll position.
- [ ] 1.4 Verify the modal renders with the shared modal-frame presentation (border, backdrop dimming, centered placement) via a buffer test comparing against the confirm/context-menu modal frame appearance.

## 2. Series — remove extension, route to modal

- [ ] 2.1 Write or update a characterization buffer test (`tests_non_music.rs` or `tv_wide_tests.rs`) capturing the current inline series hero output including seasons/episodes extension. Verify the test passes against current code.
- [ ] 2.2 Remove the seasons/episodes extension from `render_series_inline_detail` (`detail_series_view.rs`): the function calls `paint_hero_content` for title + meta + overview + image only, then returns. The `Series:` label, season pill bar, and episode table are deleted from the inline path. Update the characterization test to reflect the intended buffer change (hero content only, no extension). Verify `cargo nextest run -p mbv` passes with the updated test.
- [ ] 2.3 Route Enter on a selected Series in the inline presentation to the selection modal, listing seasons and episodes. Verify via buffer test that Enter opens the modal with season/episode items.

## 3. Music — remove workspace, route to modal

- [ ] 3.1 Write or update a characterization buffer test (`tests_music_characterization.rs`) capturing the current inline album hero output including the track list workspace. Verify the test passes against current code.
- [ ] 3.2 Replace `album_detail.rs`'s inline path with a `paint_hero_content` call (Model A): album title, metadata, album art. The track list and action hints are removed from the inline hero. Update the characterization test to reflect the intended buffer change. Verify `cargo nextest run -p mbv` passes with the updated test.
- [ ] 3.3 Route Enter on a selected album in the inline presentation to the selection modal, listing tracks. Verify via buffer test that Enter opens the modal with track items.

## 4. Podcasts — remove hand-painted block + in-hero pills, add panel pills, route to modal

- [ ] 4.1 Write or update a characterization buffer test (`tests_audiobookshelf_podcasts.rs`) capturing the current inline podcast hero output including hand-painted author/description and in-hero filter pills. Verify the test passes against current code.
- [ ] 4.2 Remove the hand-painted author/description block from `render_audiobookshelf_hero` (`audiobookshelf.rs`): author and description become `HeroLine::Plain` entries in `HeroContent::lines`. Remove the in-hero filter pill bar. Update the characterization test to reflect the intended buffer change (standard Model A hero). Verify `cargo nextest run -p mbv` passes with the updated test.
- [ ] 4.3 Add alphabetical browsing pills to the podcast tab's panel area using `render_pill_bar` (matching `render_letter_pills_row` in `screens/pills.rs`). Verify via buffer test that alphabetical pills render in the panel, one row, with `⌘` prefix, and no pills render inside the hero.
- [ ] 4.4 Route Enter on a selected podcast show to the selection modal. The modal shows the played/unplayed filter at its top and the filtered episode list below. Verify via buffer test that Enter opens the modal with filter pills and episode items, and that changing the filter updates the episode list.

## 5. ABS Books — switch from Model B to Model A

- [ ] 5.1 Write or update a characterization buffer test (`tests_audiobookshelf_books.rs`) capturing the current inline book hero output (Model B, beside-image). Verify the test passes against current code.
- [ ] 5.2 Switch `render_audiobookshelf_book_hero` (`audiobookshelf_books.rs`) from `beside_image_hero_dims` / `render_beside_image_hero` (Model B) to `paint_hero_content` (Model A) with the tall book cover. Update the characterization test to reflect the intended buffer change (right-aligned image, wrap-around text). Verify `cargo nextest run -p mbv` passes with the updated test.
- [ ] 5.3 Route Enter on a selected book in the inline presentation to the selection modal, listing chapters. Verify via buffer test that Enter opens the modal with chapter items and Esc closes it.

## 6. Home Feed — align image placement

- [ ] 6.1 Write or update a characterization buffer test (`tests_home_inline.rs`) capturing the current Home Feed inline hero output (Model B text block + image below text). Verify the test passes against current code.
- [ ] 6.2 Align the Home Feed inline hero to Model A no-image (text-only, like the dedicated Feeds tab). Feeds have never shown images; the image-below-text path in `render_home_latest_detail` is removed. Update the characterization test to assert the text-only Model A output. Verify `cargo nextest run -p mbv` passes with the updated test.

## 7. Cleanup and verification

- [ ] 7.1 Verify no inline-hero surface renders a bespoke content path, extension block, or in-hero pill bar: grep for direct `render_widget` calls inside inline-hero painters outside of `paint_hero_content` and `render_beside_image_hero`. Verify only the shared component calls remain.
- [ ] 7.2 Run `rtk cargo clippy --workspace --all-targets` and resolve any new warnings introduced by the changes.
- [ ] 7.3 Run `rtk cargo nextest run -p mbv` and verify the full test suite passes.
- [ ] 7.4 Run `rtk make check-code-file-lines` and verify no source file exceeds the 800-line cap as a result of the changes.
