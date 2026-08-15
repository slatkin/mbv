## Purpose

Defines provider-native queue identity, merged single-timeline mpv projection, chapter-row seeking, and listening-progress synchronization for Audiobookshelf books, isolated from the episode-shaped Audiobookshelf podcast playback capability.

## ADDED Requirements

### Requirement: A book is a distinct queue-item kind, isolated from podcast episode identity
mbv SHALL represent a queued Audiobookshelf book with its own `QueueItemKind` and content identity, keyed by the Audiobookshelf Service kind plus `libraryItemId` only. Book queue items, progress events, and progress reconciliation SHALL NOT share identity, a HashMap key, or a wire event shape with the episode-shaped Audiobookshelf podcast queue item, and matching one SHALL NOT match the other.

#### Scenario: A book and a podcast episode from the same server are both queued
- **WHEN** a book and a podcast episode from the same Audiobookshelf server are both present in a queue
- **THEN** each SHALL be identified, matched, and reconciled independently by its own kind
- **THEN** a progress event for one SHALL NOT update the other

### Requirement: Book activation uses ordinary play and enqueue semantics
A selected book SHALL support ordinary play and enqueue actions. Play SHALL select or create the corresponding queue slot and start it when the submission destination is eligible; enqueue SHALL add it without starting playback.

#### Scenario: User plays a selected book
- **WHEN** the user invokes ordinary play on a selected book toward an eligible Player owner
- **THEN** mbv SHALL place or select the book in the Bound queue as a single queue item and start that slot

#### Scenario: User enqueues a selected book
- **WHEN** the user invokes ordinary enqueue on a selected book
- **THEN** mbv SHALL add it through the canonical queue operation without opening a playback session or starting it

### Requirement: A book's audio files project as one continuous mpv timeline
mbv SHALL hand a book's `audioFiles` to mpv as a single merged timeline using mpv's native multi-file playlist/EDL projection, rather than opening and sequencing separate mpv items or computing per-file offsets in mbv. The book SHALL remain one queue item and one playback session across its entire runtime, including across its constituent files.

#### Scenario: Book has multiple audio files
- **WHEN** the selected book's `audioFiles` span more than one file
- **THEN** mbv SHALL project them as one continuous timeline without a queue-advance or playback-session boundary at each file transition

#### Scenario: Book has a single audio file
- **WHEN** the selected book has exactly one audio file
- **THEN** mbv SHALL play it directly without invoking multi-file projection

### Requirement: Chapter rows seek the merged book timeline
Activating a chapter row on the active book SHALL issue one absolute seek to that chapter's book-relative `start` offset on the merged timeline. Chapter seeking SHALL NOT require stopping and restarting the queue slot or reopening the playback session.

#### Scenario: User selects a later chapter
- **WHEN** the user activates a chapter row later than the current position on the active book
- **THEN** mbv SHALL seek to that chapter's `start` offset on the merged timeline
- **THEN** the queue slot and playback session SHALL remain the same

#### Scenario: User selects a chapter spanning a file boundary
- **WHEN** the target chapter's `start` offset falls in a different underlying audio file than the current position
- **THEN** the seek SHALL still resolve to the correct book-relative position without mbv computing the file offset itself

### Requirement: Book progress synchronization reports position without episode identity
While a book is active, the Player owner SHALL periodically synchronize current position, duration, and monotonic wall-clock listening time to Audiobookshelf using `libraryItemId` only. Paused time and seek distance SHALL NOT increase listening time, and an ambiguously dispatched interval SHALL NOT be counted again.

#### Scenario: Book plays continuously
- **WHEN** playback remains active through a progress interval
- **THEN** mbv SHALL synchronize current position and elapsed playing time since the prior dispatch, without an `episodeId` field

#### Scenario: Book pauses or seeks
- **WHEN** playback pauses or seeks, including a chapter-row seek
- **THEN** mbv SHALL synchronize the resulting position without counting paused time or seek distance

### Requirement: Every opened book playback session is finalized
One idempotent bounded lifecycle path SHALL synchronize final position/listening time and close every opened Audiobookshelf book playback session before discarding it, reusing the same finalization mechanics the podcast playback capability applies to its sessions.

#### Scenario: Book completes naturally
- **WHEN** an Audiobookshelf book reaches natural completion at the end of its merged timeline
- **THEN** mbv SHALL finalize its session before advancing or stopping

#### Scenario: Active book leaves playback
- **WHEN** the user stops, skips, selects another slot, replaces the queue, or removes the active slot
- **THEN** mbv SHALL finalize the prior session before discarding its lifecycle

### Requirement: Owned book playback refreshes local browse and queue progress
Acknowledged mbv-owned Audiobookshelf book progress SHALL update matching canonical queue slots and browse state by `libraryItemId` only, while the captured setup generation remains current. This reconciliation SHALL reuse the same generation-gated apply path as podcast progress reconciliation and SHALL NOT require polling or Socket.IO.

#### Scenario: Periodic synchronization succeeds
- **WHEN** Audiobookshelf accepts progress for the active book
- **THEN** matching local queue and browse progress SHALL reflect the acknowledged values

#### Scenario: Old generation reports late progress
- **WHEN** a progress completion belongs to a replaced or removed setup generation
- **THEN** mbv SHALL ignore it without updating current queue or browse state
