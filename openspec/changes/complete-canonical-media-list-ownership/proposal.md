# Complete Canonical Media-List Ownership

## Why

PR #684 repaired the shared media-list foundation, but splitting the original refactor left no active, authoritative plan connecting that foundation to the remaining visible screens. This planning-only umbrella restores the complete screen inventory, fixed proposal order, dependencies, and campaign exit criteria so intent cannot be lost between bounded implementation changes.

## What Changes

- Establish one active campaign ledger for Queue, Grouped Music, Home, Feeds, Movies, the Emby homevideos feed view, the Emby podcast channel list, TV Series, Audiobookshelf Podcasts, and Audiobookshelf Books.
- Record PR #684 as the accepted Queue and Grouped Music painting/geometry foundation while retaining the unfinished Grouped Music album-position repair as an explicit follow-on.
- Fix the order and dependencies of bounded screen-level proposals instead of selecting the next screen ad hoc.
- Define how each follow-on links back to this umbrella, preserves screen-specific workspaces, records evidence, and updates campaign status when accepted.
- Preserve other Emby library tabs that intentionally use two-column catalogs and preserve the TV season grid rather than forcing either into the one-column media-list controls.
- Hold final enforcement rules and documentation reconciliation until the screen changes reveal the surviving boundaries.
- Exclude implementation from this umbrella; every screen repair remains a separate OpenSpec change.
- Exclude Inline Search, Search sidebar, playlist management, Settings, Sessions, unrelated non-media lists, and multi-select.

## Capabilities

### New Capabilities

- None.

### Modified Capabilities

- None. This umbrella changes campaign governance and status tracking only; screen behavior remains governed by `canonical-media-lists` and each bounded follow-on delta.

## Impact

- Adds an active planning-only OpenSpec change under issue #681.
- Makes this change's `tasks.md` the authoritative campaign-status ledger; issue #681 becomes a concise mirror and index.
- References the accepted PR #684 archive and existing `canonical-media-lists` specification without changing project code, runtime behavior, APIs, dependencies, or persisted data.
