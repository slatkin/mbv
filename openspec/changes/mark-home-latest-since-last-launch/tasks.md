# Tasks

## 1. Launch cutoff and provider time

- [x] 1.1 Add the versioned per-user Home-new-content launch timestamp record, tolerant loader, and process-unique atomic replacement writer in `mbv-core`; verify with the existing hermetic test-state-directory seam that missing/malformed state returns no baseline and replacement round-trips without constructing live externals.
- [x] 1.2 Capture an injected/current launch instant once during TUI construction, read the previous timestamp, replace it immediately, and retain the immutable launch window separately from exit-only `TuiLaunchState`; verify with a deterministic startup test (no sleep or real state directory) that first launch establishes a baseline and a second launch reads the first instant before advancing it.
- [x] 1.3 Add one provider-neutral Home Latest timestamp projection for Emby `date_added` and Audiobookshelf/Feed `pub_date_secs`, including ISO-8601 parsing and the closed `previous < item <= current` comparison; verify with one table-driven test covering all three Services plus missing, invalid, cutoff-equal, and future timestamps.

## 2. Typed Home sections and acknowledgement

- [x] 2.1 Replace Home's Latest tuples with the narrowest typed section snapshot carrying title, stable `HomeLatestSource`, items, and the launch-relative new-content fact; update Emby, Audiobookshelf, and Feed merge paths and verify the existing canonical provider-order and independent-arrival Home tests still pass with source-keyed replacement.
- [x] 2.2 Add component-owned visited Latest-source state: selecting a section acknowledges it before paint, an already selected section is acknowledged when async content arrives, and later merge/refresh cannot restore its marker; verify these transitions in focused `HomeContent` component tests using stable source identities rather than section indices.

## 3. Pill marker presentation

- [ ] 3.1 Extend the shared Selector-pill content with an optional semantic marker and paint `•` in the Iris role through the existing pill painter, including marker width in overflow and hit geometry; update the narrowest existing pill buffer/geometry tests to prove marked and unmarked pills without adding a Home-only painter.
- [ ] 3.2 Project markers only for unvisited, inactive Home Latest sources with qualifying content; keep Continue and every selected pill unmarked, then verify in a Home/Library-panel buffer test that the Iris `•` appears only on the expected unselected pill in both a fitting and overflowed Selector row.

## 4. Latest-row date gutters

- [ ] 4.1 Project `fmt_publish_date_short` through `MediaListTrailing::Gutter` for valid timestamps in the active Home Latest section while leaving Continue and undated Latest rows without trailing metadata; extend the focused Home row-projection test to cover Emby, Audiobookshelf, Feed, Continue, and invalid-date behavior.
- [ ] 4.2 Verify through the existing canonical media-list painter coverage that Home's projected `17 Sep`/`7 Sep` values right-align in the fixed green gutter in Wide and non-Wide Library-panel presentations; reuse existing gutter assertions rather than adding a second date painter or whole-frame snapshot.

## 5. Integration and verification

- [ ] 5.1 Add one real `Application::tick()` integration test proving an asynchronously delivered marked Home section paints through the mounted Library panel and selecting its pill removes the marker on the next tick without shell-side acknowledgement mirroring.
- [ ] 5.2 Run `cargo fmt`, focused `cargo nextest run -p mbv-core` and `cargo nextest run -p mbv` filters for the changed modules, then full `cargo nextest run -p mbv-core` and `cargo nextest run -p mbv`; fix all failures without adding sleeps, live Services, or real user state.
- [ ] 5.3 Run `cargo clippy --workspace --all-targets -- -D warnings`, `openspec validate mark-home-latest-since-last-launch --strict`, and review the final diff to confirm no primary-tab marker, Continue date, Home-only painter, visited-state shell mirror, exit-only launch-state write, new dependency, or daemon/ctrl protocol change was introduced.
