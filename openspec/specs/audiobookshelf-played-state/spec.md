# audiobookshelf-played-state Specification

## Purpose

Lets the user mark Audiobookshelf podcast episodes and books as finished or
unfinished on the server, and keeps mbv's browse and queue progress consistent
with that change.

## Requirements

### Requirement: Marking one item writes its finished state to the server
Mark Played or Mark Unplayed on one Audiobookshelf item SHALL send that item's
finished state to the configured Audiobookshelf Service. The request SHALL
name the item by its provider-native identity: `libraryItemId` and
`episodeId` for a podcast episode, and `libraryItemId` alone for a book.

#### Scenario: Mark a podcast episode played
- **WHEN** the user chooses Mark Played on an unfinished podcast episode
- **THEN** mbv sends a finished state of true for that episode's `libraryItemId` and `episodeId`

#### Scenario: Mark a book unplayed
- **WHEN** the user chooses Mark Unplayed on a finished book
- **THEN** mbv sends a finished state of false for that book's `libraryItemId`, with no episode identity

### Requirement: Marking a multi-selection uses one batch request
Mark Played or Mark Unplayed on a multi-selection of Audiobookshelf items
SHALL send one batch request that sets the same finished state for every
selected item.

#### Scenario: Three episodes marked played
- **WHEN** three podcast episodes are selected and the user chooses Mark Played
- **THEN** mbv sends one batch request that sets finished to true for all three episodes

### Requirement: An accepted mark updates local progress
When the server accepts a mark, mbv SHALL apply it to cached browse progress
and to matching inactive queue slots without waiting for a socket event.
Finished SHALL keep the item's position. Unfinished SHALL reset the position
to zero only for an item that was finished. Queue slots SHALL change through
the local daemon's published state.

#### Scenario: Finished episode marked unplayed
- **WHEN** the server accepts Mark Unplayed for an episode that was finished at 40 minutes
- **THEN** the episode's row shows it unplayed with no saved position

#### Scenario: In-progress episode in a bulk Mark Unplayed
- **WHEN** a bulk Mark Unplayed includes an episode that is in progress and not finished
- **THEN** that episode keeps its saved position and stays unplayed

#### Scenario: Marked episode is queued
- **WHEN** the server accepts Mark Played for an episode that is an inactive queue slot
- **THEN** the queue shows that slot as finished through the local daemon's published state

### Requirement: The actively owned session is not modified by a mark
A mark SHALL NOT change the progress of the queue slot that is active in the
Player owner's own Audiobookshelf playback session. Browse progress for that
item SHALL still update.

#### Scenario: Mark the playing episode
- **WHEN** the user marks the currently playing episode played and the server accepts
- **THEN** the episode's browse row shows it finished
- **AND** the active queue slot's progress still reflects only the playback session's own synchronization

### Requirement: A failed mark changes nothing locally
When the mark request fails or the Audiobookshelf Service is not Ready, mbv
SHALL leave browse and queue progress unchanged and SHALL show an error toast.
A credential rejection SHALL use the existing Audiobookshelf authentication
failure classification.

#### Scenario: Server unreachable
- **WHEN** the user chooses Mark Played and the request fails with a connectivity error
- **THEN** the row's played state is unchanged and an error toast is shown

#### Scenario: Credential rejected
- **WHEN** the server rejects the API key on a mark request
- **THEN** mbv classifies it as an Audiobookshelf authentication failure and changes no progress
