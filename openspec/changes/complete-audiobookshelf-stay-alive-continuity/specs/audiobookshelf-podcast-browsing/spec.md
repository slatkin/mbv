## ADDED Requirements

### Requirement: Daemon-acknowledged progress reconciles client browse state
When an attached client applies a daemon owner's acknowledged Audiobookshelf progress event for the current setup generation, it SHALL update the displayed browse progress for the matching `libraryItemId` and `episodeId` and re-evaluate episode filters (such as Unplayed) accordingly, without polling, an explicit REST refresh, or Socket.IO. A superseded-generation event SHALL leave browse state unchanged.

#### Scenario: Acknowledged completion updates the Unplayed filter
- **WHEN** a capable client applies an acknowledged progress event marking a downloaded episode finished for the current generation
- **THEN** that episode SHALL present as finished and SHALL be excluded from the Unplayed filter

#### Scenario: Acknowledged position updates the resume state
- **WHEN** a capable client applies an acknowledged position below completion for the current generation
- **THEN** the matching episode SHALL display the corresponding resume position

#### Scenario: Superseded-generation acknowledgement is ignored for browse
- **WHEN** a received acknowledged progress event belongs to a replaced or removed setup generation
- **THEN** the client SHALL leave displayed browse progress and filters unchanged
