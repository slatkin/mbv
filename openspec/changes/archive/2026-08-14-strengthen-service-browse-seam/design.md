## Context

See `proposal.md` for motivation and `specs/service-browse-dispatch/spec.md` for behavior. The read-only Audiobookshelf work introduced `TabSelection::{Home, Library(usize), AudiobookshelfLibrary(usize), Feeds}` and count-aware tab ordering while correctly keeping `LibraryTab`, `AudiobookshelfBrowseState`, and `FeedTabState` separate. PR #514 subsequently added explicit Audiobookshelf guards to keyboard and mouse paths after #513 was filed, removing the immediate panics on current main.

The remaining problem is structural. Input, refresh, rendering, help, context-menu, and action code repeatedly projects the selected tab into predicates and optional unqualified indexes. Several Emby helpers recover their target from mutable `App::tab`, and cursor helpers use a missing Emby index as library zero. Adding another guard fixes one caller but does not encode which destination may invoke an Emby action.

ADR 0002 establishes one centralized input authority with keyboard and mouse front ends. The browse seam should strengthen dispatch beneath that authority rather than create another input registry. #518, `activate-audiobookshelf-podcast-playback`, depends on this seam so selected Audiobookshelf episodes can reach Service-native queue construction without passing through Emby selection code.

## Goals / Non-Goals

**Goals:**

- Make the existing selected-tab enum the exhaustive routing token at every browse front door.
- Carry an explicitly matched destination index through Service-specific action chains instead of re-reading `App::tab` downstream.
- Prevent layout indexes and row targets from being interpreted by a different destination's mouse handler.
- Preserve current Home, Emby, Audiobookshelf, and Feeds behavior except where the specification closes refresh/help inconsistencies.
- Leave a stable Audiobookshelf activation point for the subsequent local-playback change.

**Non-Goals:**

- A common Service, library, browse-item, browse-level, or catalog trait.
- Service-qualified queue identity or any Audiobookshelf playback behavior.
- Moving browse dispatch or tab state into `mbv-core`.
- Replacing library vector indexes with persistent Service library IDs.
- Completing unrelated command-registry or context-menu generalization work.

## Decisions

### 1. Strengthen `TabSelection` instead of adding a parallel browse-target enum

Use the existing enum as the sole selected-destination representation. Rename `TabSelection::Library(index)` to `TabSelection::EmbyLibrary(index)` and `library_index()` to `emby_library_index()` so the Service precondition is explicit. Keep `AudiobookshelfLibrary(index)`, `Home`, and `Feeds` as peer variants. Delete `TabSelection::from_position` and `TabSelection::to_position`; retain the count-aware mappings and require every live caller to supply current Emby, Audiobookshelf, and Feeds counts.

Do not add a second `LeftPanelBrowseTarget` enum. It would duplicate `TabSelection`, require synchronization, and permit disagreement about the selected destination. Do not replace indexes with stable IDs in this change: each Service's browse state already owns identity restoration, while tab ordering is currently vector-backed and app-local.

### 2. Match the selected destination positively at browse front doors

At keyboard view dispatch, mouse scrolling/selector/click/double-click/right-click handling, browse rendering, refresh, help classification, context-menu construction, and tab activation, match every `TabSelection` variant explicitly. Service branches call Service-specific handlers; there is no default branch whose meaning is Emby.

Global commands and overlay precedence remain above this seam per ADR 0002. For keyboard input, the exhaustive destination match belongs at the existing final `CONTEXT_STACK` rendezvous, `App::handle_key_view_dispatch`, without adding a context-stack entry. Shared `handle_global_view_key` behavior remains ahead of destination-specific handling. Mouse spatial mechanics remain local, but each left-panel entry point matches the destination before interpreting geometry. This avoids duplicating the command registry while making addition of another destination a compile-time audit of exhaustive matches.

`App::apply_tab_position` remains the tab-activation owner for keyboard and mouse selection through `set_library_tab`, `library_tab_next`, and `library_tab_prev`. It maps the displayed position, focuses the library panel where applicable, resets stale card dimensions, activates the matched Service browse state, ensures visibility, and saves preferences. Selecting an Audiobookshelf tab preserves or restores its Service-scoped show position and loads detail; it does not enter episode selection or submit playback. Startup continues to select Home; restoring a persisted selected tab is not added by this change.

Alternative rejected: retain ordered `is_home` / `is_feeds` / `is_audiobookshelf` guards. PR #514 demonstrates that guards can repair behavior, but each new interaction must independently preserve their order and remember that the final branch is Emby.

