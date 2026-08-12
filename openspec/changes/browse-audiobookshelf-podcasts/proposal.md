## Why

Audiobookshelf setup now reaches Ready without exposing any content. The next roadmap milestone must prove provider-specific podcast browsing while preserving the boundary that playback, queue transport, and live updates follow only after the catalog model is established.

Tracking issue: [#510](https://github.com/slatkin/mbv/issues/510)

Parent roadmap: [#504](https://github.com/slatkin/mbv/issues/504)

## What Changes

- Discover accessible Audiobookshelf 2.36 podcast libraries after the Service becomes Ready.
- Show each podcast library as a peer tab alongside Home, Emby libraries, and Feeds; audiobook libraries remain hidden until audiobook support arrives.
- Browse paginated podcast shows and expand downloaded episodes inline beneath the selected show.
- Keep episode rows selectable but inert on activation until local podcast playback is added.
- Display authenticated artwork, read-only episode progress, and personalized podcast shelves inside their Audiobookshelf library.
- Reconcile catalog work with the current Service setup generation and clear Audiobookshelf catalog and artwork state on replacement or removal.
- Correct Audiobookshelf-only first-launch routing and startup validation across Player-owner construction paths as prerequisites to consistent browsing.
- Exclude queue items, stream resolution, playback sessions, progress writes, Socket.IO, ctrl changes, Local daemon media support, cross-Service Home aggregation, global search, and audiobook browsing.

## Capabilities

### New Capabilities

- `audiobookshelf-podcast-browsing`: Read-only Audiobookshelf podcast library discovery, peer-tab navigation, show and inline episode browsing, progress, artwork, and library-local personalized shelves.

### Modified Capabilities

None.

## Impact

- Audiobookshelf 2.36 read-only API types and requests in `mbv-core`.
- Application Service startup, generation-tagged catalog workers, and Service-owned cleanup.
- Tab selection, input dispatch, browse state, rendering, and authenticated artwork retrieval in the TUI.
- No new dependency, persistence format, ctrl protocol, playback queue, Player-owner, shared-state, or packaged `mbvd` changes.
