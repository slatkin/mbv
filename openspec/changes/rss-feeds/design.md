# RSS Feeds — Design

## Data model

### QueueItem enum

```
enum QueueItem {
    Emby(EmbyItem),     // renamed from MediaItem
    Feed(FeedEntry),
}
```

Shared accessors on `QueueItem`:

```
fn title(&self) -> &str
fn duration(&self) -> Option<u64>        // ticks
fn media_kind(&self) -> MediaKind        // Audio | Video
fn position_key(&self) -> String         // stable identity for position lookup
fn artwork_url(&self) -> Option<String>
```

The queue, player transport, rendering, and ctrl protocol work through these
accessors. Variant-specific code (Emby session reporting, redb position
writes, playback URL resolution) is behind match arms only at the boundaries
that genuinely differ.

`playback_url()` is deliberately **not** a shared accessor: URL resolution is
variant-specific and lives at the play boundary, not in the queue model. The
Emby variant keeps the existing async API-client path, unchanged; the Feed
variant resolves synchronously from the entry (`enclosure_url`, else `link`).
The player itself is unchanged.

Feed artwork is out of scope: the Feed variant of `artwork_url()` returns
`None`. The accessor's presence is not an extraction requirement — no artwork
is fetched or stored for feed entries.

### FeedEntry

```
struct FeedEntry {
    feed_id: String,           // which feed subscription this belongs to
    guid: String,              // stable identity (guid > enclosure URL hash > title+pub_date hash)
    title: String,
    enclosure_url: Option<String>,
    link: Option<String>,
    pub_date: Option<String>,  // stored raw as parsed; missing/unparseable sorts last
    duration_ticks: Option<u64>,
    description: Option<String>,
    mime_type: Option<String>,
    // Both are per-user state: maintained by the progress-reporting path and
    // preserved by entry merges.
    position_ticks: u64,       // default 0
    played: bool,              // default false
}
```

Feed durations and positions use Emby ticks (10^7 ticks/second — the same
`TICKS_PER_SECOND` the player already works in). The parser converts seconds
or HH:MM:SS to ticks at parse time; no conversion happens later.

### Feed subscription

```
struct FeedSubscription {
    id: String,                // generated UUID
    name: String,              // display name (defaulted from feed title)
    url: String,               // RSS/Atom URL
    kind: FeedKind,            // Audio | Video
    last_fetched: Option<u64>, // unix timestamp
}
```

`FeedKind` is `Audio | Video`. `media_kind()` on the Feed variant uses the
per-entry `mime_type` when present, else the subscription's kind. A
subscription-kind override therefore reclassifies entries lacking a MIME
type, and already-queued entries are re-validated at play time, so an
override takes effect without editing the queue. Audio-only queue owners
reject non-audio QueueItems (ADR 0017); the admission mechanics themselves
are governed by the sibling changes `audio-only-mixed-queue-admission` and
`audio-only-owner-fall-through`.

## Wire compatibility

Renaming `MediaItem` → `EmbyItem` is wire-invisible: serde field names are
unchanged, so persisted queue state and queue documents decode unchanged.

Wrapping items in the `QueueItem` enum **is** a wire/persistence shape change
(a bare object becomes a tagged object), handled as follows:

- **Deserialization is backward-compatible.** Legacy bare-`MediaItem` JSON in
  persisted queue state (`queue_state.json`, shared queue documents) decodes
  as the Emby variant. Serialization always writes the new tagged shape.
- **Feed variants in queue sync are capability-gated.** A second capability
  string (`CTRL_CAP_QUEUE_FEED_ITEMS`, `"queue-feed-items"`) gates feed items
  in queue messages. Feed variants are omitted from queue messages to peers
  (daemon or client) that do not announce it. Under version skew, feed
  entries simply do not roam to old peers; the rest of the queue stays
  intact.

This honors ADR 0015's queue-continuity guarantee: what is playing, the
queue, and position survive a client closing — and a mixed-version pair never
loses the queue's Emby items over a feed-item format change.

## Feed storage service API

Feed data lives in two dedicated redb tables hosted by the daemon
(`feed_subscriptions`, `feed_entries`). Clients never touch redb directly;
all access goes through new feed operations on the existing shared-service
channel, mirroring the `SharedDataCmd` / `SharedStoreRequest` style
(request/reply over the channel, handled by the storage worker).

The operations, behaviorally:

- `list_subscriptions` — all subscriptions for the calling user.
- `add_subscription` — create a subscription (name defaulted from the parsed
  feed title, kind from the add flow).
- `update_subscription` — change name and/or kind only. A URL change creates
  a new subscription rather than updating this one.
- `delete_subscription` — remove the subscription **and** cascade its entry
  rows.
- `list_entries(feed)` — the cached entries of one feed.
- `merge_entries` — upsert by guid: new guids insert; existing guids update
  title/URL/etc. without touching `position_ticks`/`played`.
