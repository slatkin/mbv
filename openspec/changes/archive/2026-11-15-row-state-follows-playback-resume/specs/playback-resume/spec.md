# Spec Delta

## ADDED Requirements

### Requirement: A row shows resume state exactly when Play would resume

A list row SHALL show resume state, with its progress percentage, exactly when playing the item would resume from a saved position under the one-percent rule and the item's Resume source. Otherwise a played item SHALL show played state and every other item SHALL show neither. Every list that shows played or resume state SHALL apply this rule, in both the TUI Client and the GUI Client.

#### Scenario: Emby video played and partly rewatched

- **WHEN** an Emby video is played and Emby reports a saved position above 1% of its runtime
- **THEN** its row SHALL show resume state with the position's percentage

#### Scenario: Position below one percent

- **WHEN** an Emby video, feed entry, or unfinished Audiobookshelf item has a positive saved position below 1% of its known runtime
- **THEN** its row SHALL NOT show resume state
- **AND** it SHALL show played state only when the item is played

#### Scenario: Unknown runtime

- **WHEN** an item has a positive saved position and unknown runtime
- **THEN** its row SHALL show resume state with no percentage

#### Scenario: Finished Audiobookshelf item keeps a position

- **WHEN** an Audiobookshelf episode or book is finished and keeps a saved position
- **THEN** its row SHALL show played state, because Audiobookshelf starts a finished item from the beginning

#### Scenario: Unfinished Audiobookshelf item

- **WHEN** an Audiobookshelf episode or book is not finished and has a saved position above 1% of its duration
- **THEN** its row SHALL show resume state with the position's percentage

#### Scenario: Played feed entry with a resumable position

- **WHEN** a feed entry is played and keeps a locally recorded position above 1% of its runtime
- **THEN** its row SHALL show resume state, because the feed entry resumes from its local position

#### Scenario: A music track never shows resume state

- **WHEN** a list shows an Emby music track with a stored position or played flag
- **THEN** its row SHALL show neither resume state nor played state

#### Scenario: Every screen agrees

- **WHEN** the same item is shown on Home, in a Queue row that is not playing, and in its destination list, in either Client
- **THEN** every row SHALL show the same resume or played state
