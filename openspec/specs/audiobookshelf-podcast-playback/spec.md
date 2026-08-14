# audiobookshelf-podcast-playback Specification

## Purpose

Defines secure bare-mode playback activation, owner eligibility, listening-progress synchronization, ordered playback-session finalization, and local progress reconciliation for downloaded Audiobookshelf podcast episodes.

## Requirements

### Requirement: Only the complete in-process Player capability binds episodes
Only the in-process bare-mode Player with current Audiobookshelf context, prepared-source support, and reporting/finalization support SHALL bind Audiobookshelf podcast episodes. Ctrl owners, Library routes, and Emby Sessions SHALL remain ineligible.

#### Scenario: Eligible bare owner binds an episode
- **WHEN** an Audiobookshelf episode is submitted to the complete in-process Player capability
- **THEN** it SHALL be eligible for that owner's Bound queue and active-source lifecycle

#### Scenario: Unsupported owner receives a submission
- **WHEN** an Audiobookshelf episode targets a Local daemon, remote Player owner, Library route, or Emby Session
- **THEN** submission SHALL fail visibly without Bound queue mutation or local fall-through

#### Scenario: Credential is rejected during playback
- **WHEN** Audiobookshelf explicitly rejects the current credential
- **THEN** the active session SHALL be finalized or abandoned within bounds and the runtime context SHALL clear
- **THEN** repairable Composed and persisted snapshots SHALL remain while Audiobookshelf items become ineligible for Bound queues

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

### Requirement: Bare podcast playback does not add daemon or live-update transport
This capability SHALL NOT send Audiobookshelf queue items over ctrl, transfer Service credentials between processes, connect to Audiobookshelf Socket.IO, support audiobook media, or make Audiobookshelf items playable by a Local daemon or remote Player owner.

#### Scenario: User plays podcasts in bare mode
- **WHEN** the user plays, seeks, pauses, completes, or stops downloaded Audiobookshelf episodes in bare mode
- **THEN** all Audiobookshelf credentials and playback-session lifecycle SHALL remain in the in-process Player owner
- **THEN** no ctrl capability or Socket.IO connection SHALL be required