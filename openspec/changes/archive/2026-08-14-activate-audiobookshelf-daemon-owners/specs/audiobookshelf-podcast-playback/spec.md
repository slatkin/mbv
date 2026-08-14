## MODIFIED Requirements

### Requirement: Only an eligible Player owner with Audiobookshelf context binds episodes
An in-process bare Player owner with current Audiobookshelf context, prepared-source support, and reporting/finalization support SHALL bind Audiobookshelf podcast episodes. A Local daemon or packaged `mbvd` Player owner SHALL bind Audiobookshelf podcast episodes only when its owner-scoped Audiobookshelf setup is installed and it has negotiated Audiobookshelf transport capability with a capable attached client. Ctrl owners that are not daemon-owner proxies, Library routes, and Emby Sessions SHALL remain ineligible.

#### Scenario: Eligible bare owner binds an episode
- **WHEN** an Audiobookshelf episode is submitted to the complete in-process Player capability
- **THEN** it SHALL be eligible for that owner's Bound queue and active-source lifecycle

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
- **THEN** repairable Composed and persisted snapshots SHALL remain while Audiobookshelf items become ineligible for Bound queues
- **THEN** installed Audiobookshelf setup and API key SHALL be preserved; retry is available on the next explicit play

### Requirement: Daemon owner updates its canonical queue by provider-qualified identity at the post-sync acknowledgement point
After Audiobookshelf accepts a synchronization request, the daemon owner SHALL update the matching slot in its canonical Bound queue by provider-qualified identity (`library_item_id`, `episode_id`) with the acknowledged position and completion state. The daemon owner SHALL then broadcast acknowledged provider-qualified progress to all capable attached clients, reusing the capability-gated broadcast seam.

#### Scenario: Periodic synchronization accepted by server
- **WHEN** Audiobookshelf accepts a periodic progress synchronization from the daemon owner
- **THEN** the daemon owner SHALL update the matching Bound queue slot with the acknowledged position
- **THEN** the daemon owner SHALL broadcast the acknowledged progress to capable attached clients

#### Scenario: Final synchronization accepted at completion
- **WHEN** Audiobookshelf accepts the final progress report at natural episode completion
- **THEN** the daemon owner SHALL mark the Bound queue slot as complete and broadcast to capable attached clients

#### Scenario: Synchronization fails or times out on the daemon
- **WHEN** a dispatched synchronization from the daemon owner times out or loses its response
- **THEN** the daemon owner SHALL NOT add that dispatched interval to a later request
- **THEN** playback SHALL continue and progress retry follows the same rules as bare mode

### Requirement: Daemon Audiobookshelf playback and progress continue after every client exits
Daemon owner Audiobookshelf playback and progress synchronization SHALL continue without interruption after every attached client exits. Playback, periodic synchronization, and session finalization SHALL proceed from the daemon owner independently of client presence.

#### Scenario: Last client exits while daemon plays an Audiobookshelf episode
- **WHEN** the last attached client exits while the daemon owner has an Audiobookshelf episode active
- **THEN** playback and periodic synchronization SHALL continue uninterrupted in the daemon
- **THEN** no new finalization or queue mutation SHALL be triggered by client exit alone

#### Scenario: Daemon owner finalizes Audiobookshelf session without clients
- **WHEN** an Audiobookshelf episode reaches natural completion while no clients are attached
- **THEN** the daemon owner SHALL finalize the session within bounds and advance or stop the canonical Bound queue

### Requirement: Installed setup and API key are preserved after playback or progress failure
When a daemon owner's Audiobookshelf playback or progress synchronization fails for a reason other than explicit credential rejection, the owner SHALL preserve installed Audiobookshelf setup and API key and fail the request or interval visibly. The owner SHALL NOT clear setup or key on transient failure; retry is available on the next explicit play.

#### Scenario: Progress synchronization fails transiently
- **WHEN** an Audiobookshelf server is temporarily unreachable during daemon owner progress synchronization
- **THEN** the daemon owner SHALL preserve installed setup and API key
- **THEN** playback SHALL continue and the failure SHALL be indicated concisely

#### Scenario: Playback open request fails transiently
- **WHEN** a daemon owner playback open request fails because the Audiobookshelf server is unreachable
- **THEN** the daemon owner SHALL fail the submission visibly without clearing installed setup or key
- **THEN** the next explicit play SHALL be available as a retry path

### Requirement: Setup replacement and disconnect purge daemon Audiobookshelf state
On daemon owner Audiobookshelf setup replacement with a different server, the owner SHALL finalize active Audiobookshelf playback within bounds and purge Audiobookshelf-associated Bound and persisted slots for the old server before committing the replacement. On `mbvd --disconnect abs`, the owner SHALL finalize active Audiobookshelf playback within bounds, stop the entire queue, and purge Audiobookshelf Bound and persisted slots.

#### Scenario: Daemon owner setup is replaced with a different server
- **WHEN** `mbvd --connect abs` commits a setup for a different Audiobookshelf server
- **THEN** the daemon owner SHALL finalize any active Audiobookshelf playback within bounds
- **THEN** Audiobookshelf Bound and persisted slots from the old server SHALL be purged

#### Scenario: Daemon owner disconnects Audiobookshelf
- **WHEN** `mbvd --disconnect abs` is executed
- **THEN** the daemon owner SHALL finalize any active Audiobookshelf playback within bounds
- **THEN** the entire queue SHALL stop and Audiobookshelf Bound and persisted slots SHALL be purged

## MODIFIED Requirements

### Requirement: Bare podcast playback does not add Socket.IO, audiobook, or credential transport
Bare-mode Audiobookshelf playback SHALL NOT connect to Audiobookshelf Socket.IO, support audiobook media, transfer Service credentials between processes, or make Audiobookshelf items playable by a remote-only ctrl owner. Daemon owners that have negotiated transport capability MAY carry Audiobookshelf queue items and acknowledged progress over the capability-gated ctrl seam established by the transport child (#525); this is not a bare-mode concern.

#### Scenario: User plays podcasts in bare mode
- **WHEN** the user plays, seeks, pauses, completes, or stops downloaded Audiobookshelf episodes in bare mode
- **THEN** all Audiobookshelf credentials and playback-session lifecycle SHALL remain in the in-process Player owner
- **THEN** no Socket.IO connection SHALL be required or opened

#### Scenario: Daemon owner carries Audiobookshelf transport over ctrl
- **WHEN** a daemon owner with installed setup and a capable attached client plays an Audiobookshelf episode
- **THEN** Audiobookshelf credentials and playback-session lifecycle SHALL remain in the daemon owner
- **THEN** queue items and acknowledged progress MAY flow over the capability-gated ctrl seam to capable clients
