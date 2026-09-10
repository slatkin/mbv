## MODIFIED Requirements

### Requirement: Subscriptions are stored in local config

Feed subscriptions SHALL be stored in `config.toml`, each carrying a display name, a feed URL, and a kind (audio or video). Subscription configuration SHALL remain local to each machine. Per-entry playback position and played state SHALL be stored separately as local feed-entry state and SHALL NOT be written into `config.toml`.

#### Scenario: Subscription persists across restarts

- **WHEN** a subscription is added and the client is restarted
- **THEN** the subscription SHALL still be present, read from `config.toml`

#### Scenario: Playback state does not alter subscription config

- **WHEN** a feed entry gains a playback position or played state
- **THEN** the subscription's `config.toml` entry SHALL remain unchanged

#### Scenario: Playback state is machine-local

- **WHEN** an entry is played and the client is restarted on the same machine
- **THEN** the entry SHALL show its stored playback position and watched state

#### Scenario: No playback state is remembered

- **WHEN** an entry is played on one machine and the client starts on a different machine
- **THEN** that machine SHALL show no remembered playback position or watched state

#### Scenario: A second machine has no matching subscription

- **WHEN** stored entry state exists but the current machine does not configure the matching feed URL
- **THEN** the state SHALL NOT create a subscription or make that feed appear in the Feeds tab

### Requirement: The Feeds tab lists entries grouped by subscription

The Feeds tab SHALL group entries by subscription and SHALL offer an "All" group that lists every entry across subscriptions sorted by publish date descending, with entries lacking a publish date ordered last. After a feed refresh, each parsed entry SHALL be combined with available local state for the same authenticated user, feed identity, and entry identity before it appears in its subscription group or the All group.

#### Scenario: Grouped by subscription

- **WHEN** the Feeds tab is shown with multiple subscriptions
- **THEN** each subscription SHALL be selectable as its own group of entries

#### Scenario: All group sorted by date

- **WHEN** the "All" group is selected
- **THEN** entries SHALL be listed newest first, and entries with no publish date SHALL appear last

#### Scenario: Selecting the Feeds tab does not invoke library behavior

- **WHEN** the Feeds tab is selected
- **THEN** feed entries SHALL be shown, and no Emby library SHALL be fetched or displayed for that tab

#### Scenario: Stored state matches an entry

- **WHEN** refresh returns an entry whose feed and entry identities have stored playback state for the authenticated user
- **THEN** that entry SHALL expose the stored position and played state in both its subscription group and the All group

#### Scenario: Shared entry state is unavailable

- **WHEN** stored entry state cannot be read during refresh
- **THEN** fetched entries SHALL remain browsable and playable with zero position and unplayed state
- **AND** the Feeds tab SHALL NOT present the entries or feed as unavailable

### Requirement: Feed entries refresh only on explicit user action

Feed entries SHALL be fetched and their stored playback state applied only when the user requests it with the `r` key while the Feeds tab is active. Each successful refresh SHALL read stored state once for the whole subscription rather than once per entry. The client SHALL NOT auto-refresh on tab open, on a timer, or when the watched filter changes.

#### Scenario: Manual refresh

- **WHEN** the user presses `r` on the Feeds tab
- **THEN** the subscriptions SHALL be re-fetched and the entry lists updated with currently stored state

#### Scenario: Stored state is applied on refresh

- **WHEN** a refresh returns an entry that has stored playback state
- **THEN** the entry SHALL show that position and played state without a per-entry state read

#### Scenario: State changes on another machine

- **WHEN** a matching feed entry gains a new position or played state on another machine
- **THEN** this machine's stored state SHALL remain unchanged
- **AND** pressing `r` SHALL NOT make the other machine's state visible here

#### Scenario: No automatic refresh

- **WHEN** the Feeds tab is opened or re-opened without pressing `r`
- **THEN** the previously fetched entries SHALL be shown without an automatic feed or state read

#### Scenario: Changing the watched filter

- **WHEN** the user changes the watched filter without pressing `r`
- **THEN** the client SHALL filter the state already loaded in the Feeds tab and SHALL NOT perform a state read or write
