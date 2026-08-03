## 1. Artist Data And Snapshot Lifecycle

- [ ] 1.1 Extend music browse item decoding to retain album artist metadata returned by Emby, while preserving the existing fallback data for folder-backed albums.
- [ ] 1.2 Add per-music-browse-level grouping state that represents a source revision, a resolving candidate, and a settled grouped catalog.
- [ ] 1.3 Start or supersede grouping candidates when music album levels load, refresh, or append pages; keep a prior settled catalog visible while a replacement resolves.
- [ ] 1.4 Move fallback artist lookup scheduling out of rendering, retain bounded concurrency, and make successful, empty, failed, and expired lookups terminal for the candidate.

## 2. Stable Catalog And Continuity

- [ ] 2.1 Extract a pure source-derived grouped catalog from the current display-plan logic, including resolved artist identities, display metadata, precomputed sort keys, order, group boundaries, and ID lookup data.
- [ ] 2.2 Commit a resolved candidate only when its source revision still matches the active browse level, discarding obsolete candidates without letting them alter the visible grouping.
- [ ] 2.3 Anchor replacement commits by selected album or artist-header identity and nearby visible album identity, then restore the closest valid cursor, header focus, and scroll position.
- [ ] 2.4 Route grouped navigation and artist-header actions through the settled catalog so their targets and membership match the visible list.

## 3. Rendering Integration

- [ ] 3.1 Render an organizing state when an initial music album snapshot has no settled catalog, without changing non-music or non-album library rendering.
- [ ] 3.2 Make the grouped album renderer consume the settled catalog and keep catalog construction, sorting, and artist lookup scheduling out of terminal painting.
- [ ] 3.3 Narrow presentation-plan invalidation to its dynamic inputs such as selection, width, artwork, and track detail while reusing unchanged grouped ordering across redraws.

## 4. Verification

- [ ] 4.1 Add focused coverage for incomplete artist data, terminal fallback, one-time publication, late-result stability, and obsolete-result isolation.
- [ ] 4.2 Add focused coverage for page replacement, selection and viewport anchoring, and artist-header actions using the visible settled membership.
- [ ] 4.3 Verify repeated unchanged renders neither resort the grouped catalog nor start artist lookups; run formatting and the relevant Rust test suite.
- [ ] 4.4 Manually exercise first opening of a configured grouped music library against delayed or incomplete artist metadata to confirm the organizing-to-settled transition has no progressive reshuffling.
