## Context

See `proposal.md` for motivation and `specs/audiobookshelf-podcast-browsing/spec.md` for behavior. #503 established Service-independent startup and concrete Service runtimes; #505 added an identity-only `AudiobookshelfClient`, isolated API-key persistence, setup generations, and transactional replacement/removal. Audiobookshelf currently owns no catalog state, so its cleanup callbacks are intentionally empty.

The existing browser is concrete and Emby-shaped: `TabSelection::Library` indexes `App::libs`, `LibraryTab` and `BrowseLevel` hold `EmbyItem`, render and input paths use `library_index()`, and activation reaches Emby playback behavior. Artwork combines shared decode/cache machinery with Emby-specific request construction. These seams must admit a second concrete catalog without creating a false common media model.

Audiobookshelf 2.36 is the minimum supported contract. Relevant reads are accessible libraries, paginated library items, expanded podcast items, all current-user media progress, personalized library shelves, and authenticated item covers. Downloaded episodes in an expanded podcast are the catalog boundary; remote feed episodes that Audiobookshelf has not downloaded are not local media entries.

## Goals / Non-Goals

**Goals:**
- Preserve concrete Audiobookshelf identities and response semantics through a read-only core API boundary.
- Give Audiobookshelf libraries first-class tab and selection behavior without allowing Emby action fall-through.
- Make every catalog surface safe across concurrent requests and Service replacement/removal.
- Leave stable catalog and navigation seams that local podcast playback can extend next.

**Non-Goals:**
- A generic Service, library, media-item, shelf-item, or browse-level trait shared with Emby or Feeds.
- Persisting Audiobookshelf catalogs or progress for offline browsing.
- Queue identity, playback sessions, URL resolution, progress writes, Socket.IO, ctrl, Player-owner, or daemon changes.
- Compatibility fallbacks for Audiobookshelf versions older than 2.36.
- Cross-Service Home, global search, library routing, or audiobook presentation.

## Decisions

### Decision 1: Extend the concrete Audiobookshelf core boundary with minimal read models

Add concrete library, podcast show, podcast episode, media progress, and personalized shelf response types beside the existing identity client. API methods accept the Service credential only while constructing a bounded authenticated request and return redacted typed failures. Capture sanitized Audiobookshelf 2.36 responses for each supported endpoint before finalizing the wire types.

The public domain types retain only fields used by browsing, while private wire types absorb response nesting and optionality. A show carries `libraryItemId`; an episode carries both `libraryItemId` and `episodeId`. Neither type carries a playback `sessionId`, stream URL, or queue representation.

Converting Audiobookshelf responses to `EmbyItem` was rejected because it would erase provider semantics and make existing Emby actions appear valid. A broad provider trait was rejected because discovery, pagination, shelves, and detail responses do not yet share useful behavior with Emby beyond presentation concepts.

### Decision 2: Load one REST catalog snapshot from a Ready transition

Applying a successful Audiobookshelf startup/setup/Test result schedules library discovery with the accepted setup generation. Discovery retains only podcast libraries. Each library owns paginated show state; expanded details are loaded by show identity. Current-user progress is fetched in bulk from `GET /api/me/progress` and indexed by `(libraryItemId, episodeId)` rather than issuing one request per episode. Personalized shelves load per library through `/api/libraries/{id}/personalized`.

Every worker completion carries the setup generation and request identity. Results for a non-current generation are discarded. Detail results are cached by show identity but only rendered when that identity is still selected. Page results are keyed by library and page and deduplicated by `libraryItemId`.

Loading all progress avoids unbounded episode fan-out and gives shelves and expanded details one consistent REST snapshot. Polling was rejected because live refresh belongs to the Socket.IO milestone; persisted snapshots were rejected because this milestone promises no offline catalog.

### Decision 3: Add a distinct Audiobookshelf tab selection and browse state

Extend tab selection with an explicit Audiobookshelf-library destination rather than appending Audiobookshelf data to `App::libs`. Build the displayed tab strip from typed destinations so Home, Emby libraries, Audiobookshelf libraries, and Feeds have one ordered keyboard/mouse mapping without interpreting every non-special tab as Emby.

Audiobookshelf browse state is concrete and separate from `LibraryTab`: library identity, paged shows, selected show identity, expanded episodes, selected row identity, shelf state, and loading/error state. Input and rendering dispatch on the tab variant before reaching provider-specific behavior. Episode rows are selectable now but activation returns without action, preserving the navigation model for the playback milestone.

Widening `LibraryTab` or `BrowseLevel` into enums was rejected because it would spread provider branching through mature Emby browsing. Treating Audiobookshelf as a special Home section was rejected because #504 defers cross-provider Home aggregation and the user chose peer library tabs.

### Decision 4: Use an inline hierarchical row model for show details

The selected show expands its downloaded episodes immediately beneath its show row. Row identity is explicit (`Show(libraryItemId)` or `Episode(libraryItemId, episodeId)`), so cursor movement, restoration, and future activation do not depend on flattened positional coincidences. Selecting another show starts or reuses its detail request and collapses the previous inline detail.

Episodes display title, publication information, duration, and progress/completion where available. The client uses the expanded podcast's episode collection as the downloaded-media authority and does not synthesize rows from the remote podcast feed.

