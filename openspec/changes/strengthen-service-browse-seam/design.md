## Context

See `proposal.md` for motivation and `specs/service-browse-dispatch/spec.md` for behavior. The read-only Audiobookshelf work introduced `TabSelection::{Home, Library(usize), AudiobookshelfLibrary(usize), Feeds}` and count-aware tab ordering while correctly keeping `LibraryTab`, `AudiobookshelfBrowseState`, and `FeedTabState` separate. PR #514 subsequently added explicit Audiobookshelf guards to keyboard and mouse paths after #513 was filed, removing the immediate panics on current main.

The remaining problem is structural. Input, refresh, rendering, help, context-menu, and action code repeatedly projects the selected tab into predicates and optional unqualified indexes. Several Emby helpers recover their target from mutable `App::tab`, and cursor helpers use a missing Emby index as library zero. Adding another guard fixes one caller but does not encode which destination may invoke an Emby action.

ADR 0002 establishes one centralized input authority with keyboard and mouse front ends. The browse seam should strengthen dispatch beneath that authority rather than create another input registry. The later `play-audiobookshelf-podcasts-locally` change depends on this seam so selected Audiobookshelf episodes can reach provider-native queue construction without passing through Emby selection code.

## Goals / Non-Goals

**Goals:**

- Make the existing selected-tab enum the exhaustive routing token at every browse front door.
- Carry an explicitly matched destination index through provider-specific action chains instead of re-reading `App::tab` downstream.
- Prevent layout indexes and row targets from being interpreted by a different destination's mouse handler.
- Preserve current Home, Emby, Audiobookshelf, and Feeds behavior except where the specification closes refresh/help inconsistencies.
- Leave a stable Audiobookshelf activation point for the subsequent local-playback change.

**Non-Goals:**

- A common Service, library, browse-item, browse-level, or catalog trait.
- Provider-qualified queue identity or any Audiobookshelf playback behavior.
- Moving browse dispatch or tab state into `mbv-core`.
- Replacing library vector indexes with persistent Service library IDs.
- Completing unrelated command-registry or context-menu generalization work.

## Decisions

### 1. Strengthen `TabSelection` instead of adding a parallel browse-target enum

Use the existing enum as the sole selected-destination representation. Rename its Emby variant and accessor so `EmbyLibrary(index)` and `emby_library_index()` state the provider precondition. Keep `AudiobookshelfLibrary(index)`, `Home`, and `Feeds` as peer variants. Remove the count-unaware position conversion methods; only the mapping that receives current Emby, Audiobookshelf, and Feeds counts can represent the mixed strip correctly.

Do not add a second `LeftPanelBrowseTarget` enum. It would duplicate `TabSelection`, require synchronization, and permit disagreement about the selected destination. Do not replace indexes with stable IDs in this change: each provider's browse state already owns identity restoration, while tab ordering is currently vector-backed and app-local.

### 2. Match the selected destination positively at browse front doors

At keyboard view dispatch, mouse scrolling/selector/click/double-click/right-click handling, browse rendering, refresh, help classification, context-menu construction, and tab activation, match every `TabSelection` variant explicitly. Provider branches call provider-specific handlers; there is no default branch whose meaning is Emby.

Global commands and overlay precedence remain above this seam per ADR 0002. The destination match handles only browse behavior after focus and global precedence are known. This avoids duplicating the command registry while making addition of another destination a compile-time audit of exhaustive matches.

Alternative rejected: retain ordered `is_home` / `is_feeds` / `is_audiobookshelf` guards. PR #514 demonstrates that guards can repair behavior, but each new interaction must independently preserve their order and remember that the final branch is Emby.

### 3. Pass explicit Emby indexes through Emby action chains

Provider entry handlers receive the index extracted by the exhaustive match and pass it to downstream selection, cursor, refresh, search, shuffle, watched-state, enqueue, and navigation operations. Rename generic helpers where needed so an Emby-only contract is visible at the call site. Remove `unwrap_or(0)` and equivalent fallback behavior from Emby browse helpers; an absent or stale index returns without action or is normalized at the tab boundary.

Do not require an index newtype in this change. The enum variant qualifies the index at extraction, and explicit parameter flow removes the unsafe source. Wrapper types would increase mechanical churn without changing the vector-backed invariants. A provider helper may still bounds-check before use because Service lifecycle changes can invalidate an index asynchronously.

