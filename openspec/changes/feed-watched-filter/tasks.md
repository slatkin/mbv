## 1. Hydrate refreshed Feed entries

- [x] 1.1 Add a feed-scoped hydration path that scans stored state once for a normalized subscription URL and merges matching rows into parsed entries by GUID, ignoring unknown stored GUIDs.
- [x] 1.2 Invoke hydration before installing each successful refresh result and before rebuilding the All group, while preserving zero-position/unplayed entries on missing capability, disconnection, timeout, or scan failure.
- [x] 1.3 Keep playback-time point hydration intact and confirm refresh, filter cycling, and tab selection do not write feed-entry state or contact Emby.

## 2. Model one filtered Feed view

- [x] 2.1 Add an All, Watched, and Unwatched filter state to `FeedTabState`, default it to All, and provide shared filtered iteration, count, and indexed-selection accessors over the canonical group entries.
- [x] 2.2 Add the ordered filter transition and reset cursor and scroll on each transition without changing the selected subscription group.
- [x] 2.3 Route cursor/page bounds, age-heading row construction, mouse row mapping, play, and enqueue through the filtered accessors so every displayed index resolves to the displayed entry.

## 3. Wire and present the filter

- [x] 3.1 Consume only unmodified `w` in the Feeds-tab key handler and cycle `All -> Watched -> Unwatched -> All`, leaving modifier variants and non-Feeds watched actions unchanged.
- [x] 3.2 Render the active filter in the Feeds-tab header and add a compact played marker to played rows without adding resume timestamps or state-unavailable UI.
- [x] 3.3 Add Feeds-specific help text for the `w` watched filter and preserve existing refresh, group, play, and enqueue shortcuts.

## 4. Protect behavior and reconcile documentation

- [x] 4.1 Extend the closest existing Feed state/action tests only where they protect realistic regressions: filter order and empty results, filtered play/enqueue identity, and cursor/scroll reset.
- [x] 4.2 Protect the refresh boundary with focused coverage that proves feed-scoped state is merged by identity and unavailable state leaves fetched entries usable and unplayed; reuse existing helpers rather than adding a new UI snapshot or shared-daemon harness.
- [x] 4.3 Update the FeedEntry description in `CONTEXT.md` to describe its feed identity, roaming position/played fields, and no-Emby-reporting boundary.

## 5. Verify the full RSS state path

- [x] 5.1 Run `cargo fmt --all -- --check`, the narrow affected `cargo test -p mbv` tests, `cargo clippy --workspace --all-targets`, and `make check-code-file-lines`.
- [ ] 5.2 With the same authenticated user and normalized subscription URL on two machines, play beyond the 6% threshold on machine 1 while a client remains attached, pause or stop, refresh on machine 2, and verify playback resumes from the stored position.
- [ ] 5.3 Finish a known-runtime entry on machine 1, refresh machine 2, and verify Watched includes it, Unwatched excludes it, All marks it played, and cycling filters performs no state write.
- [ ] 5.4 Stop the shared-data daemon and verify feed refresh, browsing, filtering, and playback remain stateless and usable without a crash or phantom unavailable UI.
