> **Audit reset (2026-08-22):** The original implementation passed its tests but
> failed end-to-end conformance audits for Music, Movies/TV, ABS Books, pills,
> and constituent-selection modals. Reopened checkboxes below are not regressions
> in task tracking; their previous completion evidence was insufficient or
> encoded obsolete behavior. Groups 8-16 are the approved remediation plan.
>
> **Checkpoint reconciliation (after Group 12):** Groups 8-12 are reviewed and
> complete. Their production work also fulfills legacy tasks 3.2, 5.2, 7.5,
> and 7.6. Legacy modal/adaptor tasks 1.2, 1.3, 2.3, 3.3, 4.4, and 5.3 are
> superseded by canonical Groups 13-15; legacy audit task 7.1 is superseded by
> Group 16. These legacy mappings are notes, not additional completion owners.

## 1. Selection modal component

- [x] 1.1 Create `src/app/render/components/selection_modal.rs` reusing `modal_frame.rs` for frame/backdrop/centering. The modal renders a scrollable list of items (name + metadata) with existing movement-key navigation. Verify the module compiles and `cargo check -p mbv` passes.
- Legacy 1.2-1.3 ownership is superseded by tasks 13.1-15.2, which define typed live modal state and its complete interaction contract.
- [x] 1.4 Verify the modal renders with the shared modal-frame presentation (border, backdrop dimming, centered placement) via a buffer test comparing against the confirm/context-menu modal frame appearance.

## 2. Series — remove extension, route to modal

- [x] 2.1 Write or update a characterization buffer test (`tests_non_music.rs` or `tv_wide_tests.rs`) capturing the current inline series hero output including seasons/episodes extension. Verify the test passes against current code.
- [x] 2.2 Remove the seasons/episodes extension from `render_series_inline_detail` (`detail_series_view.rs`): the function calls `paint_hero_content` for title + meta + overview + image only, then returns. The `Series:` label, season pill bar, and episode table are deleted from the inline path. Update the characterization test to reflect the intended buffer change (hero content only, no extension). Verify `cargo nextest run -p mbv` passes with the updated test.
- Legacy 2.3 ownership is superseded by tasks 14.1 and 15.1-15.2.

## 3. Music — remove workspace, route to modal

- [x] 3.1 Write or update a characterization buffer test (`tests_music_characterization.rs`) capturing the current inline album hero output including the track list workspace. Verify the test passes against current code.
- [x] 3.2 Replace `album_detail.rs`'s inline path with a `paint_hero_content` call (Model A): album title, metadata, album art. The track list and action hints are removed from the inline hero. Fulfilled and reviewed by tasks 10.1-10.2.
- Legacy 3.3 ownership is superseded by tasks 14.1 and 15.1-15.2.

## 4. Podcasts — remove hand-painted block + in-hero pills, add panel pills, route to modal

- [x] 4.1 Write or update a characterization buffer test (`tests_audiobookshelf_podcasts.rs`) capturing the current inline podcast hero output including hand-painted author/description and in-hero filter pills. Verify the test passes against current code.
- [x] 4.2 Remove the hand-painted author/description block from `render_audiobookshelf_hero` (`audiobookshelf.rs`): author and description become `HeroLine::Plain` entries in `HeroContent::lines`. Remove the in-hero filter pill bar. Update the characterization test to reflect the intended buffer change (standard Model A hero). Verify `cargo nextest run -p mbv` passes with the updated test.
- [x] 4.3 Add alphabetical browsing pills to the podcast tab's panel area using `render_pill_bar` (matching `render_letter_pills_row` in `screens/pills.rs`). Verify via buffer test that alphabetical pills render in the panel, one row, with `⌘` prefix, and no pills render inside the hero.
- Legacy 4.4 ownership is superseded by tasks 14.2 and 15.1-15.2.

## 5. ABS Books — switch from Model B to Model A

- [x] 5.1 Write or update a characterization buffer test (`tests_audiobookshelf_books.rs`) capturing the current inline book hero output (Model B, beside-image). Verify the test passes against current code.
- [x] 5.2 Switch `render_audiobookshelf_book_hero` (`audiobookshelf_books.rs`) from Model B to Model A with one geometry plan for the tall cover. Fulfilled and reviewed by tasks 12.1-12.2.
- Legacy 5.3 ownership is superseded by tasks 14.2 and 15.1-15.2.

## 6. Home Feed — preserve shape, migrate painter routing

- [x] 6.1 Write or update a characterization buffer test (`tests_home_inline.rs`) capturing a literal Home Feed item in its already-correct text-only/no-image shape: title and metadata render, with no artwork reservation or image-cache path. Verify the test passes against current code.
- [x] 6.2 Route non-Audiobookshelf Home items (currently literal Feed entries) through shared `paint_hero_content` Model A, preserving the existing text-only/no-image output. Do not remove or alter `render_home_latest_detail`'s Audiobookshelf-only image branch. Verify the corrected characterization and focused Home inline tests; the Group 7a code implements and verifies this production routing migration.

## 7. Cleanup and verification