- `update_position` — write a playback position for one entry.
- `set_played` — set `played = true` for one entry. Invoked solely by the
  playback completion path (EOF with known runtime, or stop at ≥ 95% of a
  known runtime). No operation marks an entry unplayed.

Conflict policy: entry-row writes (position, played) are last-write-wins.
Feed rows deliberately have **no CAS** — unlike `shared_documents`, feed rows
carry no revision and no expected-revision check.

Additive change to the shared service: a new capability string
(`SHARED_DATA_CAP_FEEDS_V1`, `"shared-mbv-state-feeds-v1"`) advertised in the
existing shared-data handshake, per the convention in the ctrl protocol above
`CTRL_PROTOCOL_VERSION`: additive changes get a capability string, not a
version bump.

The daemon hosts the store only — it never fetches feeds. Feed fetching and
polling are client-side. Because the store is per-user in the shared daemon
database, the proposal's "roams across machines" promise holds: feed
definitions and entry state (position, played) follow the user exactly like
the existing shared documents.

## Redb schema

Two new tables, separate from the existing `shared_documents` table:

### `feed_subscriptions` table

Key: `(user_id: &str, feed_id: &str)`
Value: JSON-serialized `FeedSubscription`

### `feed_entries` table

Key: `(user_id: &str, feed_id: &str, entry_guid: &str)`
Value: JSON-serialized entry record:

```json
{
    "guid": "...",
    "title": "...",
    "enclosure_url": "...",
    "link": "...",
    "pub_date": "...",
    "duration_ticks": null,
    "description": "...",
    "mime_type": "audio/mpeg",
    "position_ticks": 0,
    "played": false
}
```

Position and played state are per-entry, per-user. Updated by the player's
progress-reporting path (position) and on playback completion (played).

Retention: entries removed from the feed are retained (the user may still
resume them). If a publisher regenerates guids, the new guids appear as new
entries alongside the retained old ones — an accepted duplicate risk, stated
here rather than silently hidden.

## Feed parsing

`feed-rs` is added to the **binary** crate's `Cargo.toml` (repo root) —
parsing and polling live in `src/`, not in mbv-core. The hand-rolled parser
in `src/feed_parse.rs` is deleted; `feed_parse.rs` is rewritten as a thin
wrapper over `feed-rs`, and the existing idle feed migrates to it (its current
need is feed title plus entry title/link only).

The parser's result type carries the feed-level title (needed for the add
flow's default subscription name) plus, per entry: guid, title,
enclosure_url, link, pub_date, duration (converted to ticks at parse time),
description, and mime_type.

### Entry identity resolution

```
fn entry_id(entry) -> String:
    if entry.guid is present and non-empty:
        return guid
    if entry.enclosure_url is present:
        return sha256(enclosure_url)
    return sha256(title + pub_date)
```

### Kind inference

On first fetch of a new subscription:
1. Scan enclosure MIME types across entries.
2. If all are `audio/*` → Audio.
3. If all are `video/*` → Video.
4. Mixed or absent → default to Video (YouTube has no enclosures).
5. The user can override via the management panel; per-entry MIME types
   continue to take precedence over the subscription kind at play time.

## Polling flow

Feeds refresh at app startup and on manual refresh (F5, the existing global
refresh binding). A fixed 30-minute cooldown — a named constant, not
configuration — prevents redundant automatic fetches. Manual refresh ignores
the cooldown.

```
App startup
  ├── read all FeedSubscriptions from the store
  ├── for each: spawn async fetch if last_fetched is None
  │             OR now >= last_fetched + COOLDOWN (30 minutes)
  │     ├── parse feed with feed-rs
  │     ├── merge entries into the store (insert new, update existing, keep state)
  │     └── update last_fetched timestamp
  └── Feeds tab renders from the store cache immediately

Manual refresh (F5)
  └── same as above, ignoring cooldown
```

A feed that parses partially merges what parsed. A failed fetch/parse leaves
cached entries visible, shows a status message through the existing
status/notify mechanism (`src/app/notify_actions.rs`), and leaves
`last_fetched` unchanged. A manual refresh (F5) retries immediately — it
ignores the cooldown; the next startup retries only when the normal predicate
(`last_fetched` is None OR `now >= last_fetched + COOLDOWN`) is satisfied.

## Player integration

### Playback URL resolution

URL resolution is variant-specific and happens at the play boundary, not in
the queue model:

- Emby variant: get the streaming URL from the Emby API — the existing
  async path, unchanged.
- Feed variant: `enclosure_url`, else `link` — resolved synchronously from
  the entry at play time.

mpv handles direct URLs natively. For YouTube links, mpv delegates to yt-dlp.

### Progress reporting

The player already calls progress-report hooks at regular intervals and on
stop. The existing path:

```
player reports progress → Emby API (PlayingProgress, PlayingStopped)
```

New path for feed entries:

```
player reports progress → store (update position_ticks on feed_entries row)
player reports stopped  → store (update position_ticks, set played if finished)
```

The branch point is inside the progress reporter, keyed on the `QueueItem`
variant. The player itself doesn't change.

