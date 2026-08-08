## Purpose

Provide a Feeds tab for browsing subscribed RSS/Atom feeds using the existing feed-view layout structure.

## ADDED Requirements

### Requirement: Feeds tab appears after libraries

When the user has at least one feed subscription and the shared store is connected, a Feeds tab SHALL appear in the tab bar after all library tabs.

#### Scenario: User has subscriptions

- **WHEN** the user has one or more feed subscriptions
- **THEN** a Feeds tab SHALL appear after the library tabs

#### Scenario: No subscriptions

- **WHEN** the user has no feed subscriptions
- **THEN** no Feeds tab SHALL appear

#### Scenario: Subscriptions added mid-session

- **WHEN** the user adds their first feed subscription
- **THEN** the Feeds tab SHALL appear without requiring a restart

### Requirement: Feed selection uses pillbar

The Feeds tab SHALL display a pillbar for selecting which feed to view. The pillbar SHALL contain an "All" option followed by one pill per subscribed feed. Selecting a pill SHALL filter the entry list to that feed's entries.

#### Scenario: All pill selected

- **WHEN** the user selects the All pill
- **THEN** entries from all subscribed feeds SHALL be displayed
- **THEN** entries SHALL be sorted by publication date descending

#### Scenario: Single feed selected

- **WHEN** the user selects a specific feed's pill
- **THEN** only entries from that feed SHALL be displayed

### Requirement: Entry list uses feed-view layout

The entry list SHALL reuse the existing feed-view layout structure: a scrollable list of entries with title, metadata, and watched/unwatched indicator.

#### Scenario: Entry displays metadata from feed

- **WHEN** an entry is rendered
- **THEN** it SHALL display title, duration (if available), and publication date from the feed
- **THEN** it SHALL indicate watched status based on stored entry state

### Requirement: Watched/unwatched toggle via keybinding

The user SHALL be able to toggle an entry's watched status via a keybinding without starting playback.

#### Scenario: Toggle unwatched entry

- **WHEN** the user presses the watched toggle key on an unwatched entry
- **THEN** the entry SHALL be marked watched in the shared store
- **THEN** the UI SHALL update to show watched status

#### Scenario: Toggle watched entry

- **WHEN** the user presses the watched toggle key on a watched entry
- **THEN** the entry SHALL be marked unwatched in the shared store

### Requirement: Feed management via sidebar panel

A sidebar panel SHALL allow users to add, remove, and edit feed subscriptions. Adding a feed SHALL require entering a URL. Editing SHALL allow changing the title override and feed kind.

#### Scenario: Add new feed

- **WHEN** the user enters a valid feed URL in the add panel
- **THEN** the feed SHALL be fetched to infer kind
- **THEN** the subscription SHALL be stored in the shared store

#### Scenario: Remove feed

- **WHEN** the user removes a feed subscription
- **THEN** the subscription and all its entry state SHALL be removed from the shared store

#### Scenario: Edit feed title

- **WHEN** the user edits a feed's title override
- **THEN** the pillbar SHALL display the override instead of the feed's native title

### Requirement: Async refresh on launch and manual trigger

Feeds SHALL refresh asynchronously on app launch and when the user triggers a manual refresh keybinding. A cooldown SHALL prevent redundant fetches within a short window.

#### Scenario: App launch refresh

- **WHEN** the app starts with an active shared-store connection
- **THEN** all subscribed feeds SHALL be fetched in the background
- **THEN** the UI SHALL update as entries arrive

#### Scenario: Manual refresh

- **WHEN** the user presses the refresh keybinding
- **THEN** all feeds SHALL be re-fetched unless within cooldown
- **THEN** new entries SHALL appear in the list

#### Scenario: Refresh during cooldown

- **WHEN** the user triggers refresh within the cooldown window
- **THEN** no fetch SHALL occur
- **THEN** a brief indicator MAY inform the user