- Legacy 7.1 final audit ownership is superseded by task 16.2.
- [x] 7.2 Run `rtk cargo clippy --workspace --all-targets` and resolve any new warnings introduced by the changes.
- [x] 7.3 Run `rtk cargo nextest run -p mbv` and verify the full test suite passes.
- [x] 7.4 Run `rtk make check-code-file-lines` and verify no source file exceeds the 800-line cap as a result of the changes.
- [x] 7.5 Route grouped Music through the shared inline replacement flow with image-enabled fit/fallback, persisted scroll, and marker coverage. Fulfilled and reviewed by tasks 10.1-10.2.
- [x] 7.6 Enforce one painted pill row, one parent-background spacer, one-row hitboxes, and one render owner on every surface. Fulfilled and reviewed by tasks 8.1-8.2.

## 8. Shared pill contract

- [x] 8.1 **First half:** Add end-to-end buffer and hit-target characterization for Movies, TV, Music, Podcasts, ABS Books, Home, and Feeds using each screen's already-defined labels, IDs, and selection. Prove current duplicate bars, background spill, sizing differences, and stale hitboxes before changing production code.
- [x] 8.2 **Second half:** Make one shared arrangement own exactly one pill row plus one parent-background spacer; keep all styling and one-row hitboxes in `render_pill_bar`; delete surface-owned pill geometry and ABS Books' duplicate renderer invocation. Update the characterizations to the approved uniform output.

## 9. Shared inline replacement contract

- [x] 9.1 **First half:** Extract the existing plain/letter replacement behavior into one shared plan covering fit admission, source-row swallowing, continuation rows, `inline_detail_flow`, persisted scroll, ordinary-row fallback, one parent target, and marker suppression. Add contract tests before migrating another surface.
- [x] 9.2 **Second half:** Route the existing plain and letter-grouped Movies/TV paths through that plan without visual change. Preserve a preceding letter header only when the complete hero still fits; otherwise preserve the shared flow offset.

## 10. Grouped Music remediation

- [x] 10.1 **First half:** Remove Music's parallel `AlbumHeroStart`/continuation replacement and custom hero offset/target/marker behavior. Keep artist grouping and album-row generation, but apply the shared replacement plan to the selected album source row.
- [x] 10.2 **Second half:** Add image-enabled end-to-end tests for bottom growth, cannot-fit ordinary-row fallback, persisted scrolling, complete source-row swallowing, one parent target, no continuation targets, and no ordinary selection marker over the hero.

## 11. Movies and TV remediation

- [x] 11.1 **First half:** Fix letter-header preservation so it cannot displace a selected Movie/Series hero below the viewport. Cover plain, letter-grouped, bottom-selected, and Mini presentations.
- [x] 11.2 **Second half:** Route TV double-click through the same narrow/wide activation gate as Enter: narrow opens the Series modal; wide retains the persistent season/episode workspace. Add keyboard/mouse parity and pill-contract tests for narrow and wide Movies/TV.

## 12. ABS Books remediation

- [x] 12.1 **First half:** Adopt shared narrow/wide arrangements, remove duplicate pill ownership, and derive hero reservation and image painting from one geometry plan so the 12-row cover cannot exceed `hero_area`.
- [x] 12.2 **Second half:** Remove chapter rows, chapter reservation, child targets, and chapter focus from narrow inline mode. Keep them wide-only; make narrow Enter and double-click open the chapter modal through one action and restore the ordinary row when the hero cannot fit.

## 13. Selection modal foundation

- [x] 13.1 **First half:** Replace copied modal rows with a typed source identity and typed `Loading`, `Empty`, or `Ready` list state. Separate status from non-selectable content headers and preserve stable item identity.
- [x] 13.2 **Second half:** Refresh the matching open modal on provider completion events, preserve cursor by stable item ID, standardize bounded modal width/height and row formatting, and render every screen's already-defined pill model through the shared pill contract without surface styling or geometry overrides.

## 14. Selection modal adapters

- [x] 14.1 **First half:** Rebuild Series and Music adapters. Series loads all required season data and uses its defined season pills; Music cold-opens as Loading and replaces it with sorted tracks when `AlbumTracksFetched` arrives. Verify empty states and activation.
- [x] 14.2 **Second half:** Rebuild Podcast and ABS Books adapters. Keep Podcast's defined filter pills modal-scoped and refresh pending detail; make Books fetch/refresh pending detail, format chapters consistently, and map fallback audio-file item IDs to the correct visible row.

## 15. Interaction and cancellation conformance

- [x] 15.1 **First half:** Verify keyboard movement, pill movement, Enter activation, Esc/Backspace cancellation, and parent cursor/scroll preservation for Series, Music, Podcasts, and Books from cold, ready, and empty states.
- [x] 15.2 **Second half:** Verify mouse hit targets and activation parity: one parent target for every narrow hero, no narrow child targets, modal pills/list targets owned by the modal, and wide-only child targets retained where specified.

## 16. Final conformance gate

- [x] 16.1 **First half:** Run the end-to-end matrix for every reported screen at narrow, bottom-selected, cannot-fit, Mini, and wide presentations. Confirm one pill bar, correct spacer, complete hero or ordinary-row fallback, swallowed source row, and correct modal lifecycle.
- [x] 16.2 **Second half:** Run frontend boundary scan, `rtk cargo clippy --workspace --all-targets`, `rtk cargo nextest run -p mbv`, `rtk make check-code-file-lines`, formatting/diff checks, and OpenSpec validation. Resolve only change-introduced findings and obtain independent read-only review before archive.
