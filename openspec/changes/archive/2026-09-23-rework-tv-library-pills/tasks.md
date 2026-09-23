# Tasks

## 1. Mode value and the TV bucket set

- [x] 1.1 Add the TV-specific three-bucket table beside the existing movie table in `src/app/render/screens/sort_filter.rs` (`A-I` = `NameLessThan("J")`, `J-R` = `[J, S)`, `S-Z` = `NameStartsWithOrGreater("S")` with no upper bound) and make `LetterFilter` construction select the table by library kind while movie construction keeps using the existing table unchanged. Verify: `cargo nextest run -p mbv` covers the three TV labels/bounds, a non-letter name resolving to `A-I`, an `S`-or-later name resolving to `S-Z`, and the movie labels still returning the nine-bucket set.
- [x] 1.2 Introduce a closed `TvContentMode` value (`Latest`, `Upcoming`, `All`, range) on the top-level TV `BrowseLevel` and remove the overloaded use of `letter_filter: Option<LetterFilter>` as both "no filter" and "the auto-applied default". Verify: `cargo check -p mbv` is clean and focused tests cover each mode's content selector and the unresolved/absent default.

## 2. Row composition, threshold, and defaults

- [x] 2.1 Compose the TV content-mode row from the mode list in design D3: above `LIBRARY_PILL_THRESHOLD` the row is `Latest | Upcoming | A-I | J-R | S-Z` with `Latest` selected; at or below it the row is `Latest | Upcoming | All` with `All` selected. Verify: `cargo nextest run -p mbv` covers both rows, their order, the selected mode, and that `All` is absent above the threshold.
- [x] 2.2 In `src/app/lib_event_actions.rs::maybe_capture_library_total_and_apply_default_pill`, keep the `library_total` capture unchanged and scope the change to TV: a TV library replaces the `A-C` auto-apply with default-mode selection (above the threshold auto-select `Latest` after the capture load and fetch its episodes; at or below it select `All` over the already-loaded unfiltered list with no re-fetch), and the small-library quirk where the TV row painted `A-C` highlighted over an unfiltered list is removed. Movie, feed, and podcast libraries keep today's auto-scope `A-C` behavior and row byte-for-byte. Verify: `cargo nextest run -p mbv` covers that a small TV library's first load is unfiltered with `All` selected and no second fetch, that a large TV library's first load selects `Latest` after the capture load with one episode fetch and no letter-scoped fetch, and that a large movie library still auto-applies `A-C` with its scoped refresh.

## 3. Latest mode

- [x] 3.1 Wire the `Latest` mode to the same feed Home's TV section uses (`get_latest_episodes(view_id, 30)`) through the library's own fetch path, so it loads whether or not Home has. Verify: `cargo nextest run -p mbv` covers `Latest` producing the feed's episodes with no dependency on a loaded Home section.
- [x] 3.2 Pass the `Latest` mode into the TV component's selector row with flat episode rows and no series detail fetch. Verify: `cargo nextest run -p mbv` covers the selector's active mode and that selecting `Latest` does not request a series detail.

## 4. Upcoming mode

- [x] 4.1 Add `get_upcoming(parent_id, limit)` to `crates/mbv-core/src/api_client_library.rs` over `GET /Shows/Upcoming` with `ParentId`, and cover it with a mock/parse unit test. Verify: `cargo nextest run -p mbv-core` is green.
- [x] 4.2 Wire the `Upcoming` mode to that fetch through the library's fetch path with flat episode rows. Verify: `cargo nextest run -p mbv` covers `Upcoming` requesting the route with the library as parent and presenting the returned episodes.

## 5. Episode activation and hero

- [x] 5.1 (REVISED 2026-09-22 — the original text was falsified by the real server; see upcoming-manual-check.md) Activating a `Latest`/`Upcoming` row that carries a playable episode id SHALL play it and SHALL NOT open a series detail or Workspace; activating an `Upcoming` row that carries no episode id but names its series SHALL navigate the library to that series and open its Workspace instead of playing. Keyboard activation and mouse activation SHALL take the same path (series-direct first; never an empty-Id first-match resolver). Verify: real-`Application::tick()` integration tests assert play-for-id, series-workspace-for-idless, and keyboard/mouse agreement row-for-row.
- [x] 5.2 Show the selected episode's hero only in the mini-view presentation, and none in the other geometries. Verify: `cargo nextest run -p mbv` covers mini view painting the episode hero and a non-mini geometry painting none.

## 6. Persistence, cycling, and mouse

- [x] 6.1 Persist the selected `TvContentMode` in `LibraryPositionLevel` and restore it (and its content load) when the library position is reopened. Verify: `cargo nextest run -p mbv` covers save/restore for each mode.
- [x] 6.2 Make `[`/`]` cycle over the modes actually present in the painted row with wrap, and make mouse selection select the clicked mode. Verify: `cargo nextest run -p mbv` covers wrap on the three-mode small-library row, wrap on the five-mode large-library row, and mouse selection of each mode.

## 7. Shared new-content marker

