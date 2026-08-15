# audiobookshelf-progress-refresh Specification

## Purpose

Keeps Audiobookshelf browse and queue progress current when listening
progress changes on another device or app, by authenticating a Socket.IO
connection with the existing API key and applying the server's push
notifications without polling or a new credential.

## Requirements

### Requirement: Socket.IO connects only in the interactive bare-mode process, tied to Audiobookshelf Service lifecycle
The Audiobookshelf Socket.IO connection SHALL exist only in the interactive bare-mode process. It SHALL open when the Audiobookshelf Service becomes Ready, close and reopen on Service replacement, and close on Service removal. A Local daemon or packaged `mbvd` Player owner SHALL NOT open an Audiobookshelf Socket.IO connection.

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

### Requirement: Socket connection authenticates with the installed API key
mbv SHALL authenticate the Audiobookshelf Socket.IO connection by emitting the `auth` client event with the currently installed API key, the same credential already used for REST requests. mbv SHALL NOT introduce a new credential type or secret storage location for this connection.

#### Scenario: Server accepts authentication
- **WHEN** Audiobookshelf acknowledges the `auth` event for the installed API key
- **THEN** mbv SHALL treat the connection as authenticated and begin applying its events

#### Scenario: Server rejects the token
- **WHEN** Audiobookshelf responds with `invalid_token` to the `auth` event
- **THEN** mbv SHALL classify this as an Audiobookshelf Service authentication failure using the existing failure classification
- **THEN** mbv SHALL NOT clear the installed API key on this rejection alone

### Requirement: user_item_progress_updated merges into cached progress by provider-qualified identity
On receiving `user_item_progress_updated`, mbv SHALL merge the event's progress data directly into cached Audiobookshelf episode and queue progress for the matching `(libraryItemId, episodeId)`, scoped to the setup generation current when the event arrived, without an additional REST request.

#### Scenario: Event matches a browsed or queued episode
- **WHEN** `user_item_progress_updated` identifies an episode currently displayed in browse state or present as an inactive queue slot
- **THEN** mbv SHALL update that episode's displayed progress from the event's data

#### Scenario: Event matches no known episode
- **WHEN** `user_item_progress_updated` identifies an episode absent from current browse and queue state
- **THEN** mbv SHALL apply no change

#### Scenario: Event belongs to a superseded setup generation
- **WHEN** a `user_item_progress_updated` event's connection generation is older than the current Audiobookshelf setup generation
- **THEN** mbv SHALL ignore it without updating browse or queue state

### Requirement: REST synchronization remains authoritative for the actively owned session
A `user_item_progress_updated` merge SHALL NOT modify the progress of the episode currently active in the in-process Player owner's own playback session. That slot's progress SHALL continue to be driven exclusively by the existing REST `sync_playback_session_bounded` and `close_playback_session_bounded` lifecycle.

#### Scenario: Socket event names the actively playing episode
- **WHEN** `user_item_progress_updated` identifies the episode currently active in the local Player owner's own Audiobookshelf playback session
- **THEN** mbv SHALL NOT apply the event's progress to that active slot
- **THEN** the active slot's progress SHALL continue to reflect only acknowledged REST synchronization

### Requirement: Only user_item_progress_updated is applied as listening progress
mbv SHALL subscribe to and act on `user_item_progress_updated` only. It SHALL NOT treat `stream_progress` (Audiobookshelf's HLS transcode chunk-encode percentage) or any other Socket.IO event as listening progress, and SHALL NOT act on admin-only or unrelated events (for example `user_online`, `user_stream_update`, `user_added`, `library_scan`).

#### Scenario: stream_progress arrives
- **WHEN** Audiobookshelf emits a `stream_progress` event during HLS transcoding
- **THEN** mbv SHALL NOT update any episode's displayed listening progress from it

#### Scenario: Unrelated event arrives
- **WHEN** Audiobookshelf emits a Socket.IO event other than `user_item_progress_updated`
- **THEN** mbv SHALL take no browse, queue, or playback action from it

### Requirement: Connection loss recovers without replaying stale progress
On unexpected Socket.IO disconnect, mbv SHALL retry the connection with backoff and SHALL NOT apply any event buffered from before the disconnect after reconnecting; only newly received events are merged.

#### Scenario: Transient network interruption
- **WHEN** the Audiobookshelf Socket.IO connection drops unexpectedly
- **THEN** mbv SHALL reconnect and re-authenticate with backoff
- **THEN** mbv SHALL NOT apply any pre-disconnect buffered event after reconnecting

### Requirement: Progress refresh adds no remote control, daemon, or ctrl transport
This capability SHALL NOT add Audiobookshelf remote-control command handling, Local daemon or packaged `mbvd` support, or any ctrl protocol change.

#### Scenario: Socket.IO event stream carries no remote-control action
- **WHEN** this capability is active
- **THEN** mbv SHALL NOT execute play, pause, seek, or other playback commands from any Audiobookshelf Socket.IO event
