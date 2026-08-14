## ADDED Requirements

### Requirement: Attached clients reconcile daemon-owned acknowledged progress
An attached client that negotiated the Audiobookshelf progress capability SHALL apply a daemon owner's acknowledged progress event to its canonical queue slots matched by provider-qualified identity (`libraryItemId` and `episodeId`), reflecting acknowledged position and completion, only while the client's captured setup generation matches the event's generation. Reconciliation SHALL reuse the same apply path as bare-mode owned progress and SHALL NOT require polling or Socket.IO.

#### Scenario: Client receives acknowledged progress for a queued episode
- **WHEN** a capable client receives a daemon progress event whose generation matches and whose identity matches one or more of its queue slots
- **THEN** every matching slot SHALL reflect the acknowledged position and completion state

#### Scenario: Client receives progress for an episode it does not hold
- **WHEN** a daemon progress event identifies an episode absent from the client's queue
- **THEN** the client SHALL apply no queue change and SHALL retain its existing queue state

#### Scenario: Progress belongs to a superseded generation
- **WHEN** a received progress event's setup generation is older than the client's current Audiobookshelf setup generation
- **THEN** the client SHALL ignore it without mutating queue or browse state

### Requirement: Daemon-owned Audiobookshelf playback continues across client exits
A Local daemon owning active Audiobookshelf playback SHALL continue playback, periodic progress synchronization, and bounded session finalization after every attached client exits, and SHALL resume emitting acknowledged progress to a later capable client without restarting the session. No client exit SHALL cause the owner to finalize or abandon otherwise-healthy Audiobookshelf playback.

#### Scenario: Sole client exits during active playback
- **WHEN** the last attached client of a stay-alive Local daemon exits while an Audiobookshelf episode is playing
- **THEN** the daemon SHALL continue playback, synchronization, and finalization with no attached client

#### Scenario: A later client attaches to the running owner
- **WHEN** a capable client attaches to a daemon that is already playing an Audiobookshelf episode
- **THEN** the client SHALL observe the live active episode, status, and last-acknowledged progress without the owner restarting the session

#### Scenario: Explicit daemon shutdown during Audiobookshelf playback
- **WHEN** the daemon is explicitly shut down while an Audiobookshelf episode is active
- **THEN** the owner SHALL finalize or abandon the active session within the existing teardown budget before exit

### Requirement: Daemon-owned Audiobookshelf playback is verified across the podcast lifecycle
Daemon-owned Audiobookshelf podcast playback SHALL be validated across direct and HLS resolution, resume, pause, seek, natural completion, no-client operation, client reattachment, and explicit daemon shutdown, preserving one canonical queue and coherent client reconciliation throughout.

#### Scenario: Resume, pause, seek, and completion under daemon ownership
- **WHEN** a daemon owner plays an Audiobookshelf episode through resume, pause, seek, and natural completion for both direct and HLS resolution
- **THEN** acknowledged progress SHALL advance monotonically in listening time and reconcile on every capable attached client

#### Scenario: Reattachment after no-client operation
- **WHEN** the daemon runs Audiobookshelf playback with no client attached and a capable client later reattaches
- **THEN** the client SHALL adopt the live queue and acknowledged position without overwriting daemon authority
