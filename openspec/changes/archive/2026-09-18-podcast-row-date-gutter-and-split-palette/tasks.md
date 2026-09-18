## 1. Row metadata variant and date gutter

- [x] 1.1 Add the publish-date variant to the closed row-metadata vocabulary and paint it in a fixed six-column right-aligned gutter in the date role, reserving its width in the title's slot; verify with focused buffer tests covering a dated row, a short date right-aligned in the same gutter, and an undated row reserving nothing
- [x] 1.2 Add the gutter's day-and-month formatter beside the hero's four-digit-year one; verify with a unit test for `17 Sep`, the unpadded single-digit day, and the saturation case
- [x] 1.3 Move the split-row palette onto its own roles (soft-white context, light-grey item title) while the playback strips keep their context and title roles; verify with the split-row buffer tests and the Home tick test that paints both roles

## 2. Podcast episode rows

- [x] 2.1 Project the episode's publish date into the row's metadata slot; verify with the episode-row projection test that a dated episode carries the formatted date and an undated one carries none
- [x] 2.2 Confirm the gutter at both presentations (Wide and non-Wide paint through the one row seam) and that the hero, Queue, TV, music, book, and feed rows are unchanged

## 3. Close-out

- [x] 3.1 Run `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo nextest run -p mbv -p mbv-core -p mbvd`, then archive the change so its deltas land in `openspec/specs/`
