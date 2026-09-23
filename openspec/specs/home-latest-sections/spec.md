# home-latest-sections Specification

## Purpose

Defines Home's per-destination "Latest" pills: one pill per visible Emby library and per visible Audiobookshelf podcast library, plus a single flattened Feeds pill, populated independently of which Services are configured, and how the user selects, hides, and plays/enqueues their items.

## Requirements

### Requirement: Latest pills cover Emby, Audiobookshelf podcast libraries, and Feeds

Home SHALL show one Latest pill per visible Emby library (existing behavior), one Latest pill per visible Audiobookshelf podcast library, and exactly one "Feeds" pill combining every feed subscription. Home SHALL NOT show a Latest pill for an Audiobookshelf book library or a per-subscription Feeds pill.

#### Scenario: Home lists pills for a mixed server

- **WHEN** the user has visible Emby libraries, at least one Audiobookshelf podcast library, an Audiobookshelf book library, and one or more feed subscriptions
- **THEN** Home SHALL display a Latest pill for each Emby library, a Latest pill for the Audiobookshelf podcast library, and a single "Feeds" pill
- **THEN** Home SHALL NOT display a pill for the Audiobookshelf book library or a separate pill per feed subscription

#### Scenario: Audiobookshelf podcast library has no newest-episodes data

- **WHEN** an Audiobookshelf podcast library's server response has no `Newest Episodes` shelf or an empty one
- **THEN** Home SHALL still display that library's Latest pill, with no selectable items (an `(empty)` section), matching the Continue Watching convention that a pill renders even when its section is bare

#### Scenario: Every Latest pill renders even when empty

- **WHEN** a section in `home.latest` (an Emby view, an Audiobookshelf podcast library, or the Feeds pill) has zero items
- **THEN** Home SHALL still display its pill and render an `(empty)` section rather than hiding the pill

#### Scenario: Latest pills keep a canonical provider ordering

- **WHEN** Emby libraries, Audiobookshelf podcast libraries, and the Feeds pill are all present, regardless of which provider populated first
- **THEN** Home SHALL display the Emby library pills, then the Audiobookshelf podcast library pills, then the "Feeds" pill, in that canonical order

#### Scenario: A long Audiobookshelf description is truncated

- **WHEN** a selected Audiobookshelf episode's description exceeds 200 display columns
- **THEN** Home SHALL display it truncated to 200 columns ending in an ellipsis rather than growing the hero unboundedly

### Requirement: The last-selected Latest pill is restored on launch

When the user selects a Latest pill and then restarts the app, Home SHALL restore that same pill once it is available, matching the selection by the pill's underlying provider identity rather than its position.

#### Scenario: Restoring an Audiobookshelf pill after restart

- **WHEN** the user selects an Audiobookshelf podcast library's Latest pill, quits, and relaunches, and that library is still available
- **THEN** Home SHALL select that same Audiobookshelf pill (not Continue Watching, and not a differently-positioned pill) once the library's section has populated
- **THEN** if that library is no longer available, Home SHALL remain on Continue Watching

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

### Requirement: Feeds pill reflects the Feeds tab's combined, newest-first entries

The "Feeds" pill's items SHALL be the same entries, in the same newest-first order, as the Feeds tab's combined "All" group, independent of any per-subscription grouping or watched-state filter currently active on the Feeds tab.

#### Scenario: Feeds tab filter does not affect the Home pill

- **WHEN** the Feeds tab's watched-state filter is set to a value other than "All"
- **THEN** the "Feeds" pill on Home SHALL still show entries regardless of played state

### Requirement: `hidden_latest` hides pills by name across providers

`hidden_latest` SHALL hide a Latest pill whose Emby or Audiobookshelf library name (case-insensitive) is listed, using the same settings mechanism as today's Emby-only hiding. `hidden_latest` SHALL also hide the "Feeds" pill when it contains the literal value `"feeds"` (case-insensitive).

#### Scenario: Hiding an Audiobookshelf library's Latest pill

- **WHEN** an Audiobookshelf podcast library's name (lowercased) is present in `hidden_latest`
- **THEN** Home SHALL NOT display that library's Latest pill

#### Scenario: Hiding the Feeds pill

- **WHEN** `hidden_latest` contains `"feeds"`
- **THEN** Home SHALL NOT display the "Feeds" pill

### Requirement: Selecting and playing a Latest item works uniformly by item provider

