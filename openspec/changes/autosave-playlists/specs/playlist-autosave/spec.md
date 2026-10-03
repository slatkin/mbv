# Spec Delta

## Purpose

Keeps an Emby playlist that is loaded as the Stay-alive queue in step with that queue:
the Player owner tracks whether the playlist matches what Emby last saved, and with
Autosave Playlists on, writes every change back to Emby in the background, whatever
caused the change.

## ADDED Requirements

### Requirement: One Autosave Playlists setting

The system SHALL offer one boolean setting, Autosave Playlists, which turns automatic
playlist saving on or off for every kind of queue change and every media type. It SHALL
default to off. When a configuration that has no Autosave Playlists value contains
either of the retired settings `save_playlist_on_consume` or
`save_playlist_on_consume_audio` set to true, Autosave Playlists SHALL start on. The
retired settings SHALL NOT be written back.

#### Scenario: Old audio-only setting migrates

- **WHEN** the configuration has `save_playlist_on_consume_audio = true` and no Autosave Playlists value
- **THEN** Autosave Playlists SHALL be on
- **AND** the next saved configuration SHALL contain Autosave Playlists and neither retired key

#### Scenario: Toggling reaches a running owner

- **WHEN** the user turns Autosave Playlists on in the TUI while the Stay-alive process is running
- **THEN** the owner SHALL apply the setting to the next queue change without restarting

### Requirement: The owner holds the saved baseline of a linked playlist

A queue is linked to a playlist when its source names a saved Emby playlist. For a
linked queue, the Stay-alive Player owner SHALL hold the saved baseline: the ordered
Emby item ids that Emby last confirmed for that playlist. The queue's playlist content
is its Emby items' ids in queue order. Items of other kinds do not count as playlist
content. The owner SHALL set the baseline when:
- it accepts a whole-queue replacement or idle load whose source names a playlist; the
  baseline is then the replacement's playlist content;
- an autosave of that playlist succeeds; the baseline is then the ids that save wrote;
- a Client reports that an explicit save of the queue's playlist succeeded for the
  current lineage; the baseline is then the ids that save wrote.

The baseline SHALL be persisted with the owner's queue and restored with it.

#### Scenario: Loading a playlist sets the baseline

- **WHEN** a Client replaces the Stay-alive queue with a saved playlist's items
- **THEN** the owner SHALL report the playlist as saved

#### Scenario: Baseline survives an owner restart

- **WHEN** the owner restarts with a persisted linked queue whose content differs from its baseline
- **THEN** the restored queue SHALL be reported unsaved

#### Scenario: Appending a non-Emby item does not change playlist content

- **WHEN** a Feed entry is appended to a saved, linked queue
- **THEN** the owner SHALL still report the playlist as saved

### Requirement: The owner reports linked-playlist save state

Every queue snapshot from the Stay-alive owner SHALL carry exactly one linked-playlist
state:
- not linked: the source names no saved playlist;
- saved: the content equals the baseline and no write is in flight;
- unsaved: the content differs from the baseline and no write is in flight;
- saving: a write of this playlist is in flight;
- save failed: the most recent write of the current content failed.

A Client SHALL derive its saved/unsaved presentation only from this state. A Client
SHALL NOT keep its own record of whether the queue is unsaved. The queue status pill
SHALL show unsaved when the state is unsaved, saving while saving, save failed after a
failure, autosave on when saved with Autosave Playlists on, and nothing otherwise.

#### Scenario: Unlinked queue never shows unsaved

- **WHEN** the user removes an entry from an album queue
- **THEN** the queue status pill SHALL NOT show unsaved

#### Scenario: Undoing back to the baseline is saved

- **WHEN** Autosave Playlists is off and the user removes an entry from a saved, linked queue and then undoes the removal
- **THEN** the owner SHALL report the playlist as saved

#### Scenario: A TUI restart keeps the unsaved state

- **WHEN** a linked queue is unsaved and the TUI exits and attaches again
- **THEN** the newly attached TUI SHALL show the queue as unsaved

### Requirement: The owner autosaves every change to a linked playlist

When Autosave Playlists is on and the Local-role Stay-alive owner accepts a queue change
after which the linked queue's playlist content differs from its baseline, the owner
SHALL write that content to the playlist on Emby. This SHALL hold for every source of
change: consume, removal, move, append, undo, and edits from any Client, including
changes made while no Client is attached. A whole-queue replacement or clear SHALL NOT
cause a write to the playlist the queue was linked to. The write SHALL run off the
owner's event loop: queue edits, undos and consumes SHALL be answered without waiting
for it. At most one write per playlist SHALL be in flight. Content that changes while a
write is in flight SHALL be written after it completes, and only the newest pending
content SHALL be written. A pending write SHALL still be completed after the queue is
replaced. The packaged mbvd owner SHALL NOT autosave.

#### Scenario: Headless consume is saved

- **WHEN** Autosave Playlists is on, no Client is attached, and the owner consumes the first entry of a linked queue
- **THEN** the owner SHALL write the shortened content to the playlist on Emby
- **AND** the playlist SHALL be reported saved once the write succeeds

#### Scenario: Rapid edits coalesce

- **WHEN** the user removes three entries in quick succession while the first write is still in flight
- **THEN** the owner SHALL perform at most one further write, containing the content after all three removals

#### Scenario: Clear does not empty the playlist

- **WHEN** Autosave Playlists is on and the user clears a linked queue
- **THEN** the owner SHALL NOT write to the playlist on Emby

#### Scenario: Autosave off leaves the queue unsaved

- **WHEN** Autosave Playlists is off and the owner consumes an entry of a saved, linked queue
- **THEN** the owner SHALL NOT write to Emby
- **AND** it SHALL report the playlist as unsaved

### Requirement: A failed autosave is visible and retried by the next change

When an autosave write fails, the owner SHALL report save failed, keep the baseline
unchanged, and SHALL NOT retry until the queue content changes again. A failed write
SHALL NOT be reported as saved.

#### Scenario: Server unreachable during autosave

- **WHEN** an autosave write fails because Emby is unreachable
- **THEN** attached Clients SHALL show save failed
- **AND** the next accepted change to the queue SHALL start a new write

### Requirement: Autosave overwrites the server playlist

An autosave SHALL replace the playlist's entries on Emby with the queue's playlist
content, without comparing the server's current entries to the baseline. Edits made to
the playlist outside mbv while it is linked SHALL be overwritten by the next autosave.

#### Scenario: Edit made in another client is overwritten

- **WHEN** the playlist gains an entry in another Emby client while it is linked, and the owner then autosaves after a consume
- **THEN** the playlist on Emby SHALL equal the queue's playlist content
