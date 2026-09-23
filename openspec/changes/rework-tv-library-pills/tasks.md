# Tasks

## 0. Sequencing precondition

- [ ] 0.1 Verify `extract-shared-list-components` is archived (`openspec list --json` no longer reports it active) and rebase onto main before touching `src/app/components/tv_content/`, `src/app/components/home_content/`, or `media_list` callers; if it is still active, hold implementation (design Risks). Verify: the command output shows the seam change archived.

## 1. Mode value and the TV bucket set

- [ ] 1.1 Add the TV-specific three-bucket table beside the existing movie table in `src/app/render/screens/sort_filter.rs` (`A-I` = `NameLessThan("J")`, `J-R` = `[J, S)`, `S-Z` = `NameStartsWithOrGreater("S")` with no upper bound) TV construction goes through TV-specific constructors (`for_tv_index`, TV `for_sort_key` variant) while the existing `for_index`/`for_sort_key` keep the nine-bucket movie table unchanged, and update the `activate_searched_series` TV landing (`src/app/lib_cursor_actions.rs`) to resolve via the TV table. Verify: `cargo nextest run -p mbv` covers the three TV labels/bounds, a non-letter name resolving to `A-I`, an `S`-or-later name resolving to `S-Z`, and the movie labels still returning the nine-bucket set, and a TV search-landing resolving into its containing TV bucket — expressed as named `#[case]` tables per repo convention.
- [ ] 1.2 Introduce a closed `TvContentMode` value (`Latest`, `Upcoming`, `All`, range) on the top-level TV `BrowseLevel` (additive `BrowseLevel.tv_content_mode` / `LibraryPositionLevel.tv_content_mode`; the movie `letter_filter` fields stay untouched) and remove the overloaded use of `letter_filter: Option<LetterFilter>` as both "no filter" and "the auto-applied default". Verify: `cargo check -p mbv` is clean and focused tests cover each mode's content selector and the unresolved/absent default.

## 2. Row composition, threshold, and defaults

- [ ] 2.1 Compose the TV content-mode row from the mode list in design D3: above `LIBRARY_PILL_THRESHOLD` the row is `Latest | Upcoming | A-I | J-R | S-Z` with `Latest` selected; at or below it the row is `Latest | Upcoming | All` with `All` selected. Verify: `cargo nextest run -p mbv` covers both rows, their order, the selected mode, and that `All` is absent above the threshold.
- [ ] 2.2 In `src/app/lib_event_actions.rs::maybe_capture_library_total_and_apply_default_pill`, keep the `library_total` capture unchanged and scope the change to TV: a TV library replaces the `A-C` auto-apply with default-mode selection (above the threshold auto-select `Latest` after the capture load and fetch its episodes; at or below it select `All` over the already-loaded unfiltered list with no re-fetch), and the small-library quirk where the TV row painted `A-C` highlighted over an unfiltered list is removed. Movie, feed, and podcast libraries keep today's auto-scope `A-C` behavior and row byte-for-byte; TV means `collection_type == "tvshows"` (the gate `shell_tv_workspace.rs` already uses), and every other non-music Emby collection type (homevideos, musicvideos, mixed) follows the movie byte-for-byte path. Verify: `cargo nextest run -p mbv` covers that a small TV library's first load is unfiltered with `All` selected and no second fetch, that a large TV library's first load selects `Latest` after the capture load with one episode fetch and no letter-scoped fetch, and that a large movie library still auto-applies `A-C` with its scoped refresh.

## 3. Latest mode

- [ ] 3.1 Wire the `Latest` mode to the same feed Home's TV section uses (`get_latest_episodes(view_id, 30)`) through the library's own fetch path, so it loads whether or not Home has. Verify: `cargo nextest run -p mbv` covers `Latest` producing the feed's episodes with no dependency on a loaded Home section.
- [ ] 3.2 Pass the `Latest` mode into the TV component's selector row with flat episode rows and no series detail fetch. Verify: `cargo nextest run -p mbv` covers the selector's active mode and that selecting `Latest` does not request a series detail.

## 4. Upcoming mode

- [ ] 4.1 Add `get_upcoming(parent_id, limit)` to `crates/mbv-core/src/api_client_library.rs` over `GET /Shows/Upcoming` with `ParentId` and limit 30 (same as `Latest`), and cover it with a mock/parse unit test. The test pins the request the client builds, not that the server honors it. Verify: `cargo nextest run -p mbv-core` is green, pinning the route, the `ParentId` query, and limit 30.
- [ ] 4.2 Wire the `Upcoming` mode to that fetch through the library's fetch path with flat episode rows. Verify: `cargo nextest run -p mbv` covers `Upcoming` requesting the route with the library as parent and presenting the returned episodes.

## 5. Episode activation and hero

