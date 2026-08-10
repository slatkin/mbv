## MODIFIED Requirements

### Requirement: Subscriptions are stored in local config

Feed subscriptions SHALL be stored in `config.toml`, each carrying a display name, a feed URL, and a kind (audio or video). Subscription configuration SHALL remain local to each machine. Per-entry playback position and played state SHALL be stored separately as roaming feed-entry state and SHALL NOT be written into `config.toml`.

#### Scenario: Subscription persists across restarts

- **WHEN** a subscription is added and the client is restarted
- **THEN** the subscription SHALL still be present, read from `config.toml`

#### Scenario: Playback state does not alter subscription config

- **WHEN** a feed entry gains a playback position or played state
- **THEN** the subscription's `config.toml` entry SHALL remain unchanged

#### Scenario: A second machine has no matching subscription

- **WHEN** roaming entry state exists but the current machine does not configure the matching feed URL
- **THEN** the state SHALL NOT create a subscription or make that feed appear in the Feeds tab

### Requirement: The Feeds tab lists entries grouped by subscription

The Feeds tab SHALL group entries by subscription and SHALL offer an "All" group that lists every entry across subscriptions sorted by publish date descending, with entries lacking a publish date ordered last. After a feed refresh, each parsed entry SHALL be combined with available roaming state for the same authenticated user, feed identity, and entry identity before it appears in its subscription group or the All group.

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

- **WHEN** the shared-data daemon is disconnected, unsupported, or returns a state-read failure during refresh
- **THEN** fetched entries SHALL remain browsable and playable with zero position and unplayed state
- **AND** the Feeds tab SHALL NOT present the entries or feed as unavailable

### Requirement: Feed entries refresh only on explicit user action

Feed entries and their roaming playback state SHALL be refreshed only when the user requests it with the `r` key while the Feeds tab is active. Each successful feed fetch SHALL use one feed-scoped state read rather than one state read per entry. The client SHALL NOT auto-refresh on tab open, on a timer, or when the watched filter changes.

#### Scenario: Manual refresh

- **WHEN** the user presses `r` on the Feeds tab
- **THEN** the subscriptions SHALL be re-fetched and the entry lists updated with currently available roaming state

#### Scenario: State changes on another machine

- **WHEN** a matching feed entry gains a new position or played state on another machine
- **THEN** pressing `r` SHALL make that state visible on the current machine after the shared state write is available

#### Scenario: No automatic refresh

- **WHEN** the Feeds tab is opened or re-opened without pressing `r`
- **THEN** the previously fetched and hydrated entries SHALL be shown without an automatic feed or state read

#### Scenario: Changing the watched filter

- **WHEN** the user changes the watched filter without pressing `r`
- **THEN** the client SHALL filter the state already loaded in the Feeds tab and SHALL NOT perform a state read or write

## ADDED Requirements

### Requirement: The Feeds tab filters entries by played state

While the Feeds tab is active, the unmodified `w` key SHALL cycle the watched filter through All, Watched, and Unwatched in that order. All SHALL show every entry, Watched SHALL show only entries whose loaded played state is true, and Unwatched SHALL show only entries whose loaded played state is false. The active filter SHALL apply to the All group and every subscription group and SHALL be visibly identified.

#### Scenario: Cycle watched filters

- **WHEN** the user repeatedly presses unmodified `w` from the All filter
- **THEN** the active filter SHALL become Watched, then Unwatched, then All

#### Scenario: Watched filter is selected

- **WHEN** the active filter is Watched
- **THEN** only entries with loaded played state set to true SHALL be shown in the selected group

#### Scenario: Unwatched filter is selected

- **WHEN** the active filter is Unwatched
- **THEN** only entries with loaded played state set to false SHALL be shown in the selected group

#### Scenario: Filter changes the displayed selection

- **WHEN** changing the filter removes the currently selected entry from the displayed list
- **THEN** selection and scrolling SHALL reset to the beginning of the filtered list without changing the selected group

#### Scenario: Act on a filtered entry

- **WHEN** the user selects, plays, or enqueues an entry from a filtered list using keyboard or mouse input
- **THEN** the action SHALL target the entry displayed at that filtered position

#### Scenario: Filtering never edits played state

- **WHEN** the user cycles or uses any watched filter
- **THEN** the client SHALL NOT write or otherwise change any entry's played state or position

### Requirement: Feed rows distinguish played entries

The Feeds tab SHALL render a compact played-state indication for entries whose loaded played state is true. The indication SHALL derive only from loaded entry state and SHALL NOT imply that unavailable state was checked successfully.

#### Scenario: Played entry appears in All

- **WHEN** an entry with loaded played state set to true is visible under the All filter
- **THEN** its row SHALL include the played-state indication

#### Scenario: Stateless entry appears

- **WHEN** no state was loaded for an entry
- **THEN** its row SHALL appear unplayed and SHALL NOT show a state-unavailable warning