The user SHALL be able to select any item in any visible Latest pill using the existing Home cursor/section navigation, and play or enqueue it. Playing or enqueueing an Audiobookshelf or Feed item from Home SHALL use that item's own provider identity and SHALL NOT read, depend on, or mutate the Audiobookshelf or Feeds tab's own cursor, selected group, or active filter.

#### Scenario: Playing an Audiobookshelf episode from Home

- **WHEN** the user plays an item from an Audiobookshelf Latest pill
- **THEN** mbv SHALL queue and play that Audiobookshelf episode
- **THEN** the Audiobookshelf tab's own episode selection and filter SHALL remain unchanged

#### Scenario: Playing a feed entry from Home whose Feeds-tab filter would hide it

- **WHEN** the user plays an item from the "Feeds" pill that is marked played, while the Feeds tab's watched-state filter is set to "Unplayed"
- **THEN** mbv SHALL queue and play that feed entry
- **THEN** the Feeds tab's selected group and filter SHALL remain unchanged

#### Scenario: Playing an Emby item from Home is unchanged

- **WHEN** the user plays an item from an Emby Latest pill or Continue Watching
- **THEN** mbv SHALL use the existing Emby Home play routing unchanged

### Requirement: Selected non-Emby Latest item shows a hero detail matching Emby's structure

When the selected Home item is from an Audiobookshelf or Feed Latest pill, Home SHALL display a hero detail that follows the same structure as the Emby Keep Watching hero: yellow bold title, show name (for Audiobookshelf episodes), duration when known, and an overview when the item carries one, without depending on Emby-specific browse/navigation state.

#### Scenario: Selecting an Audiobookshelf episode with a description

- **WHEN** the selected Home item is an Audiobookshelf episode whose catalog response carries a `recentEpisode.description`
- **THEN** Home SHALL display the episode title, its show name, its duration, and the episode description
- **THEN** the description SHALL have its HTML converted to terminal text: paragraph tags as line breaks, decoded entities (e.g. `&amp;`), and links as `text (URL)`

#### Scenario: Selecting a Feed entry with no known duration

- **WHEN** the selected Home item is a feed entry with no duration
- **THEN** Home SHALL display its title without an error or a placeholder duration value implying a known length

### Requirement: Home rows render the container context part

Home's media rows SHALL render their title and container context using the shared playback title-parts mapping (the same mapping the playback panel's now-playing title uses): an Emby episode SHALL show the series name as context, an Emby audio track SHALL show the artist as context, a feed entry SHALL show its subscription's display name as context, and an Audiobookshelf podcast episode SHALL show its show name as context. Movies, Audiobookshelf books, and items whose container name is absent or unresolvable SHALL render the title alone. A feed entry's context SHALL be the display name of the configured subscription matching the entry's recorded feed identity; an entry whose feed identity matches no configured subscription SHALL render the title alone. Container-name resolution SHALL NOT depend on the Feeds tab's or any service tab's current selection, cursor, or filter state.

#### Scenario: Episode row shows series and episode

- **WHEN** a Home row holds an Emby episode with a series name
- **THEN** the row paints the series name as the context part followed by the episode name as the title part

#### Scenario: Audio track row shows artist and track

- **WHEN** a Home row holds an Emby audio track with an artist
- **THEN** the row paints the artist as the context part followed by the track name as the title part

#### Scenario: Feed row shows the subscription

- **WHEN** a Home row holds a feed entry whose feed identity matches a configured subscription
- **THEN** the row paints that subscription's display name as the context part followed by the entry title as the title part

#### Scenario: Feed row degrades without a matching subscription

- **WHEN** a Home row holds a feed entry whose feed identity matches no configured subscription
- **THEN** the row paints the entry title alone, with no context part

#### Scenario: Movies and books render the title alone

- **WHEN** a Home row holds a movie, an Audiobookshelf book, or an item with no container name
- **THEN** the row paints the item's title alone in the ordinary title role

### Requirement: Home Latest pills identify content new since the previous client launch
At client startup, mbv SHALL read the previously recorded client-launch timestamp and immediately replace it with the current launch timestamp. For each Home Latest section, mbv SHALL mark the section when at least one item carries a valid provider timestamp strictly later than the previous launch and no later than the current launch. Emby items SHALL use their provider date-added timestamp; Audiobookshelf podcast episodes and Feed entries SHALL use their provider publication timestamp.

