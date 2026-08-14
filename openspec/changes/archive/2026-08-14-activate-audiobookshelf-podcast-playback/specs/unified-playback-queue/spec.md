## MODIFIED Requirements

### Requirement: The Player branches only at source and reporting boundaries

The playback pipeline SHALL treat all admitted queue slots uniformly through ordering, lifecycle, status, and queue management. Item-kind branching SHALL occur only to resolve the active media source and to select progress-reporting behavior. Resolution MAY be just in time when the source requires an active server lifecycle.

#### Scenario: Resolve an Emby item

- **WHEN** an Emby item reaches the play boundary
- **THEN** the Player owner SHALL resolve its authenticated Emby stream URL
- **AND** SHALL use Emby playback reporting

#### Scenario: Resolve a Feed entry

- **WHEN** a Feed entry reaches the play boundary
- **THEN** the Player owner SHALL resolve its enclosure URL or fallback link directly
- **AND** SHALL NOT report progress to Emby

#### Scenario: Resolve an Audiobookshelf episode

- **WHEN** an Audiobookshelf podcast episode becomes active on the eligible in-process Player owner
- **THEN** that owner SHALL create and own its Audiobookshelf playback session before resolving its source
- **AND** SHALL use Audiobookshelf playback-session progress reporting
