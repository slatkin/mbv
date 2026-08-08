## Purpose

Defines what an audio-only Player owner admits into its queue and what it
discards, so that a submission containing non-audio items plays its audio rather
than being refused whole, and no non-audio item ever reaches a player that has
no display.

## ADDED Requirements

### Requirement: Mixed submissions are admitted minus their non-audio items

An audio-only Player owner SHALL accept a play or enqueue submission that
contains non-audio items, admitting the audio items and discarding the rest. It
SHALL NOT refuse the submission on the grounds that it is not wholly audio.

#### Scenario: Playing one track from a mixed selection

- **WHEN** a submission of five audio items and one video item is played on an
  audio-only owner
- **THEN** the owner's queue SHALL contain the five audio items
- **THEN** playback SHALL begin
- **THEN** the video item SHALL NOT appear in the owner's queue

#### Scenario: Submission is wholly non-audio

- **WHEN** a submission containing only non-audio items reaches an audio-only
  owner
- **THEN** the owner SHALL admit nothing
- **THEN** the owner SHALL NOT begin playback
- **THEN** the owner SHALL NOT change the queue it already holds

#### Scenario: Start index points at a discarded item

- **WHEN** a mixed submission is played with its start index on an item that is
  discarded
- **THEN** playback SHALL begin at the first admitted item at or after that
  position
- **THEN** if no admitted item follows, playback SHALL begin at the last
  admitted item

### Requirement: A non-audio item never reaches an audio-only owner's player

An audio-only Player owner SHALL NOT pass a non-audio item to its media player
by any path. Discarding SHALL happen at admission, so the owner's queue and its
player's playlist hold the same items.

#### Scenario: Advancing through an admitted queue

- **WHEN** an audio-only owner advances through a queue admitted from a mixed
  submission
- **THEN** every item it plays SHALL be audio
- **THEN** no skip SHALL be required at advance time, because no non-audio item
  is present to skip

### Requirement: Emby-started playback is admitted on the same terms

An audio-only Player owner SHALL apply the same admission rules to playback
started from Emby as to playback submitted over ctrl, with no client involved.

#### Scenario: Emby sends a mixed play command

- **WHEN** Emby starts playback of a mixed selection on an audio-only owner
- **THEN** the owner SHALL admit the audio items and discard the rest
- **THEN** the owner SHALL record the discard in its log
- **THEN** the owner SHALL NOT refuse the command

### Requirement: An owner-side discard is recorded but not reported

An audio-only Player owner SHALL record a discard in its log. It SHALL NOT send
a discard notification over ctrl. A controlling client strips non-audio items
before submitting and is where the user is told, so an owner-side discard means
the client's view of an item's type was wrong or no client was involved.

#### Scenario: Owner discards items a client did not strip

- **WHEN** a client submits a selection the owner then discards items from
- **THEN** the owner SHALL log the discard
- **THEN** the owner SHALL NOT send the submitting connection a notification
  about it
- **THEN** the submission SHALL otherwise be admitted and played normally