- [x] 7.1 Hoist the acknowledged `HomeLatestSource` set out of the Home component into shell-owned state and have the Home selector read it; keep the marker value the shell-computed `has_new_content`. Verify: `cargo nextest run -p mbv` covers Home's marker rendering through the shell-owned set and that a fetch after launch does not add a marker.
- [x] 7.2 Render that same marker on the TV library's `Latest` mode for the matching library view, and share acknowledgement so selecting either surface clears both. Verify: a real-`Application::tick()` integration test selects the library `Latest` mode and asserts both markers clear, and the reverse via Home's pill.

## 8. Verification and close-out

- [x] 8.1 Run the full gate: `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all -- --check`, `cargo nextest run -p mbv`, and `cargo nextest run --workspace`. Verify: all commands are clean.
- [x] 8.2 Manually confirm against a real Emby server that `/Shows/Upcoming` scoped to a TV library returns playable rows (or record the filtering needed if it does not), and record the result. Verify: a note exists naming the observed behavior and any follow-up. (AMENDED 2026-09-22: a negative finding SHALL reopen planning as follow-up tasks — filing the note does not close the row. The 2026-09-22 Virtual/id-less finding is tracked by §9.)
- [x] 8.3 Sync the applied deltas into `openspec/specs/`: apply the modified `tv-letter-filtering` and `home-latest-sections` requirements and add the `tv-library-content-modes` capability. Verify: `openspec validate --all` is clean.
- [x] 8.4 Record any new domain vocabulary (the content-mode value and the TV range set) in `CONTEXT.md` without renaming existing terms. Verify: the terms appear with definitions.
- [x] 8.5 Commit the implementation and the synced main specs. Verify: `git status --short` is empty after the commit.
- [x] 8.6 Archive the change with `openspec archive rework-tv-library-pills`. Verify: `openspec list --json` no longer reports the change as active and `openspec validate --all` stays clean. (UNARCHIVED 2026-09-22 — the archive was premature; re-archive only after §9 is complete.)

## 9. Real-server repair (spec review of `main...661e374e` against the handoff, 2026-09-22)

- [x] 9.1 Upcoming date-grouped headings: `Upcoming` rows SHALL be grouped under `Heading` rows derived from `PremiereDate` with relative labels (mirroring Emby web: "Yesterday", weekday + date). Verify: `cargo nextest run -p mbv` covers headings emitted from mixed `PremiereDate`s and no `Heading` when dates are absent.
- [x] 9.2 Upcoming row text: each `Upcoming` row SHALL render `SeriesName` as primary and `"Sxx:Eyy — episode title"` (from `ParentIndexNumber`/`IndexNumber`/`Name`) as subtitle — never the bare `"{n}. {name}"` shape. Verify: tests cover Virtual/id-less rows rendering series + season/episode context.
- [x] 9.3 Synthesized stable targets: id-less `Upcoming` rows SHALL carry stable per-row targets keyed by series + season/episode identity (never a shared empty `Id`), so cursor, selection, and resolvers never first-match the wrong row. Verify: tests cover two id-less rows in one list resolving independently.
- [x] 9.4 Mouse/keyboard agreement: mouse activation of an id-less `Upcoming` row SHALL take the series-direct path first and SHALL NOT consult an empty-Id first-match resolver beforehand; keyboard and mouse SHALL agree row-for-row. Verify: tick-integration tests click each of several id-less rows and land on the clicked row's series.
- [x] 9.5 TV `Latest` renders identically to Home's `Latest` TV section (HARD): the same items SHALL produce the same row text (primary/secondary/trailing) on both surfaces. Verify: a differential test feeds identical items to both render paths and asserts identical text.
- [x] 9.6 Wide flat-mode pane: `Latest`/`Upcoming` in non-mini geometry SHALL reclaim the hero pane (no reserved black slab) — the arrangement split goes away with the suppressed content. Verify: buffer tests cover Wide flat modes painting the list across the full panel with mini view unchanged.
- [x] 9.7 Re-run the full gate (`cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all -- --check`, `cargo nextest run -p mbv`, `cargo nextest run --workspace`), re-verify against the real server, sync the §9 deltas into `openspec/specs/`, and re-archive. Verify: all commands clean, the manual note updated, and `openspec validate --all` clean.
- [x] 9.8 Re-clamp a restored `TvContentMode` the reopened library's current count no longer offers (count crossed `LIBRARY_PILL_THRESHOLD` between runs): resolve the count's default before paint/fetch, never paint a stale pill highlight. Verify: real-`Application::tick()` integration tests cover saved-`All`-grown-large reopening on `Latest` and saved-`S-Z`-shrunk-small reopening on `All`.
- [x] 9.9 (ADDED 2026-09-23, user ruling) One shared TV `Latest` list: the shell owns a single snapshot of each TV library's newest-episodes section (same ownership pattern as the shell-owned acknowledged `HomeLatestSource` set); Home's `Latest` TV section and the library's `Latest` mode both render from that snapshot, and a load/refresh on either surface updates the snapshot once so the other shows the same items without issuing its own fetch. Verify: real-`Application::tick()` integration tests assert both surfaces present the same items after either surface loads, and a refresh on one updates the other with no second fetch.