The marker SHALL be one `•` after the relevant pill label, painted in the Iris role. For an Emby library section, the same marker SHALL also appear on that library's own `Latest` content mode, because both surfaces stand for the same section (see `tv-library-content-modes`). Continue SHALL never receive the marker. A missing, invalid, equal-to-cutoff, or future provider timestamp SHALL NOT make an item new. When no previous launch timestamp exists, startup SHALL establish the first baseline and show no new-content markers.

#### Scenario: Emby library gained content between launches
- **WHEN** an Emby Latest section contains an item whose date-added timestamp is later than the previous launch and no later than the current launch
- **THEN** that section's Home Latest pill SHALL show an Iris `•`
- **AND** that library's own `Latest` content mode SHALL show the same marker

#### Scenario: Podcast and Feed sections use publication time
- **WHEN** an Audiobookshelf podcast or Feed Latest section contains an item whose publication timestamp falls within the launch interval
- **THEN** that section's Home Latest pill SHALL show an Iris `•`

#### Scenario: Timestamp does not establish new content
- **WHEN** an item has no valid provider timestamp, has a timestamp at or before the previous launch, or has a timestamp after the current launch
- **THEN** that item SHALL NOT cause its Latest pill to show a marker
- **AND** it SHALL NOT cause a library `Latest` content mode to show a marker

#### Scenario: First launch establishes the baseline
- **WHEN** no previous client-launch timestamp exists
- **THEN** mbv SHALL record the current launch timestamp
- **THEN** Home SHALL show no new-content markers for that launch

#### Scenario: Continue is not a Latest section
- **WHEN** one or more Latest pills show new-content markers
- **THEN** the Continue pill SHALL remain unmarked

### Requirement: Visiting a Home Latest pill clears its new-content marker
Selecting a Home Latest pill SHALL acknowledge that section and clear its marker immediately for the remainder of that client run. A Latest pill that is already selected when its content arrives SHALL count as visited and SHALL NOT show a marker. Acknowledgement SHALL be keyed by the section's provider identity so asynchronous section replacement, merging, or reordering cannot restore the marker during the same run.

For an Emby library section, acknowledgement SHALL be shared with that library's `Latest` content mode: selecting Home's `Latest` pill for a library SHALL clear the marker on that library's `Latest` mode, and selecting the library's `Latest` mode SHALL clear it on Home's pill for that library. Because two surfaces now read this acknowledgement, it SHALL be owned by the shell rather than by a single surface.

New content discovered after startup SHALL NOT create a marker during the current run; launch-relative marker evaluation SHALL remain bounded by the current launch timestamp.

#### Scenario: User selects a marked Latest pill
- **WHEN** the user selects a Home Latest pill showing `•`
- **THEN** its marker SHALL clear before the next frame
- **THEN** later refresh or asynchronous replacement of that section during the same run SHALL NOT restore it

#### Scenario: Acknowledging on the library surface clears Home
- **WHEN** the user selects a library's `Latest` content mode while that library's section shows a marker
- **THEN** the marker SHALL clear on the library's `Latest` mode and on Home's `Latest` pill for that library

#### Scenario: Selected section arrives asynchronously
- **WHEN** a Latest section is selected before or as its content arrives
- **THEN** that section SHALL count as visited
- **THEN** its pill SHALL NOT show `•`

#### Scenario: Content appears after the launch instant
- **WHEN** an item carries a provider timestamp later than the current launch timestamp
- **THEN** it SHALL NOT add a marker during the current run
- **THEN** it MAY qualify against the launch interval of a later run

### Requirement: Home Latest rows show their provider dates in the canonical gutter
Each item in a Home Latest section that carries a valid provider timestamp SHALL show that date in the canonical fixed-width right-aligned media-row gutter, formatted as unpadded day plus abbreviated month (for example, `7 Sep` or `17 Sep`) in the existing green gutter role. Emby rows SHALL show date added; Audiobookshelf podcast and Feed rows SHALL show publication date.

Continue rows SHALL NOT gain a date gutter from this behavior. An item without a valid provider timestamp SHALL reserve no date gutter.

#### Scenario: Latest rows from every supported source carry dates
- **WHEN** dated Emby, Audiobookshelf podcast, and Feed items render in their respective Home Latest sections
- **THEN** each row SHALL show its provider date in the same canonical right-aligned green gutter

#### Scenario: Continue keeps its playback-oriented presentation
- **WHEN** a dated item renders in Continue
- **THEN** this capability SHALL NOT add a date gutter to that row

#### Scenario: Latest item has no usable date
- **WHEN** a Home Latest item has no valid provider timestamp
- **THEN** its row SHALL reserve and paint no date gutter