### 3. Pass explicit Emby indexes through Emby action chains

Service entry handlers receive the index extracted by the exhaustive match and pass it to downstream selection, cursor, refresh, search, shuffle, watched-state, enqueue, and navigation operations. Rename only routing-reachable generic helpers whose Emby-only contract remains hidden after parameterization. Remove `unwrap_or(0)` and equivalent fallback behavior from Emby browse helpers.

One `App::normalize_stale_browse_destination() -> bool` helper checks `TabSelection::EmbyLibrary(index)` against `App::libs` and `TabSelection::AudiobookshelfLibrary(index)` against `App::audiobookshelf_libraries` before each browse front door dispatches. A stale Service library index changes the selected destination to Home and returns `true`, causing the triggering destination-specific operation to stop. The return value is internal control flow, not a toast, log, or other user-visible report. This check owns asynchronous Service removal/replacement invalidation; downstream Service helpers may still bounds-check defensively but do not choose another destination.

Do not require an index newtype in this change. The enum variant qualifies the index at extraction, and explicit parameter flow removes the unsafe source. Wrapper types would increase mechanical churn without changing the vector-backed invariants. A Service helper may still bounds-check before use because Service lifecycle changes can invalidate an index asynchronously.

Alternative rejected: perform one exhaustive match and then call existing generic helpers that re-read `App::tab`. That would make the front door look safe while preserving the hidden precondition and library-zero fallback underneath it.

### 4. Keep Service-local mouse hit testing

After matching `TabSelection`, delegate spatial interpretation to Home, Emby, Audiobookshelf, or Feeds hit-testing code. A Service may continue to use shared layout geometry and numeric row maps internally, but another Service never reads those values. Preserve `App::render` constructing a fresh `AppLayout::default()` and installing it only after a completed frame; this is the reset boundary for render-published hit targets. Add `LayoutMain::browse_destination: Option<TabSelection>` and set it to the rendered destination only on the completed layout that is installed. Browse mouse handling compares that tag with the normalized selected destination and performs no action when they differ or the tag is absent.

Do not introduce a universal browse-row enum carrying all Service row models. A typed layout target could be useful if cross-Service row widgets later share interaction behavior, but it would broaden #513 into a layout protocol redesign. Positive Service dispatch is the smaller boundary that prevents cross-Service interpretation now.

### 5. Keep actions concrete and expose only destination-valid surfaces

Emby selection and context actions continue to operate on `EmbyItem`; Feed and Audiobookshelf state retain their native types. Context-menu construction first matches panel and destination. Home and Emby retain their existing menus when their branch resolves a supported `EmbyItem`; Audiobookshelf and Feeds browse rows, non-Emby queue items, and empty or stale targets produce no Emby menu. This change does not introduce a cross-Service row-kind taxonomy or alter which Home item a pre-existing Home context selects.

Help uses the same destination classification. With library focus, it places the matched destination section first while retaining other sections below it. The Home section remains limited to section switching, watched state, and enqueue. A new Audiobookshelf section lists show navigation, paging, first/last show, entry into episode selection, episode navigation, filter cycling, return to show selection, and inert activation; it advertises no Emby play, enqueue, search, watched, shuffle, rescan, route, or context action. With queue focus, Queue is first and the retained browse destination is not treated as active.

F5 refresh matches panel and destination: Home reloads Home content, Emby reloads the matched library, Audiobookshelf clears and restarts its current catalog request, Feeds calls its feed refresh, and Queue refreshes only the visible queue. This intentionally fixes current Feeds no-op and Audiobookshelf clear-only behavior.

Alternative rejected: generalize `ContextAction` or selected browse items into a Service enum. No shared context-action contract is needed for the current read-only Audiobookshelf surface, and the later shared boundary is `QueueItem`, not browse selection.

### 6. Preserve the playback handoff boundary

This change keeps Audiobookshelf episode activation and enqueue inert. `activate_audiobookshelf_episode(audiobookshelf_library_index)` and `enqueue_audiobookshelf_episode(audiobookshelf_library_index)` are the Service-specific action seams used by keyboard and mouse entry points. They resolve only Audiobookshelf browse state and, in this change, consume supported episode requests without mutation. #518 replaces their inert episode behavior with native episode extraction and explicit play/enqueue submission. No browse action creates a queue item merely because the dispatch seam exists.

The follow-on change extends `QueueItem` and owner admission independently. It must not reopen generic Emby browse handlers or infer Service from queue metadata.

