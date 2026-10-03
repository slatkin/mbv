# Spec Delta

## MODIFIED Requirements

### Requirement: A queue replacement deferred behind the unsaved-playlist prompt runs only through that prompt's answer

When a queue replacement would discard unsaved changes to the queue's saved playlist, the system SHALL hold the replacement and ask the user to save, discard, or cancel. The queue has unsaved changes only when the Player owner reports its linked-playlist state as unsaved or save failed; a playlist that is saved or saving SHALL NOT raise the prompt. Discard SHALL run the held replacement immediately. Cancel SHALL drop it. Save SHALL start one playlist save and bind the held replacement to that save. The held replacement SHALL run only when that bound save completes successfully for the same queue and playlist. If the bound save fails or cannot start, the held replacement SHALL be dropped. No other playlist save SHALL run it, including an owner autosave, a save on quit, or a later manual save. If the prompt is closed without an answer, the held replacement SHALL never run. Replacements held by the populated-queue confirmation and by the "play locally instead" prompt SHALL run only from their own prompt's confirmation.

#### Scenario: Save answer runs the replacement after its save

- **WHEN** the user answers Save on the unsaved-playlist prompt
- **AND** the save it started completes successfully
- **THEN** the held replacement SHALL run

#### Scenario: Prompt closed without an answer

- **WHEN** the unsaved-playlist prompt is closed by something other than the user's answer
- **AND** a later owner autosave of the same playlist completes successfully
- **THEN** the held replacement SHALL NOT run

#### Scenario: Another save completes first

- **WHEN** the user answers Save while an earlier save of the same playlist is still in flight
- **AND** that earlier save completes successfully
- **THEN** the held replacement SHALL NOT run until the save started by the answer completes

#### Scenario: Bound save fails

- **WHEN** the save started by the Save answer fails
- **THEN** the held replacement SHALL NOT run
- **AND** a later successful save of the same playlist SHALL NOT run it

#### Scenario: Save completion with a gated replacement outstanding

- **WHEN** a replacement awaits the populated-queue confirmation
- **AND** a playlist save completes
- **THEN** the gated replacement SHALL NOT run until the user confirms that prompt

#### Scenario: An autosave in flight does not prompt

- **WHEN** Autosave Playlists is on, the owner reports the linked playlist as saving, and the user plays a different album
- **THEN** the unsaved-playlist prompt SHALL NOT appear

### Requirement: The Stay-alive process holds the queue source

The Stay-alive process SHALL hold the queue source as part of its queue state. Every whole-queue replacement it accepts, every clear, and every source-only update SHALL set its source; a clear SHALL reset it to Unknown. A source-only update SHALL apply only to the queue lineage the owner held when the update was requested; a delayed update from an earlier queue SHALL NOT rename a later queue. A Client attached to that owner SHALL display the owner's source and SHALL NOT maintain an independent authoritative source for it. Saving the queue as a new playlist (Save As) and overwriting an existing playlist with the queue SHALL both reach the owner as source-only updates carrying the lineage observed when the save was requested and the playlist content that save wrote; the owner SHALL adopt that content as the new playlist's saved baseline, and the Client SHALL report the queue saved only once an owner snapshot reports the new source's playlist as saved. A Client that has not yet received an owner snapshot SHALL refuse to save the queue to a playlist, and SHALL create or change no server playlist.

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
- **THEN** the Client SHALL send the owner a source-only update naming the replacement playlist, carrying the lineage observed when the overwrite was requested and the content it wrote
- **AND** the queue SHALL stay unsaved until an owner snapshot reports that playlist as saved

#### Scenario: No owner snapshot refuses a playlist save

- **WHEN** a Client attached to the Stay-alive process has received no owner queue snapshot and the user saves, saves as, or overwrites a playlist
- **THEN** the Client SHALL show an error
- **AND** no server playlist SHALL be created, updated, or deleted

### Requirement: Queue undo is an owner operation

The Player owner SHALL hold the queue's undo history. It SHALL record one undo entry for each append, removal, multi-removal, and move it applies, whichever Client requested it, and for each consume it performs, including consumes while no Client is attached. A Client's undo request SHALL ask the owner to undo its most recent entry and SHALL follow the same answered-edit rule as any other edit; the owner's answer SHALL say whether an entry was undone, the history was empty, or the entry no longer applies. Undoing a removal or consume SHALL insert the removed item before the entry now at the removed item's former position, or at the end when that position no longer exists; the restored entry is a new occurrence. Undoing a move SHALL move the same slot back to its former position. Undoing an append SHALL remove the appended slots that still exist. An undo is a queue change and SHALL be autosaved like any other. A whole-queue replacement or clear SHALL discard the history. The history SHALL be bounded, discarding its oldest entries first, and SHALL NOT survive an owner restart. A Client SHALL send undo requests only to an owner that advertises owner-held undo.

#### Scenario: Undo a removal

- **WHEN** the user removes the third entry and then undoes
- **THEN** the owner SHALL hold that item at the third position again
- **AND** every attached Client SHALL display it there

#### Scenario: Undo a move whose slot is gone

- **WHEN** the user undoes a move after another Client removed the moved entry
- **THEN** the owner SHALL reject the undo as stale slot addressing
- **AND** the Client SHALL report that the undo did not apply

#### Scenario: Undo a consume that happened while detached

- **WHEN** the owner consumes the first entry while no Client is attached, and a Client then attaches and undoes
- **THEN** the owner SHALL hold the consumed item at the first position again

#### Scenario: Undo an append

- **WHEN** the user appends two items and then undoes
- **THEN** the owner SHALL remove both appended slots

#### Scenario: Undo undoes another Client's edit

- **WHEN** one Client moves an entry and a second Client then undoes
- **THEN** the owner SHALL move that entry back

#### Scenario: Nothing to undo

- **WHEN** the user undoes after the queue was replaced
- **THEN** the Client SHALL report that there is nothing to undo
- **AND** the queue SHALL be unchanged
