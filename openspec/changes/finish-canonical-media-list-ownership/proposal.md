# Finish Canonical Media-List Ownership

## Why

The canonical media-list campaign was closed after functional and bounded seam verification, but a source-to-spec audit found that every named destination except Queue still violates part of the accepted ownership contract. This planning-only umbrella restores an accurate campaign ledger so later bounded changes repair verified failures without reopening accepted Queue work or expanding into unrelated lists.

## What Changes

- Record Queue as the sole completed destination under the full current contract.
- Record the verified remaining families: Grouped Music; Home; Feeds; Movies and the Emby homevideos feed view; TV Series; Audiobookshelf Podcasts; and Audiobookshelf Books.
- Preserve non-hero two-column Emby catalogs and destination-specific workspace authority (track, season/episode, chapter, episode-filter, image, effect, persistence) rather than treating those ownership boundaries as migration work; presentation that deviates from the Emby reference design is a repair target, not heritage.
- Require Emby-first canonical design: the Emby destinations are the most developed surfaces and are the design reference; the Feeds, Audiobookshelf Podcast, and Audiobookshelf Book families conform their presentation to that reference within their bounded changes instead of preserving deviant behaviour.
- Keep non-grouped Music (excluded by the 2026-09-09 decision) and the Emby podcast channel list (removed from scope by user direction) outside this campaign; neither is a ledger row.
- Require one separately explored and authorized bounded proposal per row or cohesive family; this umbrella neither creates those proposals nor authorizes implementation.
- Require durable requirement-to-source-and-test conformance evidence before a row can close. Functional verification, successful OpenSpec validation, automatic spec sync, or a bare review pass is not sufficient alone.
- Keep the human-readable campaign plan and status in GitHub issue #681 and agent-facing execution detail here, with each linking the other and neither silently replacing the other.
- Defer structural ratchets and documentation cleanup until a completed family establishes the concrete surviving boundary or the final audit proves a remaining need.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

None. This umbrella changes campaign governance and status tracking only. The existing `canonical-media-lists`, `interactive-component-framework`, and `right-panel-arrangements` specifications remain authoritative and are not weakened or restated here.

## Impact

- Adds a planning-only OpenSpec campaign ledger linked to GitHub issue #681.
- No runtime behavior, source code, API, dependency, persisted data, or behavioral specification changes.
- Future source changes occur only in separately authorized bounded OpenSpec changes.
