# audiobookshelf-podcast-playback Specification

## Purpose

Defines secure bare-mode playback activation, owner eligibility, listening-progress synchronization, ordered playback-session finalization, and local progress reconciliation for downloaded Audiobookshelf podcast episodes.

## Requirements

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

### Requirement: Podcast activation uses ordinary play and enqueue semantics
The selected downloaded Audiobookshelf episode SHALL support ordinary play and enqueue actions. Play SHALL select or create the corresponding queue slot and start it when the submission destination is eligible; enqueue SHALL add it without starting playback.

#### Scenario: User plays a selected episode in bare mode
- **WHEN** the user invokes ordinary play on a downloaded episode toward the eligible in-process Player
- **THEN** mbv SHALL place or select the episode in the local Bound queue and start that slot

#### Scenario: User enqueues a selected episode
- **WHEN** the user invokes ordinary enqueue on a downloaded episode
- **THEN** mbv SHALL add it through the canonical queue operation without opening a playback session or starting it

#### Scenario: Selected row is not an available episode
- **WHEN** play or enqueue targets a show, loading state, empty state, or unavailable episode
- **THEN** mbv SHALL NOT create a QueueItem or playback session

### Requirement: Progress synchronization reports position and actual listening time
While an Audiobookshelf episode is active, the Player owner SHALL periodically synchronize current position, duration, and monotonic wall-clock listening time accumulated while mpv was actually playing. Paused time and seek distance SHALL NOT increase listening time, and an ambiguously dispatched interval SHALL NOT be counted again.

#### Scenario: Episode plays continuously
- **WHEN** playback remains active through a progress interval
- **THEN** mbv SHALL synchronize current position and elapsed playing time since the prior dispatch

#### Scenario: Episode pauses or seeks
- **WHEN** playback pauses or seeks
- **THEN** mbv SHALL synchronize the resulting position without counting paused time or seek distance

#### Scenario: Playback speed changes
- **WHEN** playback speed differs from 1.0
- **THEN** listening time SHALL continue to represent elapsed playing wall-clock time rather than media-position advancement

#### Scenario: Synchronization outcome is ambiguous
- **WHEN** a dispatched synchronization times out or loses its response
- **THEN** mbv SHALL NOT add that dispatched listening interval to a later request
- **THEN** playback SHALL continue with a concise redacted failure indication

### Requirement: Every opened playback session is finalized
One idempotent bounded lifecycle path SHALL synchronize final position/listening time and close every opened Audiobookshelf playback session before discarding it. Normal transitions SHALL complete or exhaust that bound before opening the next Audiobookshelf session; teardown SHALL never block indefinitely.

#### Scenario: Episode completes naturally
- **WHEN** an Audiobookshelf episode reaches natural completion
- **THEN** mbv SHALL finalize its session before advancing or stopping

#### Scenario: Active item leaves playback
- **WHEN** the user stops, skips, selects another slot, replaces the queue, or removes the active slot
- **THEN** mbv SHALL finalize the prior session before discarding its lifecycle

#### Scenario: Service or Player tears down
- **WHEN** Audiobookshelf is replaced or removed, the credential is rejected, the run shuts down, or the process exits
- **THEN** mbv SHALL finalize or abandon the active lifecycle within the existing teardown budget
- **THEN** teardown SHALL complete even if the server is unavailable

### Requirement: Owned playback refreshes local episode progress
Acknowledged mbv-owned Audiobookshelf progress SHALL update matching canonical queue slots and browse state by provider-qualified identity only while the captured setup generation remains current. This reconciliation SHALL NOT require polling or Socket.IO.

#### Scenario: Periodic synchronization succeeds
- **WHEN** Audiobookshelf accepts progress for the active episode
- **THEN** matching local queue and browse progress SHALL reflect the acknowledged values

#### Scenario: Episode completion succeeds
- **WHEN** final progress is accepted at natural completion
- **THEN** matching local state SHALL present the episode as finished

#### Scenario: Old generation reports late progress
- **WHEN** a progress completion belongs to a replaced or removed setup generation
- **THEN** mbv SHALL ignore it without updating current queue or browse state

### Requirement: Bare podcast playback adds no audiobook support or credential transport, and no Socket.IO remote control
Bare-mode Audiobookshelf playback SHALL NOT support audiobook media, transfer Service credentials between processes, or make Audiobookshelf items playable by a remote-only ctrl owner. The Audiobookshelf Socket.IO connection added by the `audiobookshelf-progress-refresh` capability SHALL NOT carry remote-control commands and SHALL NOT alter the in-process Player owner's own playback-session lifecycle, which remains driven exclusively by REST. Daemon owners that have negotiated transport capability MAY carry Audiobookshelf queue items and acknowledged progress over the capability-gated ctrl seam established by the transport child (#525); this is not a bare-mode concern.

#### Scenario: User plays podcasts in bare mode
- **WHEN** the user plays, seeks, pauses, completes, or stops downloaded Audiobookshelf episodes in bare mode
- **THEN** all Audiobookshelf credentials and playback-session lifecycle SHALL remain in the in-process Player owner
- **THEN** the active session's progress SHALL be driven only by REST synchronization, never by a Socket.IO event

#### Scenario: Daemon owner carries Audiobookshelf transport over ctrl
- **WHEN** a daemon owner with installed setup and a capable attached client plays an Audiobookshelf episode
- **THEN** Audiobookshelf credentials and playback-session lifecycle SHALL remain in the daemon owner
- **THEN** queue items and acknowledged progress MAY flow over the capability-gated ctrl seam to capable clients
- **THEN** no Audiobookshelf Socket.IO connection SHALL be opened by the daemon owner

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