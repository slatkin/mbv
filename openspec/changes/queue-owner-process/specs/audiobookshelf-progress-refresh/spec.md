## RENAMED Requirements

- FROM: `### Requirement: Socket.IO connects only in the interactive bare-mode process, tied to Audiobookshelf Service lifecycle`
- TO: `### Requirement: Socket.IO connects only in the terminal UI process, tied to Audiobookshelf Service lifecycle`

- FROM: `### Requirement: Progress refresh adds no remote control, daemon, or ctrl transport`
- TO: `### Requirement: Progress refresh adds no remote control and no daemon socket`

## MODIFIED Requirements

### Requirement: Socket.IO connects only in the terminal UI process, tied to Audiobookshelf Service lifecycle
The Audiobookshelf Socket.IO connection SHALL exist only in the interactive terminal UI process. It SHALL open when the Audiobookshelf Service becomes Ready, close and reopen on Service replacement, and close on Service removal. A Local daemon or packaged `mbvd` Player owner SHALL NOT open an Audiobookshelf Socket.IO connection.

#### Scenario: Audiobookshelf Service becomes Ready
- **WHEN** the Audiobookshelf Service transitions to Ready in the interactive process
- **THEN** mbv SHALL open an Audiobookshelf Socket.IO connection

#### Scenario: Audiobookshelf Service is replaced
- **WHEN** Audiobookshelf setup is replaced with a different server
- **THEN** mbv SHALL close the existing socket and open a new one for the replacement server

#### Scenario: Audiobookshelf Service is removed
- **WHEN** Audiobookshelf setup is removed
- **THEN** mbv SHALL close the socket and open no new connection

#### Scenario: Daemon or packaged mbvd owner
- **WHEN** a Local daemon or packaged `mbvd` Player owner is running
- **THEN** it SHALL NOT open an Audiobookshelf Socket.IO connection

### Requirement: user_item_progress_updated merges into cached progress by provider-qualified identity
On receiving `user_item_progress_updated`, mbv SHALL merge the event's progress data directly into cached Audiobookshelf episode browse progress for the matching `(libraryItemId, episodeId)`, scoped to the setup generation current when the event arrived, without an additional REST request. For queue progress, the terminal UI SHALL relay the event's progress to the local daemon, which SHALL apply it to its matching inactive queue slots and publish its resulting queue state; the terminal UI SHALL NOT write the progress into a queue slot itself.

#### Scenario: Event matches a browsed or queued episode
- **WHEN** `user_item_progress_updated` identifies an episode currently displayed in browse state or present as an inactive queue slot
- **THEN** mbv SHALL update that episode's displayed progress from the event's data, in browse state directly and in the queue through the local daemon's published state

#### Scenario: Event matches no known episode
- **WHEN** `user_item_progress_updated` identifies an episode absent from current browse and queue state
- **THEN** mbv SHALL apply no change

#### Scenario: Event belongs to a superseded setup generation
- **WHEN** a `user_item_progress_updated` event's connection generation is older than the current Audiobookshelf setup generation
- **THEN** mbv SHALL ignore it without updating browse or queue state

### Requirement: REST synchronization remains authoritative for the actively owned session
A `user_item_progress_updated` merge SHALL NOT modify the progress of the episode currently active in the Player owner's own playback session. That slot's progress SHALL continue to be driven exclusively by the existing REST `sync_playback_session_bounded` and `close_playback_session_bounded` lifecycle.

#### Scenario: Socket event names the actively playing episode
- **WHEN** `user_item_progress_updated` identifies the episode currently active in the local Player owner's own Audiobookshelf playback session
- **THEN** mbv SHALL NOT apply the event's progress to that active slot
- **THEN** the active slot's progress SHALL continue to reflect only acknowledged REST synchronization

### Requirement: Progress refresh adds no remote control and no daemon socket
This capability SHALL NOT add Audiobookshelf remote-control command handling, and SHALL NOT make the Local daemon or packaged `mbvd` open a Socket.IO connection. Its only ctrl use SHALL be relaying received listening progress to the local daemon as a queue progress update.

#### Scenario: Socket.IO event stream carries no remote-control action
- **WHEN** this capability is active
- **THEN** mbv SHALL NOT execute play, pause, seek, or other playback commands from any Audiobookshelf Socket.IO event
