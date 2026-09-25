# Spec Delta

## MODIFIED Requirements

### Requirement: The Stay-alive process holds the queue source

The Stay-alive process SHALL hold the queue source as part of its queue state. Every whole-queue replacement it accepts, every clear, and every source-only update SHALL set its source; a clear SHALL reset it to Unknown. A source-only update SHALL apply only to the queue lineage the owner held when the update was requested; a delayed update from an earlier queue SHALL NOT rename a later queue. A Client attached to that owner SHALL display the owner's source and SHALL NOT maintain an independent authoritative source for it. Saving the queue as a new playlist (Save As) and overwriting an existing playlist with the queue SHALL both reach the owner as source-only updates carrying the lineage observed when the save was requested; the Client SHALL report the queue clean only once an owner snapshot shows the new source. A Client that has not yet received an owner snapshot SHALL refuse to save the queue to a playlist, and SHALL create or change no server playlist.

#### Scenario: Playing a different source updates the owner

- **WHEN** a Client replaces the Stay-alive queue with items from a new album, playlist, or other source and starts playback
- **THEN** the owner's queue source SHALL become that new source
- **AND** every attached Client SHALL display the new source

#### Scenario: Clearing resets the source

- **WHEN** the Stay-alive queue is cleared
- **THEN** the owner's queue source SHALL reset to Unknown
- **AND** attached Clients SHALL display an empty queue with no source label

#### Scenario: A delayed Save As cannot rename a later queue

- **WHEN** a source-only update from an earlier queue arrives after another Client replaced the queue
- **THEN** the owner SHALL reject it
- **AND** the later queue's source SHALL remain unchanged

#### Scenario: Overwriting a playlist updates the owner's source

- **WHEN** a Client attached to the Stay-alive process overwrites an existing playlist with the queue and the server replacement succeeds
- **THEN** the Client SHALL send the owner a source-only update naming the replacement playlist, carrying the lineage observed when the overwrite was requested
- **AND** the queue SHALL stay dirty until an owner snapshot with that source arrives

#### Scenario: No owner snapshot refuses a playlist save

- **WHEN** a Client attached to the Stay-alive process has received no owner queue snapshot and the user saves, saves as, or overwrites a playlist
- **THEN** the Client SHALL show an error
- **AND** no server playlist SHALL be created, updated, or deleted
