## 1. Audiobookshelf 2.36 Catalog Contract

- [x] 1.1 Capture minimal sanitized Audiobookshelf 2.36 responses for accessible mixed-media libraries, paginated podcast items, expanded downloaded episodes, current-user progress, personalized podcast shelves, and present/missing covers.
- [x] 1.2 Add concrete read-only library, podcast show, downloaded episode, progress, and shelf types without queue, stream, or playback-session fields.
- [x] 1.3 Add bounded Bearer-authenticated methods for library discovery, show pages, expanded podcast detail, bulk progress, personalized shelves, and cover bytes using the existing redacted failure classification.
- [x] 1.4 Verify fixture decoding, pagination metadata, provider-native identity, malformed-response handling, 2.36 incompatibility, authentication rejection, and credential redaction at the core API boundary.

## 2. Service Startup And Catalog Lifecycle

- [x] 2.1 Make initial Services routing treat configured Audiobookshelf as configured content even when Emby and Feeds are absent.
- [x] 2.2 Schedule Audiobookshelf startup validation from every TUI constructor, including clients attached to the Local daemon, without passing its credential or catalog work to the Player owner.
- [x] 2.3 Introduce concrete Audiobookshelf catalog state and generation-tagged completion events for discovery, pages, details, progress, shelves, and artwork.
- [x] 2.4 Start podcast-library discovery after accepted Ready completions, retain audiobook-only Services as Ready with no content tabs, and classify initial discovery failures through the Service lifecycle.
- [x] 2.5 Clear Audiobookshelf catalog, loading, selection, progress, shelf, and artwork state on authentication rejection, replacement, and removal; reject every stale-generation completion.

## 3. Peer Tab Navigation

- [x] 3.1 Introduce a typed Audiobookshelf-library tab destination and one ordered position mapping for Home, Emby libraries, Audiobookshelf libraries, and Feeds.
- [x] 3.2 Wire keyboard and mouse tab navigation, titles, visible-range calculations, refresh, and provider-specific loading, empty, and error rendering for Audiobookshelf tabs.
- [x] 3.3 Audit and guard all non-Home tab dispatch so Audiobookshelf selections cannot reach Emby indexing, activation, playlist, watched-state, shuffle, route, search, or context-menu behavior.
- [x] 3.4 Verify mixed tab ordering and round trips, Audiobookshelf-only startup, audiobook-only Ready state, and images-off rendering without brittle visual snapshots.

## 4. Paginated Podcast Shows

- [x] 4.1 Load the first bounded show page for each discovered podcast library and expose additional pages as navigation approaches the loaded boundary.
- [x] 4.2 Deduplicate pages by `libraryItemId`, prevent duplicate in-flight page requests, and retain loaded shows while a later page is pending or fails.
- [x] 4.3 Render podcast show rows with concrete Audiobookshelf selection state and restore the selected show by identity across append and refresh, falling back to the nearest valid row when removed.
- [x] 4.4 Verify page ordering, duplicate suppression, stable selection, scoped page failure, and stale page rejection at the state-transition boundary.

## 5. Inline Downloaded Episodes And Progress

- [x] 5.1 Load and cache expanded podcast detail by `libraryItemId` when a show is selected, deriving rows only from downloaded episodes in the expanded response.
- [x] 5.2 Add explicit show and episode row identities and render the selected show's episodes inline with title, publication information, duration, and loading or empty state.
- [x] 5.3 Keep episode rows selectable while making activation inert with no queue mutation, playback request, session creation, or progress write.
- [x] 5.4 Fetch the current user's progress snapshot in bulk, index it by `(libraryItemId, episodeId)`, and hydrate inline episode resume and finished presentation without polling or persistence.
- [x] 5.5 Verify rapid show-selection races, empty downloaded-episode lists, missing progress, cross-show episode-ID isolation, selection movement, and inert activation.

## 6. Authenticated Artwork

- [x] 6.1 Separate provider-neutral image decode, crop, throttle, and terminal-protocol handling from provider-specific authenticated byte retrieval.
- [x] 6.2 Add generation-tagged Audiobookshelf cover requests whose cache identity includes Service kind, configured server, native item identity, and presentation suffix but never the API key.
- [x] 6.3 Connect Audiobookshelf replacement and removal cleanup callbacks to memory cache, pending requests, and any Service-owned disk cache entries.
- [x] 6.4 Verify Bearer-header use, redacted failures, missing-cover behavior, stale image rejection, replacement cache isolation, and functional text/placeholder browsing with images disabled.

## 7. Personalized Podcast Shelves

- [x] 7.1 Decode the supported Audiobookshelf 2.36 podcast shelf variants while preserving server label and order and normalizing entries only to show or downloaded-episode navigation identities.
- [x] 7.2 Render personalized shelves within their Audiobookshelf library and navigate resolvable entries through the same show selection and inline episode model.
- [x] 7.3 Degrade unresolved or inaccessible shelf entries independently and keep Audiobookshelf shelves out of the cross-Service Home tab.
- [x] 7.4 Verify show shelves, episode shelves, unresolved entries, stable server order, stale shelf rejection, and unchanged Home behavior.

## 8. Boundary And Final Verification

- [x] 8.1 Audit the completed change to confirm it adds no Audiobookshelf `QueueItem`, stream resolution, playback session, progress mutation, Socket.IO connection, ctrl protocol behavior, Player-owner request, shared-state identity, or packaged `mbvd` behavior.
- [x] 8.2 Run focused mbv-core catalog-contract and App state-transition checks, then `cargo check -p mbv-core`, relevant binary checks, `cargo clippy --workspace --all-targets`, formatting verification, and `make check-code-file-lines`.
- [ ] 8.3 Manually verify peer-tab navigation, pagination, inline episode selection, progress, shelves, replacement/removal cleanup, unavailable Service recovery, audiobook-only Ready state, and images-off presentation against Audiobookshelf 2.36.
