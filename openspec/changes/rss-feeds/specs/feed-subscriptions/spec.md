## Purpose

Defines the RSS/Atom feed feature: subscribing to feeds, browsing their
entries in a dedicated Feeds tab, playing entry media through the same queue
as Emby items, and tracking position and played state in the shared store so
they roam across machines.

## ADDED Requirements

### Requirement: Users can subscribe to and manage RSS/Atom feeds

A user can add, edit, and delete feed subscriptions. A subscription has a
name, a feed URL, a kind (audio or video), and a last-fetched timestamp.
Subscriptions are stored per-user in the daemon-hosted shared store, never in
a config file.

#### Scenario: Adding a feed from its URL

- **WHEN** the user enters a feed URL in the feed-management overlay
- **THEN** mbv fetches and parses the feed
- **THEN** mbv shows the inferred name and kind for confirmation
- **THEN** the user confirms or changes them and the subscription is saved to
  the store

#### Scenario: A feed URL cannot be fetched or parsed

- **WHEN** the add flow's fetch or parse fails
- **THEN** the subscription SHALL NOT be saved
- **THEN** the error SHALL surface through the existing status/notify
  mechanism
- **THEN** no partial subscription SHALL remain in the store

#### Scenario: Editing a subscription

- **WHEN** the user changes a subscription's name or kind
- **THEN** the store SHALL update that subscription
- **WHEN** the user changes a subscription's URL
- **THEN** the store SHALL create a new subscription rather than updating the
  existing one

#### Scenario: Deleting a subscription

- **WHEN** the user deletes a subscription and confirms
- **THEN** the store SHALL remove the subscription and cascade its entry rows
- **THEN** entries already queued from that subscription SHALL remain playable

### Requirement: Feeds poll on startup and manual refresh with a fixed cooldown

Feeds are fetched asynchronously at app startup and on manual refresh. A fixed
30-minute cooldown prevents redundant automatic fetches; manual refresh
ignores it. Fetching and polling happen in the client — the daemon hosts the
store only and never fetches feeds.

#### Scenario: Startup fetch respects the cooldown

- **WHEN** the app starts and a subscription has never been fetched
- **THEN** mbv SHALL fetch it
- **WHEN** the app starts and a subscription was fetched less than 30 minutes
  ago
- **THEN** mbv SHALL NOT fetch it
- **WHEN** the app starts and a subscription was fetched 30 minutes ago or
  more
- **THEN** mbv SHALL fetch it

#### Scenario: Manual refresh ignores the cooldown

- **WHEN** the user presses F5 (the existing global refresh binding)
- **THEN** mbv SHALL fetch every subscribed feed regardless of the cooldown

#### Scenario: A poll fails after cached entries exist

- **WHEN** a refresh fetch or parse fails
- **THEN** cached entries SHALL remain visible
- **THEN** a status message SHALL be shown
- **THEN** the subscription's last-fetched timestamp SHALL be left unchanged
- **THEN** a manual refresh (F5) SHALL retry the fetch immediately, because
  manual refresh ignores the cooldown
- **THEN** the next startup SHALL retry the fetch only once the normal
  cooldown predicate is satisfied (never fetched, or last fetched 30 minutes
  ago or more)

#### Scenario: A feed parses partially

- **WHEN** a fetch returns a feed that parses only in part
- **THEN** mbv SHALL merge what parsed

### Requirement: Feed parsing uses feed-rs and resolves entry identity by guid

Feed parsing uses the feed-rs crate; the hand-rolled parser is removed. Entry
identity is the guid when present, else the enclosure URL hash, else a hash of
title and publication date.

#### Scenario: Two polls of the same feed keep entry identity stable

- **WHEN** a feed is parsed twice and an entry has the same guid
- **THEN** the second parse SHALL update the existing stored entry rather
  than inserting a duplicate

#### Scenario: A publisher regenerates guids

- **WHEN** a feed publishes entries whose guids differ from a previous poll
  for the same content
- **THEN** the new guids SHALL appear as new entries alongside the retained
  old ones
- **THEN** the old entry rows SHALL be retained

### Requirement: Cached entries merge without overwriting position or played state

Cached entries are stored per user, per feed, per entry in a dedicated store
table. A poll merges: new guids insert, existing guids update their parsed
fields (title, URL, and so on) without touching position or played state.

#### Scenario: Merging preserves playback state

- **WHEN** a poll updates an entry that has a stored position and played flag
- **THEN** the position SHALL be unchanged
- **THEN** the played flag SHALL be unchanged

#### Scenario: Entries removed from the feed remain playable

- **WHEN** an entry disappears from a subsequent poll
- **THEN** its stored row SHALL be retained

### Requirement: The queue carries feed entries as snapshots with capability-gated sync

The queue's unit is a tagged item that is either an Emby item or a feed entry.
Renaming the Emby item type is wire-invisible; wrapping items in the tagged
form is a wire and persistence shape change, so legacy bare-Emby-item JSON
still decodes, and feed entries are omitted from queue messages to peers that
do not announce the `queue-feed-items` capability. This keeps the
queue-continuity guarantee of ADR 0015 intact across a format change.

#### Scenario: Persisted queue state from before the tagged shape loads

- **WHEN** queue state persisted in the legacy bare-item JSON form is loaded
- **THEN** each item SHALL decode as the Emby variant
- **WHEN** queue state is saved
- **THEN** it SHALL always be written in the new tagged form

