# Finish Canonical Media-List Ownership

## Why

The canonical media-list campaign was closed after functional and bounded seam verification, but a source-to-spec audit found that every named destination except Queue still violates part of the accepted ownership contract. This planning-only umbrella restores an accurate campaign ledger so later bounded changes repair verified failures without reopening accepted Queue work or expanding into unrelated lists.

## What Changes

- Record Queue as the sole completed destination under the full current contract.
- Record the verified remaining families in Emby-first execution order: Grouped Music; Home; Movies and the Emby homevideos feed view; TV Series; then Feeds; Audiobookshelf Podcasts; and Audiobookshelf Books.
- Preserve non-hero two-column Emby catalogs and destination-specific workspace authority (track, season/episode, chapter, episode-filter, image, effect, persistence) rather than treating those ownership boundaries as migration work; presentation that deviates from the Emby reference design is a repair target, not heritage.
- Require Emby-first canonical design with full visual parity: the Emby destinations are the most developed surfaces and are the design reference; the Feeds, Audiobookshelf Podcast, and Audiobookshelf Book families converge fully on that reference (visual design, chrome, framing, and interaction) within their bounded changes instead of preserving deviant behaviour.
- Complete the Emby families (Grouped Music, Home, Movies/homevideos, TV Series) before the skeleton families (Feeds, Audiobookshelf Podcasts, Audiobookshelf Books); TV Series stays after the Movies/homevideos Browser family.
- Keep non-grouped Music (excluded by the 2026-09-09 decision) outside this campaign. Remove the obsolete Emby podcast channel-list requirement: it is not a core Emby feature, Audiobookshelf is the supported podcast path, and no further Emby-podcast work is planned; neither is a ledger row.
- Require one separately explored and authorized bounded proposal per row or cohesive family; this umbrella neither creates those proposals nor authorizes implementation.
- Close a row on merged PR plus reviewer sign-off, represented consistently here and in GitHub issue #681. No per-requirement trace or live-evidence bundle is required.
- Keep the human-readable campaign plan and status in GitHub issue #681 and agent-facing execution detail here, with each linking the other and neither silently replacing the other.
- Defer structural ratchets and documentation cleanup until a completed family establishes the concrete surviving boundary or the final audit proves a remaining need.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

None. This umbrella changes campaign governance and status tracking only. The obsolete Emby podcast channel-list requirement is corrected directly in the current specifications and architecture guidance; the remaining requirements stay authoritative.

## Impact

- Adds a planning-only OpenSpec campaign ledger linked to GitHub issue #681.
- No runtime behavior, source code, API, dependency, or persisted data changes.
- Removes obsolete Emby podcast requirements from current specifications and architecture guidance; existing historical implementation records remain history.
- Future source changes occur only in separately authorized bounded OpenSpec changes.