Alternative rejected: perform one exhaustive match and then call existing generic helpers that re-read `App::tab`. That would make the front door look safe while preserving the hidden precondition and library-zero fallback underneath it.

### 4. Keep provider-local mouse hit testing

After matching `TabSelection`, delegate spatial interpretation to Home, Emby, Audiobookshelf, or Feeds hit-testing code. A provider may continue to use shared layout geometry and numeric row maps internally, but another provider never reads those values. Renderers must clear or overwrite the provider-local layout state they publish for the current frame.

Do not introduce a universal browse-row enum carrying all provider row models. A typed layout target could be useful if cross-provider row widgets later share interaction behavior, but it would broaden #513 into a layout protocol redesign. Positive provider dispatch is the smaller boundary that prevents cross-provider interpretation now.

### 5. Keep actions concrete and expose only destination-valid surfaces

Emby selection and context actions continue to operate on `EmbyItem`; Feed and Audiobookshelf state retain their native types. Context-menu construction first matches panel and destination, then offers only actions implemented for that row kind. An unsupported destination produces no provider-specific menu rather than an empty Emby selection path.

Help uses the same destination classification and adds an Audiobookshelf section that describes only current read-only navigation. Feeds remains separate from the Emby Library section. F5 refresh matches the destination: Home refreshes Home, Emby refreshes that library, Audiobookshelf refreshes its catalog state, and Feeds refreshes Feeds. Queue focus remains destination-independent.

Alternative rejected: generalize `ContextAction` or selected browse items into a provider enum. No shared context-action contract is needed for the current read-only Audiobookshelf surface, and the later shared boundary is `QueueItem`, not browse selection.

### 6. Preserve the playback handoff boundary

This change keeps Audiobookshelf episode activation and enqueue inert. It only establishes a provider-specific handler that the local-playback change can replace with native episode extraction and explicit play/enqueue submission. No browse action creates a queue item merely because the dispatch seam exists.

The follow-on change extends `QueueItem` and owner admission independently. It must not reopen generic Emby browse handlers or infer provider from queue metadata.

## Risks / Trade-offs

- **[Risk] Parameterizing mature Emby action chains touches many call sites** -> Change one action family at a time, preserve behavior at the provider boundary, and use compiler errors plus focused interaction checks to find every caller.
- **[Risk] Current main has moved beyond the local checkout through PR #514** -> Base implementation and verification on current main, retain #514's observable podcast navigation, and replace its guards with exhaustive routing rather than deleting behavior.
- **[Risk] Shared layout fields retain stale data across destination changes** -> Reset transient hit maps during render setup and let only the matched provider interpret fields populated by its current renderer.
- **[Risk] Exhaustive matching becomes repetitive** -> Centralize only selection of the provider handler; keep provider-specific state machines in their current modules rather than abstracting unlike behavior.
- **[Trade-off] Raw indexes remain inside enum variants and provider handlers** -> Accept the existing vector-backed tab model; explicit variant matching and bounds checks provide the required safety without a broader identity migration.
- **[Trade-off] Unsupported Audiobookshelf context actions remain absent** -> Prefer an honest no-action surface until playback adds provider-native actions instead of displaying Emby operations that cannot work.

## Migration Plan

1. Start from current main including PR #514 and characterize mixed-tab keyboard, mouse, refresh, help, and inert Audiobookshelf activation behavior.
2. Rename the Emby tab variant/accessor and remove legacy count-unaware tab-position conversion, allowing exhaustive compiler errors to expose affected routing sites.
3. Introduce positive provider dispatch at keyboard, refresh, render/help/context-menu, tab activation, and mouse front doors while preserving global input precedence.
4. Parameterize and rename routing-reachable Emby helpers, then remove missing-index-to-library-zero fallbacks.
5. Isolate provider-local hit testing and verify each renderer resets the transient layout state its handler consumes.
6. Verify mixed destination behavior and leave the Audiobookshelf episode activation boundary inert for the dependent playback change.

No persisted or protocol migration is required. Rollback restores the prior guard-based routing and helper signatures; Service setup, browse data, queue state, and playback state require no conversion.
