# Tasks

## 1. Shared destination contract

- [x] 1.1 Add stable Latest selector identities and fallback for saved Home Latest selections in both TUI launch state and legacy `home_section` preferences; verify config, launch-state, and Home restore tests (`cargo nextest run -p mbv-core` and `cargo nextest run -p mbv`).
- [x] 1.2 Move launch-window marker derivation and source-qualified acknowledgement to destination Latest snapshots without Home projection; verify first-launch, timestamp-boundary, preselected-before-load, and refresh tests (`cargo nextest run -p mbv`).
- [x] 1.3 Route existing Emby latest requests by library id into destination-owned snapshots so an unopened Home is irrelevant; verify TV still gets episodes, non-TV gets additions, and one destination refresh does not fetch another (`cargo nextest run -p mbv`).

## 2. Emby destinations

- [x] 2.1 Preserve TV's current Latest/Upcoming mode behavior while removing TV→Home snapshot synchronization; verify TV mode and flat-row tests (`cargo nextest run -p mbv`).
- [x] 2.2 Add Latest alongside Movies/Generic letter pills, with their original default, search, sort, and folder navigation intact; verify list identity, cycling, click and direct play/enqueue in focused owner and mounted-tick tests (`cargo nextest run -p mbv`).
- [x] 2.3 Add Latest alongside Home Videos group pills without altering existing group state on return; verify Home Videos selector and action tests at Wide and Narrow (`cargo nextest run -p mbv`).
- [ ] 2.4 Add flat Latest to Music's selector/list-slot path while preserving its existing tree/group state and activation when returning; verify group↔Latest round-trip and one-painter Wide/Narrow tests (`cargo nextest run -p mbv`).

## 3. Other destinations

- [ ] 3.1 Add Latest to each Audiobookshelf podcast library's state/show selector using its existing per-library shelf cache; verify empty shelf, async completion, provider-target play/enqueue, selector cycling without Emby, and selected-tab exit/relaunch restore versus All on other podcast tabs (`cargo nextest run -p mbv`). Shelf refetch on podcast refresh is separate issue #763.
- [ ] 3.2 Add Latest to Feeds' group/filter selector using only the already loaded combined entries; verify played entries remain visible, selector exit restores the former group/filter, `w` leaves Latest and cycles the restored filter once, no fetch on entry, and `r` refresh updates Latest (`cargo nextest run -p mbv`).
- [ ] 3.3 Project shared title parts, provider dates, marker glyph and selected detail through existing row/Hero painters for non-TV Latest; verify focused buffer tests at Narrow and Wide (`cargo nextest run -p mbv`).

## 4. Retire Home duplicates and settings

- [ ] 4.1 Remove Home Latest population, provider merge events, pills, and actions while preserving Continue Watching and fallback from saved Home Latest; verify Home tests and mounted-tick navigation (`cargo nextest run -p mbv`).
- [ ] 4.2 Remove `hidden_latest` parsing, serialization, config state and Settings control; verify legacy TOML is accepted but ignored and subsequent save omits the key, while `hidden_libraries` still works (`cargo nextest run -p mbv-core` and `cargo nextest run -p mbv`).

## 5. Integration and close-out

- [ ] 5.1 Verify all destination selectors, mouse/keyboard activation, launch restore, markers after async refresh, and absence of Home Latest through mounted `Application::tick()` tests; run `cargo check -p mbv`, `cargo nextest run -p mbv -j 2`, `cargo nextest run -p mbv-core`, `cargo fmt --all -- --check`, and `cargo clippy --workspace --all-targets -- -D warnings`.
- [ ] 5.2 Sync applied delta specs into main specs, including the Home-only Latest promise in `service-independent-startup` and the personalized-shelf prohibition in both `audiobookshelf-podcast-library-ui` and `audiobookshelf-podcast-browsing` (allow the Newest Episodes shelf only in podcast Latest); revise the obsolete `home-latest-sections` Purpose and relevant `CONTEXT.md` vocabulary, and validate the change (`openspec validate migrate-home-latest-to-destinations --strict`).
- [ ] 5.3 Manually inspect Home and every eligible destination (including a Music tree round-trip) at Narrow and Wide, confirm Latest selectors/dates/markers with real loaded content, and record the observations before requesting user visual sign-off; do not archive before that sign-off.