#### Scenario: A peer without the `queue-feed-items` capability

- **WHEN** queue state is sent to a daemon or client that does not announce
  the `queue-feed-items` capability
- **THEN** feed entries SHALL be omitted from the queue message
- **THEN** the remaining queue SHALL be intact

### Requirement: Entry media kind drives admission and classification

Each queue item reports whether it is audio or video. For a feed entry, the
per-entry MIME type decides when present; otherwise the subscription's kind
decides. Audio-only queue owners admit only audio items (ADR 0017).

#### Scenario: An entry with a MIME type

- **WHEN** a feed entry has an audio MIME type
- **THEN** the entry SHALL classify as audio
- **WHEN** a feed entry has a video MIME type
- **THEN** the entry SHALL classify as video

#### Scenario: An entry without a MIME type

- **WHEN** a feed entry has no MIME type
- **THEN** the entry SHALL classify by its subscription's kind

#### Scenario: A kind override takes effect without editing the queue

- **WHEN** the user changes a subscription's kind
- **THEN** entries without a MIME type SHALL reclassify under the new kind
- **THEN** already-queued entries SHALL be re-validated at play time

#### Scenario: A non-audio item reaches an audio-only owner

- **WHEN** a feed entry classified as video is submitted to an audio-only
  queue owner
- **THEN** the owner SHALL NOT admit it, per ADR 0017

### Requirement: Feed entries resolve their playback URL at the play boundary

The play path resolves the media URL from the entry: the enclosure URL when
present, else the entry link. Emby items keep their existing API-client URL
path. The player is unchanged. No artwork is fetched or stored for feed
entries.

#### Scenario: Playing a feed entry with an enclosure

- **WHEN** a feed entry with an enclosure URL is played
- **THEN** the enclosure URL SHALL be the playback source

#### Scenario: Playing a feed entry without an enclosure

- **WHEN** a feed entry with no enclosure URL is played
- **THEN** its link SHALL be the playback source
- **THEN** mpv SHALL handle the URL, delegating to yt-dlp for YouTube links

#### Scenario: Feed artwork is not extracted

- **WHEN** a feed entry is queued or displayed
- **THEN** no artwork SHALL be fetched or stored for it

### Requirement: Feed positions and durations use Emby ticks

All feed durations and playback positions are stored in Emby ticks (10^7
ticks per second). The parser converts seconds or HH:MM:SS to ticks at parse
time.

#### Scenario: A duration expressed in seconds is stored as ticks

- **WHEN** a feed entry's duration is parsed from seconds or HH:MM:SS
- **THEN** the stored duration SHALL be in ticks

### Requirement: Feed entries resume and complete like Emby items

Feed entries honor the same resume threshold and completion rule as Emby
playback. Position keys are stable within a subscription, and position writes
land only on rows that still exist in the store.

#### Scenario: Resuming a feed entry

- **WHEN** a feed entry has saved progress of at least 6 percent of a known
  runtime
- **THEN** playback SHALL resume from the saved position
- **WHEN** a feed entry has positive saved progress and unknown runtime
- **THEN** playback SHALL resume from the saved position
- **WHEN** a feed entry has saved progress below 6 percent of a known runtime
- **THEN** playback SHALL start from the beginning

#### Scenario: Marking a feed entry played

- **WHEN** a feed entry plays to EOF with a known runtime
- **THEN** the entry SHALL be marked played
- **WHEN** a feed entry stops at or past 95 percent of a known runtime
- **THEN** the entry SHALL be marked played

#### Scenario: Position write-through for a deleted subscription

- **WHEN** a queued feed entry reports progress but its subscription row no
  longer exists
- **THEN** the position write SHALL be dropped silently

### Requirement: The Feeds tab shows subscriptions and filters watched state

The Feeds tab is the last tab, visible only when at least one subscription
exists. It groups entries per feed with an "All" pill across feeds, sorted by
publication date descending; entries without a parseable date sort last. The
`w` key toggles a watched/unwatched filter, scoped to the Feeds tab; it is a
filter, not a mark-played action.

#### Scenario: Tab visibility

- **WHEN** no subscriptions exist
- **THEN** the Feeds tab SHALL NOT appear
- **WHEN** at least one subscription exists
- **THEN** the Feeds tab SHALL appear as the last tab

#### Scenario: The watched/unwatched filter

- **WHEN** the user presses `w` on the Feeds tab
- **THEN** the view SHALL switch between all, watched, and unwatched entries
- **THEN** no entry's played state SHALL be changed by the toggle

### Requirement: Feed management is an overlay opened from the Feeds tab

The feed-management overlay opens from the Feeds tab via `s`. Inside it, `a`
adds, `e` edits, and `d` deletes subscriptions.

#### Scenario: Opening the overlay

- **WHEN** the user presses `s` on the Feeds tab
- **THEN** the feed-management overlay SHALL open over the current view

### Requirement: The Feeds tab requires a reachable daemon

Feed state is read from the daemon-hosted shared store. When the daemon is
unreachable, no subscriptions are readable and the Feeds tab is hidden.

#### Scenario: Daemon unreachable

- **WHEN** the daemon is unreachable
- **THEN** the Feeds tab SHALL be hidden
- **THEN** no unavailable state SHALL be shown in its place