- [ ] 5.1 Make `Latest` and `Upcoming` episode rows activate play, and ensure the TV component never treats an episode target as a series selection (no workspace, no season selector, no detail request). Verify: a real-`Application::tick()` integration test asserts activation plays the episode and that no series workspace opens.
- [ ] 5.2 Show the selected episode's hero only in the mini-view presentation, and none in the other geometries. Verify: `cargo nextest run -p mbv` covers mini view painting the episode hero and a non-mini geometry painting none.

## 6. Persistence, cycling, and mouse

- [ ] 6.1 Keep the selected `TvContentMode` on the in-session browse level and in the in-memory `LibraryPositionLevel.tv_content_mode` (movies keep `letter_filter_index` byte-for-byte). Restoring an in-session position uses precedence `tv_content_mode` → legacy nine-bucket TV `letter_filter_index` mapped to its containing TV bucket (0–7 → `index/3`, 8 → 0) → D3 size default. This field is not the orderly-exit store (D8). A large TV library reopened in-session on `Latest` issues the episode fetch as its initial load with no unfiltered capture fetch. Verify: `cargo nextest run -p mbv` covers in-session save/restore for each mode, the legacy-index mapping cases, and the reopen-issues-no-capture-fetch path.
- [ ] 6.2 Make `[`/`]` cycle over the modes actually present in the painted row with wrap, and make mouse selection select the clicked mode by translating the clicked position through the painted mode row (`src/app/mouse_gestures.rs`); update the `cycle_letter_pill` doc comment in `src/app/components/msg/shell.rs`. Verify: `cargo nextest run -p mbv` covers wrap on the three-mode small-library row, wrap on the five-mode large-library row, and mouse selection of each mode.
- [ ] 6.3 Persist the selected mode in `TuiLaunchState.selector` (design D8). Add `EmbySelectorKey::Latest`, `Upcoming`, and `TvRange(TvLetterBucket)` (`AToI` | `JToR` | `SToZ`) in `crates/mbv-core/src/config_launch_state.rs`; `All` stays `Unfiltered`; movie `Letter(EmbyLetterBucket)` is unchanged. TV `launch_snapshot` / `launch_selector` (`src/app/components/tv_content/mod.rs`) write and restore those identities whenever the content-mode row is shown. `migrate_legacy_launch_state` (`src/app/cw_library_tab_actions.rs`) maps a TV position (`collection_type == "tvshows"`) through the same precedence as 6.1 onto those identities; movie positions stay on `EmbyLetterBucket`. A restored key that is not in the current row uses the D3 size default. Verify: `cargo nextest run -p mbv` covers an orderly-exit snapshot of each mode restoring that mode on the next launch, `Latest` and `Upcoming` not restoring as `All`, a legacy nine-bucket TV index migrating to its containing `TvRange`, and a movie letter snapshot still restoring the nine-bucket pill.

## 7. Shared new-content marker

- [ ] 7.1 Hoist the acknowledged `HomeLatestSource` set out of the Home component into shell-owned state (moving the `restore_section` writers in `src/app/shell_home.rs`, which also mark visited) and have the Home selector read it; keep the marker value the shell-computed `has_new_content`. Verify: `cargo nextest run -p mbv` covers Home's marker rendering through the shell-owned set and that a fetch after launch does not add a marker, and that a section selected before its content arrives still counts as visited (no marker).
- [ ] 7.2 Render that same marker on the TV library's `Latest` mode for the matching library view, and share acknowledgement so selecting either surface clears both. Verify: a real-`Application::tick()` integration test selects the library `Latest` mode and asserts both markers clear, and the reverse via Home's pill.

## 8. Verification and close-out

- [ ] 8.1 Run the full gate: `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all -- --check`, `cargo nextest run -p mbv`, and `cargo nextest run --workspace`. Verify: all commands are clean.
- [ ] 8.2 Manually confirm against a real Emby server, using two TV libraries, that `/Shows/Upcoming` with the requested `ParentId` and limit 30 returns only episodes from the requested library, at most 30, and whether those rows are playable. This check is required before the change is called done, not before task 4.1. If the server ignores `ParentId` or `Limit`, or returns materially misleading rows, record the local filtering to add and add it before close-out. Verify: a note exists naming the observed ancestry, count, playability, and any filtering added.
- [ ] 8.3 Sync the applied deltas into `openspec/specs/`: apply the modified `tv-letter-filtering` and `home-latest-sections` requirements and add the `tv-library-content-modes` capability. Verify: `openspec validate --all` is clean.
- [ ] 8.4 Record any new domain vocabulary (the content-mode value and the TV range set) in `CONTEXT.md` without renaming existing terms. Verify: the terms appear with definitions.
- [ ] 8.5 Commit the implementation and the synced main specs. Verify: `git status --short` is empty after the commit.
- [ ] 8.6 Archive the change with `openspec archive rework-tv-library-pills`. Verify: `openspec list --json` no longer reports the change as active and `openspec validate --all` stays clean.