### 7. Pin the implementation surfaces

The following source surfaces define the review boundary. Paths may move during implementation, but replacement symbols must retain these responsibilities:

| Concern | Current or required surface |
|---|---|
| Selected destination | `src/app/types_tab_selection.rs`: rename `TabSelection::Library` to `EmbyLibrary` and `library_index()` to `emby_library_index()` |
| Position conversion | Delete `TabSelection::from_position` and `to_position`; retain the count-aware conversions |
| Keyboard rendezvous | `src/app/input.rs`: `App::handle_key_view_dispatch`, reached as the final `CONTEXT_STACK` entry |
| Tab activation | `src/app/cw_library_tab_actions.rs`: `App::apply_tab_position` and its `set_library_tab` / next / previous callers |
| Stale destination | `App::normalize_stale_browse_destination() -> bool`, used by keyboard, mouse, refresh, render, help, context-menu, and tab-navigation front doors |
| Completed browse layout | `src/app/layout.rs`: `LayoutMain::browse_destination: Option<TabSelection>`, checked before browse mouse hit testing |
| Audiobookshelf handoff | `activate_audiobookshelf_episode(index)` and `enqueue_audiobookshelf_episode(index)`, inert until #518 |

The routing-reachable Emby chains to parameterize and audit are: `move_lib_cursor_rows` / `move_lib_cursor` / `jump_lib_cursor`; `handle_key_lib_search`; `current_lib_item` / `select` / `go_back`; `enqueue_selected`; `shuffle_play`; `toggle_watched`; `refresh_lib`; and `context_menu_lib_idx` / `open_context_menu`. Rename a helper only when its post-parameterization name still conceals an Emby-only contract.

The render-published browse fields consumed by mouse handling are also part of the audit: Home uses `home.hitmap`; Feeds uses `left_row_map`; Audiobookshelf uses `hero_area`, `audiobookshelf_episode_rows`, `left_item_rows`, and `left_screen_offset`; Emby uses `hero_area`, `left_row_map`, `left_row_targets`, `left_item_rows`, `left_screen_offset`, `wide_music_track_hitmap`, `wide_music_art_area`, `wide_music_right_area`, and `breadcrumbs`. Shared `left_area` and `selector_tabs` remain geometry, not destination identity. Fresh-frame replacement, the completed-frame destination tag, and positive dispatch prevent one destination from consuming another's published fields.

## Risks / Trade-offs

- **[Risk] Parameterizing mature Emby action chains touches many call sites** -> Change one action family at a time, preserve behavior at the Service boundary, and use compiler errors plus focused interaction checks to find every caller.
- **[Risk] Current main has moved beyond the local checkout through PR #514** -> Base implementation and verification on current main, retain #514's observable podcast navigation, and replace its guards with exhaustive routing rather than deleting behavior.
- **[Risk] Shared layout fields can describe the previous destination until redraw completes** -> Preserve fresh-frame replacement, tag installed layouts with their rendered destination, and no-op mouse browse handling until the tag matches the selected destination.
- **[Risk] Exhaustive matching becomes repetitive** -> Centralize only selection of the Service handler; keep Service-specific state machines in their current modules rather than abstracting unlike behavior.
- **[Trade-off] Raw indexes remain inside enum variants and Service handlers** -> Accept the existing vector-backed tab model; explicit variant matching and bounds checks provide the required safety without a broader identity migration.
- **[Trade-off] Unsupported Audiobookshelf context actions remain absent** -> Prefer an honest no-action surface until playback adds Service-native actions instead of displaying Emby operations that cannot work.

## Migration Plan

1. Start from current main including PR #514 and identify existing durable checks for mixed-tab keyboard, mouse, refresh, help, and inert Audiobookshelf activation behavior.
2. Rename the Emby tab variant/accessor and remove `from_position` / `to_position`, allowing exhaustive compiler errors to expose affected routing sites.
3. Add stale-destination normalization and positive Service dispatch at keyboard, refresh, render/help/context-menu, tab activation, and mouse front doors while preserving global input precedence.
4. Parameterize and rename routing-reachable Emby helpers, then remove missing-index-to-library-zero fallbacks.
5. Preserve fresh-frame layout replacement, tag completed browse layouts with their destination, and keep hit testing Service-local.
6. Verify mixed destination behavior and leave the named Audiobookshelf episode action seams inert for #518.

No persisted or protocol migration is required. Rollback restores the prior guard-based routing and helper signatures; Service setup, browse data, queue state, and playback state require no conversion.
