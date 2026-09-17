# home-latest-sections Delta

## ADDED Requirements

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
