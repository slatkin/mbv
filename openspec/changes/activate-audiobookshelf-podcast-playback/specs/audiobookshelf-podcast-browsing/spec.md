## MODIFIED Requirements

### Requirement: Podcast activation starts supported local playback
Downloaded podcast episodes SHALL support ordinary play and enqueue activation through the Audiobookshelf podcast playback capability. Non-episode rows and unavailable episodes SHALL retain selection without queue or playback side effects.

#### Scenario: User plays a downloaded podcast episode
- **WHEN** the user invokes the ordinary play action on a selected downloaded episode
- **THEN** mbv SHALL submit that provider-native episode through the ordinary queue and owner-admission boundary

#### Scenario: User enqueues a downloaded podcast episode
- **WHEN** the user invokes the ordinary enqueue action on a selected downloaded episode
- **THEN** mbv SHALL add it to the selected Composed or eligible Bound queue without starting it

#### Scenario: User activates a non-episode row
- **WHEN** the selected Audiobookshelf row does not identify an available downloaded episode
- **THEN** mbv SHALL retain selection without creating a QueueItem or opening a playback session

### Requirement: Podcast browsing reaches playback only through explicit episode actions
Catalog discovery, pagination, detail loading, progress hydration, artwork, filtering, and navigation SHALL remain read-oriented and SHALL NOT themselves create queue items, resolve streams, or open playback sessions. Only an explicit play or enqueue action on a downloaded episode SHALL cross into the Audiobookshelf podcast playback capability.

#### Scenario: User browses podcast catalog surfaces
- **WHEN** the user discovers libraries, pages shows, expands episodes, views progress or artwork, changes filters, or moves selection
- **THEN** no Audiobookshelf media SHALL enter a Composed or Bound queue
- **THEN** no Audiobookshelf playback lifecycle request SHALL occur

#### Scenario: User explicitly submits an episode
- **WHEN** the user invokes play or enqueue on a selected downloaded episode
- **THEN** browsing SHALL provide its provider-native identity and snapshot metadata to the playback boundary
- **THEN** browsing state SHALL NOT receive or retain the Service credential, playback `sessionId`, resolved media URL, or request headers
