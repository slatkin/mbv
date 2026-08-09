## Purpose

The user can subscribe to RSS/podcast/video feeds in local configuration, browse their entries in a dedicated tab, and play any entry through the playback queue — without the client remembering playback position or watched state.

## ADDED Requirements

### Requirement: Subscriptions are stored in local config

Feed subscriptions SHALL be stored in `config.toml`, each carrying a display name, a feed URL, and a kind (audio or video). The subscription list SHALL be the only persisted feed data; the client SHALL NOT persist per-entry playback state.

#### Scenario: Subscription persists across restarts

- **WHEN** a subscription is added and the client is restarted
- **THEN** the subscription SHALL still be present, read from `config.toml`

#### Scenario: No playback state is remembered

- **WHEN** an entry is played and the client is restarted
- **THEN** the entry SHALL show no remembered playback position or watched state

### Requirement: The Feeds tab appears only when a subscription exists

A Feeds tab SHALL be presented as the last tab. It SHALL be visible when at least one subscription exists and hidden when none do.

#### Scenario: Tab hidden with no subscriptions

- **WHEN** there are no subscriptions in config
- **THEN** the Feeds tab SHALL NOT be shown

#### Scenario: Tab shown with a subscription

- **WHEN** at least one subscription exists in config
- **THEN** the Feeds tab SHALL be shown as the last tab

### Requirement: The Feeds tab lists entries grouped by subscription

The Feeds tab SHALL group entries by subscription and SHALL offer an "All" group that lists every entry across subscriptions sorted by publish date descending, with entries lacking a publish date ordered last.

#### Scenario: Grouped by subscription

- **WHEN** the Feeds tab is shown with multiple subscriptions
- **THEN** each subscription SHALL be selectable as its own group of entries

#### Scenario: All group sorted by date

- **WHEN** the "All" group is selected
- **THEN** entries SHALL be listed newest first, and entries with no publish date SHALL appear last

#### Scenario: Selecting the Feeds tab does not invoke library behavior

- **WHEN** the Feeds tab is selected
- **THEN** feed entries SHALL be shown, and no Emby library SHALL be fetched or displayed for that tab

### Requirement: Feed entries refresh only on explicit user action

Feed entries SHALL be refreshed (re-fetched and re-parsed) only when the user requests it with the `r` key while the Feeds tab is active. The client SHALL NOT auto-refresh on tab open or on a timer.

#### Scenario: Manual refresh

- **WHEN** the user presses `r` on the Feeds tab
- **THEN** the subscriptions SHALL be re-fetched and the entry lists updated

#### Scenario: No automatic refresh

- **WHEN** the Feeds tab is opened or re-opened without pressing `r`
- **THEN** the previously fetched entries SHALL be shown without an automatic re-fetch

### Requirement: Playing an entry enqueues and plays it

Selecting play on a feed entry SHALL add it to the playback queue as a feed item and begin playback. The entry's enclosure URL SHALL be used as the playable source, or its link when there is no enclosure.

#### Scenario: Play an entry with an enclosure

- **WHEN** the user plays an entry that has an enclosure URL
- **THEN** the entry SHALL be added to the queue and playback SHALL start from the enclosure URL

#### Scenario: Play an entry without an enclosure

- **WHEN** the user plays an entry that has no enclosure URL but has a link
- **THEN** the entry SHALL be added to the queue and playback SHALL start from the link

### Requirement: Subscriptions are managed through an overlay

The client SHALL provide an overlay to add, remove, and edit subscriptions, writing changes to `config.toml`. Adding a subscription SHALL fetch and parse the feed first; if that fails, the failure SHALL be surfaced to the user and the subscription SHALL NOT be saved.

#### Scenario: Add a valid feed

- **WHEN** the user adds a subscription whose feed fetches and parses successfully
- **THEN** the subscription SHALL be written to `config.toml` and appear in the Feeds tab

#### Scenario: Add an invalid feed

- **WHEN** the user adds a subscription whose feed fails to fetch or parse
- **THEN** the failure SHALL be surfaced via the status/notification path and the subscription SHALL NOT be saved

#### Scenario: Remove a subscription

- **WHEN** the user removes a subscription
- **THEN** it SHALL be deleted from `config.toml` and no longer appear in the Feeds tab
