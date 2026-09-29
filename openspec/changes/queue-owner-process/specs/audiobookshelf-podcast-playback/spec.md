## MODIFIED Requirements

### Requirement: Attached clients reconcile daemon-owned acknowledged progress
An attached client that negotiated the Audiobookshelf progress capability SHALL apply a daemon owner's acknowledged progress event to its browse state matched by provider-qualified identity (`libraryItemId` and `episodeId`), reflecting acknowledged position and completion, only while the client's captured setup generation matches the event's generation. The client's displayed queue SHALL reflect acknowledged progress only through the owner's queue snapshot: after applying acknowledged progress to its own queue slots, the daemon owner SHALL publish its resulting queue state, and the client SHALL NOT write progress into queue slots itself. Reconciliation SHALL NOT require polling or Socket.IO.

#### Scenario: Client receives acknowledged progress for a queued episode
- **WHEN** a capable client receives a daemon progress event whose generation matches and whose identity matches one or more of the owner's queue slots
- **THEN** every matching slot SHALL reflect the acknowledged position and completion state through the owner's published queue state

#### Scenario: Client receives progress for an episode it does not hold
- **WHEN** a daemon progress event identifies an episode absent from the owner's queue
- **THEN** the client SHALL apply no queue change and SHALL retain its existing queue state

#### Scenario: Progress belongs to a superseded generation
- **WHEN** a received progress event's setup generation is older than the client's current Audiobookshelf setup generation
- **THEN** the client SHALL ignore it without mutating queue or browse state

## ADDED Requirements

### Requirement: Only an eligible daemon Player owner binds episodes
A Local daemon or packaged `mbvd` Player owner SHALL bind Audiobookshelf podcast episodes only when its owner-scoped Audiobookshelf setup is installed and it has negotiated Audiobookshelf transport capability with a capable attached client. Ctrl owners that are not daemon-owner proxies, Library routes, and Emby Sessions SHALL remain ineligible.

#### Scenario: Eligible daemon owner with installed setup binds an episode
- **WHEN** an Audiobookshelf episode is submitted to a Local daemon or packaged `mbvd` owner that has installed Audiobookshelf setup and has negotiated Audiobookshelf transport capability
- **THEN** the episode SHALL be eligible for that owner's Bound queue and active-source lifecycle

#### Scenario: Daemon owner without installed setup receives a submission
- **WHEN** an Audiobookshelf episode targets a Local daemon or packaged `mbvd` owner that has no installed Audiobookshelf setup, or that has not negotiated Audiobookshelf transport capability
- **THEN** submission SHALL fail visibly without Bound queue mutation

#### Scenario: Unsupported owner receives a submission
- **WHEN** an Audiobookshelf episode targets a remote-only ctrl owner, Library route, or Emby Session
- **THEN** submission SHALL fail visibly without Bound queue mutation or local fall-through

#### Scenario: Credential is rejected during playback
- **WHEN** Audiobookshelf explicitly rejects the current credential
- **THEN** the active session SHALL be finalized or abandoned within bounds and the runtime context SHALL clear
- **THEN** repairable persisted owner snapshots SHALL remain while Audiobookshelf items become ineligible for Bound queues
- **THEN** installed Audiobookshelf setup and API key SHALL be preserved; retry is available on the next explicit play

### Requirement: Podcast activation uses ordinary play and enqueue on the local daemon
The selected downloaded Audiobookshelf episode SHALL support ordinary play and enqueue actions. Play SHALL select or create the corresponding queue slot and start it when the submission destination is eligible; enqueue SHALL add it without starting playback.

#### Scenario: User plays a selected episode
- **WHEN** the user invokes ordinary play on a downloaded episode toward the eligible local daemon
- **THEN** mbv SHALL place or select the episode in the local Bound queue and start that slot

#### Scenario: User enqueues a selected episode
- **WHEN** the user invokes ordinary enqueue on a downloaded episode
- **THEN** mbv SHALL add it through the canonical queue operation without opening a playback session or starting it

#### Scenario: Selected row is not an available episode
- **WHEN** play or enqueue targets a show, loading state, empty state, or unavailable episode
- **THEN** mbv SHALL NOT create a QueueItem or playback session

### Requirement: Podcast playback adds no credential transport and no Socket.IO remote control
Audiobookshelf podcast playback SHALL NOT transfer Service credentials between processes or make Audiobookshelf items playable by a remote-only ctrl owner; audiobook media is governed by the `audiobookshelf-book-playback` capability. The Audiobookshelf Socket.IO connection added by the `audiobookshelf-progress-refresh` capability SHALL NOT carry remote-control commands and SHALL NOT alter the Player owner's own playback-session lifecycle, which remains driven exclusively by REST. Daemon owners that have negotiated transport capability MAY carry Audiobookshelf queue items and acknowledged progress over the capability-gated ctrl seam established by the transport child (#525).

#### Scenario: User plays podcasts on the local daemon
- **WHEN** the user plays, seeks, pauses, completes, or stops downloaded Audiobookshelf episodes on the local daemon
- **THEN** all Audiobookshelf credentials and playback-session lifecycle SHALL remain in the local daemon
- **THEN** the active session's progress SHALL be driven only by REST synchronization, never by a Socket.IO event

#### Scenario: Daemon owner carries Audiobookshelf transport over ctrl
- **WHEN** a daemon owner with installed setup and a capable attached client plays an Audiobookshelf episode
- **THEN** Audiobookshelf credentials and playback-session lifecycle SHALL remain in the daemon owner
- **THEN** queue items and acknowledged progress MAY flow over the capability-gated ctrl seam to capable clients
- **THEN** no Audiobookshelf Socket.IO connection SHALL be opened by the daemon owner

## REMOVED Requirements

### Requirement: Only an eligible Player owner with Audiobookshelf context binds episodes
**Reason**: It named an in-process bare Player owner, which no longer exists.
**Migration**: Replaced by "Only an eligible daemon Player owner binds episodes".

### Requirement: Podcast activation uses ordinary play and enqueue semantics
**Reason**: Its play scenario targeted the in-process Player.
**Migration**: Replaced by "Podcast activation uses ordinary play and enqueue on the local daemon".

### Requirement: Bare podcast playback adds no audiobook support or credential transport, and no Socket.IO remote control
**Reason**: It described bare-mode playback; audiobook support is now governed by `audiobookshelf-book-playback`.
**Migration**: Replaced by "Podcast playback adds no credential transport and no Socket.IO remote control".
