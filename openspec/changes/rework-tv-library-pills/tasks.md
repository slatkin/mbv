# Tasks

## 1. Mode value and the TV bucket set

- [x] 1.1 Add the TV-specific three-bucket table beside the existing movie table in `src/app/render/screens/sort_filter.rs` (`A-I` = `NameLessThan("J")`, `J-R` = `[J, S)`, `S-Z` = `NameStartsWithOrGreater("S")` with no upper bound) and make `LetterFilter` construction select the table by library kind while movie construction keeps using the existing table unchanged. Verify: `cargo nextest run -p mbv` covers the three TV labels/bounds, a non-letter name resolving to `A-I`, an `S`-or-later name resolving to `S-Z`, and the movie labels still returning the nine-bucket set.
- [ ] 1.2 Introduce a closed `TvContentMode` value (`Latest`, `Upcoming`, `All`, range) on the top-level TV `BrowseLevel` and remove the overloaded use of `letter_filter: Option<LetterFilter>` as both "no filter" and "the auto-applied default". Verify: `cargo check -p mbv` is clean and focused tests cover each mode's content selector and the unresolved/absent default.

## 2. Row composition, threshold, and defaults

- [ ] 2.1 Compose the TV content-mode row from the mode list in design D3: above `LIBRARY_PILL_THRESHOLD` the row is `Latest | Upcoming | A-I | J-R | S-Z` with `Latest` selected; at or below it the row is `Latest | Upcoming | All` with `All` selected. Verify: `cargo nextest run -p mbv` covers both rows, their order, the selected mode, and that `All` is absent above the threshold.
- [ ] 2.2 In `src/app/lib_event_actions.rs::maybe_capture_library_total_and_apply_default_pill`, keep the `library_total` capture unchanged and scope the change to TV: a TV library replaces the `A-C` auto-apply with default-mode selection (above the threshold auto-select `Latest` after the capture load and fetch its episodes; at or below it select `All` over the already-loaded unfiltered list with no re-fetch), and the small-library quirk where the TV row painted `A-C` highlighted over an unfiltered list is removed. Movie, feed, and podcast libraries keep today's auto-scope `A-C` behavior and row byte-for-byte. Verify: `cargo nextest run -p mbv` covers that a small TV library's first load is unfiltered with `All` selected and no second fetch, that a large TV library's first load selects `Latest` after the capture load with one episode fetch and no letter-scoped fetch, and that a large movie library still auto-applies `A-C` with its scoped refresh.

## 3. Latest mode

- [ ] 3.1 Wire the `Latest` mode to the same feed Home's TV section uses (`get_latest_episodes(view_id, 30)`) through the library's own fetch path, so it loads whether or not Home has. Verify: `cargo nextest run -p mbv` covers `Latest` producing the feed's episodes with no dependency on a loaded Home section.
- [ ] 3.2 Pass the `Latest` mode into the TV component's selector row with flat episode rows and no series detail fetch. Verify: `cargo nextest run -p mbv` covers the selector's active mode and that selecting `Latest` does not request a series detail.

## 4. Upcoming mode

- [ ] 4.1 Add `get_upcoming(parent_id, limit)` to `crates/mbv-core/src/api_client_library.rs` over `GET /Shows/Upcoming` with `ParentId`, and cover it with a mock/parse unit test. Verify: `cargo nextest run -p mbv-core` is green.
- [ ] 4.2 Wire the `Upcoming` mode to that fetch through the library's fetch path with flat episode rows. Verify: `cargo nextest run -p mbv` covers `Upcoming` requesting the route with the library as parent and presenting the returned episodes.

## 5. Episode activation and hero

- [ ] 5.1 Make `Latest` and `Upcoming` episode rows activate play, and ensure the TV component never treats an episode target as a series selection (no workspace, no season selector, no detail request). Verify: a real-`Application::tick()` integration test asserts activation plays the episode and that no series workspace opens.
- [ ] 5.2 Show the selected episode's hero only in the mini-view presentation, and none in the other geometries. Verify: `cargo nextest run -p mbv` covers mini view painting the episode hero and a non-mini geometry painting none.

## 6. Persistence, cycling, and mouse

- [ ] 6.1 Persist the selected `TvContentMode` in `LibraryPositionLevel` and restore it (and its content load) when the library position is reopened. Verify: `cargo nextest run -p mbv` covers save/restore for each mode.
- [ ] 6.2 Make `[`/`]` cycle over the modes actually present in the painted row with wrap, and make mouse selection select the clicked mode. Verify: `cargo nextest run -p mbv` covers wrap on the three-mode small-library row, wrap on the five-mode large-library row, and mouse selection of each mode.

## 7. Shared new-content marker

- [ ] 7.1 Hoist the acknowledged `HomeLatestSource` set out of the Home component into shell-owned state and have the Home selector read it; keep the marker value the shell-computed `has_new_content`. Verify: `cargo nextest run -p mbv` covers Home's marker rendering through the shell-owned set and that a fetch after launch does not add a marker.
- [ ] 7.2 Render that same marker on the TV library's `Latest` mode for the matching library view, and share acknowledgement so selecting either surface clears both. Verify: a real-`Application::tick()` integration test selects the library `Latest` mode and asserts both markers clear, and the reverse via Home's pill.

## 8. Verification and close-out

- [ ] 8.1 Run the full gate: `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all -- --check`, `cargo nextest run -p mbv`, and `cargo nextest run --workspace`. Verify: all commands are clean.
- [ ] 8.2 Manually confirm against a real Emby server that `/Shows/Upcoming` scoped to a TV library returns playable rows (or record the filtering needed if it does not), and record the result. Verify: a note exists naming the observed behavior and any follow-up.
- [ ] 8.3 Sync the applied deltas into `openspec/specs/`: apply the modified `tv-letter-filtering` and `home-latest-sections` requirements and add the `tv-library-content-modes` capability. Verify: `openspec validate --all` is clean.
- [ ] 8.4 Record any new domain vocabulary (the content-mode value and the TV range set) in `CONTEXT.md` without renaming existing terms. Verify: the terms appear with definitions.
- [ ] 8.5 Commit the implementation and the synced main specs. Verify: `git status --short` is empty after the commit.
- [ ] 8.6 Archive the change with `openspec archive rework-tv-library-pills`. Verify: `openspec list --json` no longer reports the change as active and `openspec validate --all` stays clean.
