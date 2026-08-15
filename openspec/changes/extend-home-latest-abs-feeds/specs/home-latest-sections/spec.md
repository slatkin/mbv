## Purpose

Defines Home's per-destination "Latest" pills: one pill per visible Emby library and per visible Audiobookshelf podcast library, plus a single flattened Feeds pill, populated independently of which Services are configured, and how the user selects, hides, and plays/enqueues their items.

## ADDED Requirements

### Requirement: Latest pills cover Emby, Audiobookshelf podcast libraries, and Feeds

Home SHALL show one Latest pill per visible Emby library (existing behavior), one Latest pill per visible Audiobookshelf podcast library, and exactly one "Latest Feeds" pill combining every feed subscription. Home SHALL NOT show a Latest pill for an Audiobookshelf book library or a per-subscription Feeds pill.

#### Scenario: Home lists pills for a mixed server

- **WHEN** the user has visible Emby libraries, at least one Audiobookshelf podcast library, an Audiobookshelf book library, and one or more feed subscriptions
- **THEN** Home SHALL display a Latest pill for each Emby library, a Latest pill for the podcast library, and a single "Latest Feeds" pill
- **THEN** Home SHALL NOT display a pill for the Audiobookshelf book library or a separate pill per feed subscription

#### Scenario: Audiobookshelf podcast library has no newest-episodes data

- **WHEN** an Audiobookshelf podcast library's server response has no `Newest Episodes` shelf or an empty one
- **THEN** Home SHALL still display that library's Latest pill, with no selectable items (an `(empty)` section), matching the Continue Watching convention that a pill renders even when its section is bare

#### Scenario: Every Latest pill renders even when empty

- **WHEN** a section in `home.latest` (an Emby view, an Audiobookshelf podcast library, or the Feeds pill) has zero items
- **THEN** Home SHALL still display its pill and render an `(empty)` section rather than hiding the pill

### Requirement: Latest pills populate and refresh independently of Emby's connection state

Home's Audiobookshelf and Feeds Latest pills SHALL populate, refresh, and remain hideable via `hidden_latest` whether or not an Emby Service is configured, connecting, or reachable. A refresh of Home SHALL NOT fail, and SHALL NOT skip updating the Audiobookshelf or Feeds pills, solely because no Emby Service is configured or connected. Continue Watching MAY remain empty when no Emby Service is configured, since it stays Emby-only.

#### Scenario: Home refreshes with no Emby Service configured

- **WHEN** the user has an Audiobookshelf podcast library and feed subscriptions, and no Emby Service configured, and refreshes Home
- **THEN** Home SHALL display the Audiobookshelf and Feeds Latest pills with current data
- **THEN** the refresh SHALL NOT produce an Emby-related error
- **THEN** Continue Watching MAY remain empty

#### Scenario: Hiding an Audiobookshelf or Feeds pill with no Emby Service configured

- **WHEN** the user has no Emby Service configured and changes `hidden_latest` to hide an Audiobookshelf library's pill or the Feeds pill
- **THEN** Home SHALL stop displaying that pill

#### Scenario: Emby finishes connecting after other Latest pills are populated

- **WHEN** Home already displays Audiobookshelf and Feeds Latest pills and Emby then finishes its independent startup connection
- **THEN** Home SHALL add Continue Watching and Emby Latest pills
- **THEN** the existing Audiobookshelf and Feeds Latest pills SHALL remain displayed with their data intact

### Requirement: Latest Feeds pill reflects the Feeds tab's combined, newest-first entries

The "Latest Feeds" pill's items SHALL be the same entries, in the same newest-first order, as the Feeds tab's combined "All" group, independent of any per-subscription grouping or watched-state filter currently active on the Feeds tab.

#### Scenario: Feeds tab filter does not affect the Home pill

- **WHEN** the Feeds tab's watched-state filter is set to a value other than "All"
- **THEN** the "Latest Feeds" pill on Home SHALL still show entries regardless of played state

### Requirement: `hidden_latest` hides pills by name across providers

`hidden_latest` SHALL hide a Latest pill whose Emby or Audiobookshelf library name (case-insensitive) is listed, using the same settings mechanism as today's Emby-only hiding. `hidden_latest` SHALL also hide the "Latest Feeds" pill when it contains the literal value `"feeds"` (case-insensitive).

#### Scenario: Hiding an Audiobookshelf library's Latest pill

- **WHEN** an Audiobookshelf podcast library's name (lowercased) is present in `hidden_latest`
- **THEN** Home SHALL NOT display that library's Latest pill

#### Scenario: Hiding the Feeds pill

- **WHEN** `hidden_latest` contains `"feeds"`
- **THEN** Home SHALL NOT display the "Latest Feeds" pill

### Requirement: Selecting and playing a Latest item works uniformly by item provider

The user SHALL be able to select any item in any visible Latest pill using the existing Home cursor/section navigation, and play or enqueue it. Playing or enqueueing an Audiobookshelf or Feed item from Home SHALL use that item's own provider identity and SHALL NOT read, depend on, or mutate the Audiobookshelf or Feeds tab's own cursor, selected group, or active filter.

#### Scenario: Playing an Audiobookshelf episode from Home

- **WHEN** the user plays an item from an Audiobookshelf Latest pill
- **THEN** mbv SHALL queue and play that Audiobookshelf episode
- **THEN** the Audiobookshelf tab's own episode selection and filter SHALL remain unchanged

#### Scenario: Playing a feed entry from Home whose Feeds-tab filter would hide it

- **WHEN** the user plays an item from the "Latest Feeds" pill that is marked played, while the Feeds tab's watched-state filter is set to "Unplayed"
- **THEN** mbv SHALL queue and play that feed entry
- **THEN** the Feeds tab's selected group and filter SHALL remain unchanged

#### Scenario: Playing an Emby item from Home is unchanged

- **WHEN** the user plays an item from an Emby Latest pill or Continue Watching
- **THEN** mbv SHALL use the existing Emby Home play routing unchanged

### Requirement: Selected non-Emby Latest item shows a generic detail view

When the selected Home item is from an Audiobookshelf or Feed Latest pill, Home SHALL display its title, duration (if known), and cover art (if available), without depending on Emby-specific browse/navigation state.

#### Scenario: Selecting a Feed entry with no known duration

- **WHEN** the selected Home item is a feed entry with no duration
- **THEN** Home SHALL display its title without an error or a placeholder duration value implying a known length
