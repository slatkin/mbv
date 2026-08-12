## 1. Browse State

- [x] 1.1 Add a concrete `All`/`Played`/`Unplayed` episode-filter state to each Audiobookshelf podcast library and keep the selected show identity independent from the episode cursor.
- [x] 1.2 Derive the visible episode list from downloaded episodes and read-only progress, treating missing or incomplete progress as unplayed.
- [x] 1.3 Remove shelf entries from Audiobookshelf visible row generation, cursor movement, and selection targets while preserving show pagination and detail loading.

## 2. TV-Style Presentation

- [x] 2.1 Replace the flat Audiobookshelf paragraph renderer with show-only library rows and a selected-show detail block using the established TV layout conventions.
- [x] 2.2 Render selected podcast metadata, cover/placeholder behavior, and `All`/`Played`/`Unplayed` pills with shared visual primitives.
- [x] 2.3 Render filtered downloaded episodes as a structured, selectable table with title, publication information, duration, and read-only progress/completion styling.
- [x] 2.4 Add scoped loading and empty states for detail loading and filters with no matching episodes, without hiding the show list.

## 3. Input And Activation

- [x] 3.1 Mirror TV show-selection input: show navigation selects shows, activation enters episode selection, and escape returns to show selection.
- [x] 3.2 Add episode-filter cycling and episode cursor clamping/reset when the filter or selected show changes.
- [x] 3.3 Keep episode activation inert and verify it cannot enqueue, play, open a session, or write progress.
- [x] 3.4 Preserve keyboard and mouse show selection while preventing shelf rows from receiving navigation or activation events.

## 4. Verification

- [x] 4.1 Add or update focused state-transition checks for filter matching, stable show identity, episode identity, shelf omission, and cursor restoration.
- [x] 4.2 Verify focused and unfocused rendering against the TV-style hierarchy at narrow and wide terminal widths, with images enabled and disabled.
- [x] 4.3 Run the relevant Audiobookshelf tests, `cargo check -p mbv`, formatting, diff checks, and manual peer-tab/filter/episode navigation verification.