The queue stores an owned snapshot of the `FeedEntry`. Position writes on
queued entries write through to the store only if the `(user, feed_id, guid)`
row still exists; otherwise the write is dropped silently. Deleting a
subscription removes its rows but leaves already-queued snapshots playable.

### Resume

On play of a feed entry:
1. Read `position_ticks` from the store via the entry's `position_key()`.
2. Apply the same resume threshold as Emby playback (per the sibling change
   `raise-playback-resume-threshold`): a saved position counts as resumable
   only when it is at least 6% of a known runtime; a positive saved position
   with unknown runtime stays resumable.
3. If resumable, pass `--start = position_ticks / 10^7` seconds to mpv — the
   same `--start` conversion the player already applies to Emby items
   (`resume_seconds()`).

### Completion

Feed entries use the same completion rule as Emby items today
(`player_run_events.rs`; `is_near_end` in `player_proxy.rs`): `played` is set
when playback reaches EOF (mpv EndFile with Eof reason and known runtime), or
when playback stops with the final position at or past 95% of the known
runtime (the existing 19/20 integer check). Manual mark-played/unplayed is
explicitly out of scope — the watched/unwatched keybinding on the Feeds tab
is a filter toggle only, never a state edit.

### Position keys

`position_key()` uses a namespaced format, collision-free across variants:

```
emby:<item_id>
feed:<feed_id>:<guid>
```

Feed URL changes create a new subscription (see the storage service API), so
keys stay stable within a subscription. If a feed's identity fallback shifts
(a guid appears where none existed before), the entry gets a new key and the
old entry row is retained per the retention rule.

## UI integration

### Feeds tab

Position: last tab in the tab bar, after all Emby libraries.
Visibility: only when at least one `FeedSubscription` exists in the store.
When the daemon is unreachable, no subscriptions are readable, so the
"visible only when subscriptions exist" rule naturally yields a hidden tab —
there is no "unavailable state" alternative.

The feed-view data model (`FeedHomeVideoState` in `src/app/types_feed.rs`) is
MediaItem-typed today; this change **generalizes** the data model and the
renderer (`src/app/render/home_feed.rs`) to work over QueueItem-backed view
data rather than reusing the Emby-shaped types as-is.

- Groups = feed subscriptions (like folder groups today).
- "All" pill = entries from all feeds, sorted by `pub_date` descending;
  entries with missing or unparseable dates sort last.
- Per-group = entries from one feed, sorted by `pub_date` descending.
- Watched/unwatched filter toggle via `w` (scoped to the Feeds tab; filters
  on the `played` field). `w` is free today — the library and Home scopes
  only bind `Ctrl+w`, and no playback key uses `w`.

### Feed management overlay

The feed-management panel is a conditional overlay following the
sessions-panel pattern (`src/app/render/overlays/sessions.rs`), not a
persistent queue-panel panel. It opens from the Feeds tab via `s`. (The
suggested `m` is taken: it is the global mute binding, which fires
unconditionally in the playback context before view dispatch.) Because the
overlay captures input, its interior keys are safe:

- `a` add — enter URL → fetch → show inferred name and kind → confirm. A
  failed fetch/parse means the subscription **cannot** be saved; the error
  surfaces via the existing status/notify mechanism.
- `e` edit — change name or kind. URL changes create a new subscription.
- `d` delete — confirm, then remove the subscription and all its entry state
  from the store (cascade).
- Manual refresh uses F5, the existing global refresh binding.

## File impact

### mbv-core (crates/mbv-core/)
- New `feed_types.rs`: `FeedSubscription`, `FeedEntry`, `FeedKind`.
- New `feed_store.rs`: redb table definitions, CRUD for subscriptions and
  entries (the storage service API).
- `shared_protocol.rs` / `shared_service.rs` / `shared_store.rs`: new feed
  commands/events on the shared-service channel plus the capability string.
- `shared_worker.rs`: expose the feed tables alongside the existing shared DB.
- Rename `MediaItem` → `EmbyItem` in `api_types.rs` and all call sites.
- New `queue_item.rs`: `QueueItem` enum with accessors.
- Player progress reporting: branch on the `QueueItem` variant.
- **No `feed-rs` here** — parsing is a binary-crate concern.

### mbv (src/)
- `Cargo.toml` (repo root): add the `feed-rs` dependency.
- `feed_parse.rs`: rewritten as a thin wrapper over `feed-rs` (the hand-rolled
  parser is deleted); the idle feed migrates to it.
- `feeds_actions.rs`: **new file** — subscription polling, merge
  orchestration, and Feeds-tab data loading. `feed_actions.rs` stays scoped to
  the idle feed (already 675 lines; the 800-line cap is what drives the
  split).
- `types_feed.rs`: generalize `FeedHomeVideoState` off `MediaItem`.
- New overlay files for feed management (sessions-panel pattern).
- Queue rendering: work through `QueueItem` accessors.
- Library tab bar: append the Feeds tab conditionally.