A separate drill-down level was rejected by the user's chosen interaction. Making episodes non-selectable was rejected because playback should be able to add activation later without replacing navigation and row identity.

### Decision 5: Keep personalized shelves library-local and normalize only navigation targets

Preserve each supported shelf's server-provided label and order. Decode the known 2.36 show and episode shelf shapes into a small concrete shelf-entry enum that points at a show or downloaded episode identity. Shelf activation changes selection within that Audiobookshelf library and opens the same inline detail; unresolved or inaccessible entries cannot poison the whole shelf response.

Creating a universal shelf item was rejected because the response shapes are provider-specific and audiobook shelves will introduce different semantics later. Rendering these shelves in Home was rejected because it would prematurely establish cross-Service aggregation and selection rules.

### Decision 6: Split shared image processing from provider-specific byte retrieval

Retain one decode, crop, protocol, in-flight throttling, and memory-cache path. Replace Emby-only fetch parameters at its boundary with a provider-specific image source: existing Emby image identity or Audiobookshelf cover identity plus captured setup generation. The Audiobookshelf source loads the current credential at request construction and sends it only in the Bearer header through the concrete API boundary.

Cache identity includes Service kind, configured server identity, native item identity, and presentation suffix; it never includes the credential. Audiobookshelf image completions are generation-checked. Replacement/removal cleanup becomes real by clearing Audiobookshelf catalog image entries and pending requests before the lifecycle transaction commits the new setup.

Duplicating the full image pipeline was rejected because decode and terminal protocol handling are provider-neutral. Passing authenticated cover URLs with query credentials through general UI state was rejected because it expands secret exposure and cache-key risk.

### Decision 7: Catalog failures reuse Service authentication semantics but retain local request context

An explicit 401/403 from an authenticated Audiobookshelf catalog request follows #505: clear the rejected secret, enter Needs authentication, and clear visible Audiobookshelf catalog state. Connectivity, server, malformed-response, and unsupported-2.36-contract failures preserve setup and credential. Initial discovery failure enters Unavailable and exposes no tabs; a later page, detail, shelf, progress, or artwork failure leaves already loaded content usable and presents a scoped retryable error rather than deleting the credential.

Treating all endpoint failures as authentication rejection was rejected because transient and compatibility failures do not prove the key invalid. Clearing the whole catalog for one optional shelf or cover failure was rejected because those surfaces can degrade independently.

### Decision 8: Close the two setup-to-browse lifecycle gaps first

Initial Services routing considers configured Audiobookshelf alongside Emby and Feeds, so an Audiobookshelf-only setup is not mistaken for an empty application. Every client constructor that can host the TUI, including attachment to the Local daemon, schedules Audiobookshelf validation when setup and credential exist. This remains client-side catalog work; the attached Player owner receives no Audiobookshelf credential or catalog request.

Leaving these as unrelated cleanup was rejected because the same configured Service would otherwise browse differently depending on Player-owner mode, violating Service-independent startup.

## Risks / Trade-offs

- **[Risk] Existing non-Home/non-Feeds paths assume an Emby library and unwrap its index** -> Introduce typed tab destinations and provider dispatch before any Audiobookshelf tab becomes visible; audit input, rendering, refresh, context-menu, search, route, playlist, watched-state, and shuffle paths.
- **[Risk] Audiobookshelf 2.36 shelf and expanded-item shapes vary by shelf or missing media** -> Capture sanitized live fixtures, keep wire fields optional where absence is valid, and normalize only supported navigation identities.
- **[Risk] Bulk progress can be large** -> Fetch it once per catalog refresh under the existing hard bound, index only progress relevant to accessible podcast identities, and do not persist it.
- **[Risk] Concurrent page/detail requests apply stale presentation state** -> Tag by setup generation and request identity, cache by native identity, and derive visible detail from the current selection.
- **[Risk] Authenticated artwork leaks the API key** -> Keep credentials out of URLs, events, cache keys, diagnostics, and retained App state; request through the redacted Audiobookshelf API boundary.
- **[Risk] Replacement cleanup failure leaves old state visible** -> Clear Service-owned in-memory catalog and image state through the transactional lifecycle callback before committing replacement; generation checks reject late completions.
- **[Trade-off] Audiobook-only users see Ready with no content tab** -> Accept this deliberate roadmap state; audiobook tabs arrive only when their file, track, and chapter model exists.
- **[Trade-off] Progress is stale until explicit REST reload** -> Accept the snapshot boundary; Socket.IO live refresh is roadmap milestone 5.

## Migration Plan

1. Record the supported Audiobookshelf 2.36 catalog fixtures and add minimal read-only core API types and methods.
2. Fix Audiobookshelf-only initial routing and validation scheduling across bare and Local daemon client construction.
3. Add generation-safe discovery and concrete Audiobookshelf browse state without exposing tabs.
4. Add typed peer-tab navigation and provider dispatch, then expose paginated show lists.
5. Add inline expanded episodes and bulk progress hydration while keeping activation inert.
6. Add provider-specific authenticated artwork retrieval and transactional cleanup.
7. Add library-local personalized shelves and audit the complete change for playback, queue, ctrl, and Socket.IO absence.

The change is additive and introduces no persisted migration. Rollback removes Audiobookshelf tabs and catalog state while leaving #505 setup and credentials intact; existing Emby and Feeds behavior remains available.
