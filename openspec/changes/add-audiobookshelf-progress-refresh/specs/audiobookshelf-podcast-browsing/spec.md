## MODIFIED Requirements

### Requirement: Episode progress is read-only and identity-qualified
mbv SHALL display the authenticated user's Audiobookshelf progress for
downloaded podcast episodes using `libraryItemId` and `episodeId`. Catalog
browsing SHALL NOT write, infer, or periodically report progress.

#### Scenario: Episode has listening progress
- **WHEN** Audiobookshelf reports current time or completion state for a downloaded episode
- **THEN** mbv SHALL display the corresponding resume position or finished state on that episode

#### Scenario: Episode has no listening progress
- **WHEN** no progress record exists for a downloaded episode
- **THEN** mbv SHALL display it as unstarted rather than borrowing progress from another show or episode

#### Scenario: Progress changes outside mbv while the tab remains open
- **WHEN** progress changes on the server while the Audiobookshelf Socket.IO connection is authenticated and the tab remains open
- **THEN** mbv SHALL update the displayed progress for the matching episode from the resulting `user_item_progress_updated` event, without requiring an explicit REST refresh

#### Scenario: Progress changes while the socket is disconnected
- **WHEN** progress changes on the server while the Audiobookshelf Socket.IO connection is not currently authenticated
- **THEN** mbv MAY continue displaying the last REST-loaded value until the socket reconnects or an explicit REST refresh occurs
