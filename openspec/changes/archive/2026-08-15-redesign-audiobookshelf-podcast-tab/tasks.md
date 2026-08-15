## 1. Remove The Nonconforming Layout

- [x] 1.1 Delete the Audiobookshelf renderer's horizontal show-list/detail split; at every width, reserve one full-width top hero and put the show list only below it.
- [x] 1.2 Route Audiobookshelf geometry through the same top-hero/list-area planning rules used by TV Shows, including hero suppression, separator rows, one/two-column breakpoint, content padding, and stable loading placeholder height.
- [x] 1.3 Populate the lower list exclusively from provider-native podcast shows; keep shelf entries and episodes out of its row map, scrolling, pagination, cursor movement, and mouse hit targets.
- [x] 1.4 Render podcast show cells with the same one-column and two-column content, selected-cell marker, focus colors, truncation, and scrolling behavior as TV Series cells while retaining Audiobookshelf show identities.

## 2. Reproduce The TV Hero With Podcast Data

- [x] 2.1 Build the selected-podcast hero in the same full-width shell and content rectangle used by TV Series, matching its border, background, padding, title-row rule, image/text wrapping, and content-derived row budget.
- [x] 2.2 Fetch the selected podcast's cover through the existing Audiobookshelf cover request and cache-key path, keyed by Service and provider-native library item identity; do not fetch list thumbnails as a substitute for hero artwork.
- [x] 2.3 Render the fetched cover in the TV Series Primary-image slot with the same right alignment, dimensions, scaling filter, loading placeholder, missing-image behavior, and images-disabled behavior.
- [x] 2.4 Map podcast title and available author metadata into the TV hero text area without introducing a left hero column, right detail panel, or alternate wide Music layout.
- [x] 2.5 Keep the pinned hero content synchronized with the selected podcast even when that podcast's row scrolls outside the visible lower list.

## 3. Map Season Selection To Episode Filters

- [x] 3.1 Keep exactly one `All`/`Played`/`Unplayed` filter state for the selected podcast and derive matching downloaded episodes from read-only Audiobookshelf progress, treating missing or incomplete progress as Unplayed.
- [x] 3.2 In show-selection mode, render the filter summary where TV Shows renders its season summary and keep episode rows hidden exactly when TV episode rows are hidden.
- [x] 3.3 On podcast-show activation, enter episode-selection mode and render the three filter pills in the TV season-selector row using the shared pill appearance, prefix spacing, overflow, and focus treatment.
- [x] 3.4 Use the same left/right controls as TV season navigation to switch filters, then reset or clamp the episode cursor using the TV season-change rule without changing the selected podcast.
- [x] 3.5 Use the same up/down, Enter/Space, Escape/Backspace, single-click, and double-click mode transitions as TV show and episode selection, while consuming podcast-episode activation without playback, queue mutation, playback-run or Session creation, Emby action fall-through, or progress writes.

## 4. Match The TV Episode Table

- [x] 4.1 Render filtered downloaded episodes inside the top hero's TV episode-table rectangle, not beside the show list and not interleaved with show rows.
- [x] 4.2 Match TV episode rows for row height, selection-marker position, focused/unfocused colors, title and duration column geometry, truncation, column spacing, and available row budget.
- [x] 4.3 Place podcast publication and read-only progress/completion information within corresponding TV row text or style slots; do not add structural columns or change the table width from the TV layout.
- [x] 4.4 Preserve both provider-native library item and episode IDs for cursor restoration, filter changes, asynchronous detail results, and inert activation.
- [x] 4.5 Render scoped loading and no-matching-episodes states inside the episode-table area without moving or collapsing the top hero or hiding the lower show list.

## 5. Remove Shelf Behavior

- [x] 5.1 Remove personalized shelves from visible Audiobookshelf state derivation and ignore shelf-fetch results for podcast-tab rendering.
- [x] 5.2 Remove shelf headings, shelf entries, and shelf-derived shows or episodes from keyboard navigation, mouse hit testing, selection restoration, and hero selection.
- [x] 5.3 Verify that identical show-page and selection state produces identical visible rows and hero content whether shelf data is absent, loading, successful, or failed.

## 6. Add Objective Regression Gates

- [x] 6.1 Add a render-test harness for Audiobookshelf podcasts after the panel layout stabilizes; assert top-hero and show-list geometry at one- and two-column widths.
- [x] 6.2 Add a render regression assertion that no terminal width produces horizontally adjacent show-list and detail rectangles, and compare podcast hero/list geometry with TV Shows.
- [x] 6.3 Add rendered-output assertions for selected podcast title, description, show-row presence, and hero selection tracking after the panel layout stabilizes.
- [x] 6.4 Add image-path regression coverage for the selected Audiobookshelf cover slot and images-enabled/disabled behavior after the panel image design stabilizes.
- [x] 6.5 Add state/input regressions for exact filter membership, incomplete progress as Unplayed, filter cursor clamping, provider-native identity restoration, show-to-episode mode entry, escape back to show selection, inert episode activation, and shelf omission.
- [x] 6.6 Manually compare representative narrow and wide podcast tabs with images enabled and disabled against TV Shows when the panel UI is no longer undergoing active churn.

## 7. Final Verification

- [x] 7.1 Run the focused Audiobookshelf state and input tests and record their passing counts; defer panel render tests to tasks 6.1-6.6.
- [x] 7.2 Run `cargo check -p mbv`, `cargo fmt --all -- --check`, and `cargo clippy --workspace --all-targets`.
- [x] 7.3 Run `make check-code-file-lines` and `rtk git diff --check`.
- [x] 7.4 Confirm the final diff contains no Audiobookshelf side-panel layout, no visible shelf path, no episode activation side effect, and no source file over the repository's 800-line cap.
