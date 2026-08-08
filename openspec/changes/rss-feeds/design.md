## Context

mbv currently supports only Emby-sourced media. The queue, playback reporting,
and position tracking are all Emby-specific. See proposal.md for motivation.

The shared-mbv-state capability provides per-user document storage via the
mbvd-hosted redb. Currently it stores four fixed document kinds (QueueState,
LibraryPositionState, LastRemoteConnection, RoamingSettings) accessed through
a dedicated socket protocol. RSS feeds require extending this storage.

## Goals / Non-Goals

**Goals:**
- Feed subscriptions and entry state roam across machines via shared store
- Feed entries can be queued and played alongside Emby items
- Existing feed-view UI structure is reused for RSS browsing

**Non-Goals:**
- Daemon-side feed fetching or caching
- Supporting feeds without mbvd connectivity
- Per-entry audio/video kind (feeds are uniformly one kind)

## Decisions

### Decision: Extend SharedDocumentKind for feed data

Add two new document kinds to the existing shared-store protocol:
- `FeedSubscriptions` — list of feed subscriptions with URL, title override, kind
- `FeedEntryState` — map of entry keys to position/watched state

**Why:** Reuses the existing CAS-based protocol, revision tracking, and
cross-client notifications. No new socket or protocol needed.

**Alternative considered:** Dedicated redb tables with separate access protocol.
Rejected because the existing shared-data infrastructure already handles auth,
CAS, and notifications. Adding document kinds is lower risk than new tables.

### Decision: QueueItem enum wrapping MediaItem and FeedEntry

Introduce a `QueueItem` enum:
```rust
enum QueueItem {
    Emby(MediaItem),
    Feed(FeedEntry),
}
```

Queue, transport, and rendering code operate through trait methods (title,
duration, playback_url, position_key, is_audio) rather than direct field access.

**Why:** Keeps MediaItem unchanged (no Emby regression risk). Lets feed entries
carry only the fields they need. Queue serialization tags each variant.

**Alternative considered:** Add optional feed fields to MediaItem. Rejected
because it conflates two distinct sources and makes is_audio/is_video logic
messier.

### Decision: FeedEntry carries serialized playback data

A `FeedEntry` in the queue carries: entry_key, title, enclosure_url, duration,
feed_url, feed_kind. This is enough to render and play without re-fetching.

**Why:** Queue restore must not require network. Entry metadata may have changed
or the feed may be unreachable.

### Decision: Ctrl protocol capability for feed queue items

Add a capability string (e.g., `queue-feed-items-v1`) to signal that a client
or daemon understands FeedEntry in queue payloads. Old clients that don't
advertise the capability receive queues with feed items omitted or downgraded.

**Why:** Additive capability, not a protocol version bump. Matches existing
ctrl protocol convention.

### Decision: Progress routing by queue item variant

The existing progress-reporting path (started/progress/stopped) checks the
queue item variant. Emby items report to Emby API. Feed items report to the
shared store's FeedEntryState document.

**Why:** Reuses existing lifecycle without forking the player. Routing is the
only change.

### Decision: Feed kind inferred from first fetch

On initial subscription, parse the feed once, inspect enclosure MIME types.
Default to audio if all are audio/*, video otherwise. User can override in the
management panel.

**Why:** Avoids requiring the user to specify. Most feeds are homogeneous.

## Risks / Trade-offs

**Entry identity instability** → Feeds with no guid and changing enclosure URLs
will lose position tracking. Mitigated by the fallback chain (guid > enclosure >
title+date hash). Documented as a known limitation.

**Mixed queue serialization size** → FeedEntry adds more data per item than
MediaItem (which references Emby by ID). Acceptable for typical queue sizes.

**Shared store unavailability** → Feeds are entirely blocked. This is intentional
and documented in the proposal prerequisite. No degraded mode.

**Old client compatibility** → Clients without `queue-feed-items-v1` will see
queues with feed items missing. Acceptable during rollout.
