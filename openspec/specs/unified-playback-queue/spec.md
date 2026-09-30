# unified-playback-queue Specification

<!-- #810: docs/invariants/02-pending-sync-protection.md -->

## Purpose
Define one queue and playback-submission model shared by every QueueItem across composed editing, Player ownership, local and ctrl control, persistence, and mpv playback.

## Requirements

### Requirement: A queue replacement deferred behind the unsaved-playlist prompt runs only through that prompt's answer

When a queue replacement would discard unsaved changes to the queue's saved playlist, the system SHALL hold the replacement and ask the user to save, discard, or cancel. Discard SHALL run the held replacement immediately. Cancel SHALL drop it. Save SHALL start one playlist save and bind the held replacement to that save. The held replacement SHALL run only when that bound save completes successfully for the same queue and playlist. If the bound save fails or cannot start, the held replacement SHALL be dropped. No other playlist save SHALL run it, including an automatic save after consume, a save on quit, or a later manual save. If the prompt is closed without an answer, the held replacement SHALL never run. Replacements held by the populated-queue confirmation and by the "play locally instead" prompt SHALL run only from their own prompt's confirmation.

#### Scenario: Save answer runs the replacement after its save

- **WHEN** the user answers Save on the unsaved-playlist prompt
- **AND** the save it started completes successfully
- **THEN** the held replacement SHALL run

#### Scenario: Prompt closed without an answer

- **WHEN** the unsaved-playlist prompt is closed by something other than the user's answer
- **AND** a later automatic save of the same playlist completes successfully
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

### Requirement: Each queue has one canonical ordered representation

Every Composed or Bound queue SHALL be represented by one ordered collection of queue slots containing `QueueItem` values. A Player owner SHALL hold the only authoritative collection for its Bound queue. A Client MAY hold a replaceable snapshot of a Bound queue, and a Playback run MAY hold an mpv execution projection, but neither SHALL independently decide canonical order, active slot, revision, or queue mutation outcome. A component SHALL NOT maintain parallel item-kind collections whose synchronization is required to determine queue contents, order, length, or current slot. An mpv projection MAY contain the full playable sequence or only the active materialized file as required by source lifecycle, but that projection SHALL NOT become queue authority. A populate-only queue replacement MAY update a Client's Composed queue without submitting it to an owner only when that Client does not present the queue as the Stay-alive process's queue. A Client presenting the Stay-alive queue SHALL submit a populate-only replacement to that owner at load time and SHALL display only owner-accepted contents, including the queue source. Until explicit play submits a Composed replacement to another playback target, the Client SHALL NOT present an unconfirmed slot from that replacement as the playing slot.

#### Scenario: Mixed queue order

- **WHEN** a queue contains interleaved Emby items, Feed entries, and Audiobookshelf podcast episodes
- **THEN** every queue operation and view SHALL observe the Player owner's canonical slot order
- **AND** no item kind SHALL be constrained to a prefix or tail

#### Scenario: Bound queue is viewed by a Client

- **WHEN** a Client displays or mutates a Bound queue
- **THEN** it SHALL use the Player owner's latest queue snapshot
- **AND** its local representation SHALL NOT become an independent queue authority

#### Scenario: Queue coordinates

- **WHEN** a queue reports its length or current position
- **THEN** both values SHALL use the canonical slot sequence regardless of how many files mpv has materialized

#### Scenario: Owner-driven active-file projection

- **WHEN** a Playback run uses owner-driven projection
- **THEN** mpv SHALL contain exactly the active materialized file while the canonical queue retains every slot
- **AND** mpv playlist position/count observations SHALL NOT resize, reorder, or reposition the canonical queue

#### Scenario: Eager mpv projection

- **WHEN** a Playback run materializes multiple canonical slots in mpv
- **THEN** the materialized entries SHALL remain an execution projection of owner-assigned slots
- **AND** mpv playlist mutation or position SHALL NOT independently redefine the Bound queue

#### Scenario: Populate-only load on Stay-alive

- **WHEN** a Client loads a playlist without starting playback while presenting the Stay-alive process's queue
- **THEN** the Client SHALL send the replacement to that owner at load time
- **AND** its displayed queue and source SHALL follow the owner's accepted snapshot, not a private staged queue

### Requirement: Queue occurrences have stable slot identity

Each occurrence of a `QueueItem` SHALL have stable runtime slot identity independent of its provider-qualified content identity or source URL. Operations on an existing queue occurrence SHALL target its slot identity. A slot identity assigned by a Player owner SHALL be used by the Client snapshot and Playback-run projection for that occurrence. An ordinal index MAY be used only as a presentation or mpv-adapter coordinate within the component that resolved it, and SHALL NOT address a queue occurrence across a component boundary or after the queue may have mutated. A Client-side queue replacement SHALL preserve monotonic slot allocation within its `PlaybackQueue`; replacement SHALL NOT re-mint an identity already issued by that tab, even when the old Playback run still contains that numeric slot. While a local owner is active, a Client SHALL compare its queue generation with `PlayerStatus.sequence_generation` before issuing a slot-addressed jump. On mismatch, explicit play SHALL submit the canonical queue at the selected cursor rather than issue `JumpTo`.

#### Scenario: Duplicate content occurrences

- **WHEN** the same `QueueItem` is appended twice
- **THEN** the queue SHALL contain two independently addressable slots

#### Scenario: Play an existing slot

- **WHEN** the user plays an item already present in the queue
- **THEN** playback SHALL select that slot
- **AND** SHALL NOT append another occurrence as a side effect

#### Scenario: Playback run reports an occurrence

- **WHEN** a Playback run reports which occurrence became active, completed, or stopped
- **THEN** it SHALL name the slot identity assigned by the Player owner
- **AND** the owner SHALL resolve that report without inferring the occurrence from an ordinal position

#### Scenario: Occurrence moves while a report is pending

- **WHEN** a queue occurrence changes ordinal position while a command or report naming it is in flight
- **THEN** the occurrence SHALL still resolve to the same slot
- **AND** no other occurrence SHALL be activated, completed, consumed, or removed in its place

### Requirement: Queue operations are item-kind agnostic

Append, replace, remove, move, clear, consume, and play-existing-slot operations SHALL accept every `QueueItem` kind and apply the same canonical ordering and mutation semantics. In owner-driven projection, inactive mutations SHALL update the canonical queue without requiring an inactive mpv playlist entry.

#### Scenario: Append an inactive item

- **WHEN** an item is appended after the active slot during owner-driven projection
- **THEN** it SHALL appear in canonical order without being prepared or inserted into mpv

#### Scenario: Append an Audiobookshelf episode

- **WHEN** an Audiobookshelf episode is appended to a Composed queue containing other item kinds
- **THEN** it SHALL be inserted using the same append operation as every other QueueItem
- **AND** subsequent ordinary mutations SHALL remain available

#### Scenario: Reorder a mixed queue

- **WHEN** a user moves an inactive item across another item during owner-driven projection
- **THEN** canonical queue state, UI, and persistence SHALL reflect the new order
- **AND** mpv SHALL continue representing only the active slot

#### Scenario: Active slot is selected or removed

- **WHEN** an explicit selection, removal, consume, skip, or natural completion changes the active canonical slot
- **THEN** the prior materialized file SHALL be finalized as required and replaced by the newly active slot

#### Scenario: Consume one duplicate

- **WHEN** one of two slots containing the same content is consumed
- **THEN** only the consumed slot SHALL be removed

### Requirement: Completion and consumption address the canonical slot

Natural completion and explicit consumption SHALL identify the affected canonical Queue slot and apply the queue's existing consume policy without branching by item kind. Content identity SHALL NOT be used to remove other occurrences. Consume SHALL be applied by the Player owner that holds the Bound queue, so the same completion produces the same queue outcome for Bare, Local daemon, and packaged `mbvd` ownership and whether or not a Client is attached.

#### Scenario: Feed slot completes naturally

- **WHEN** playback naturally completes a Feed entry whose slot is eligible for consumption
- **THEN** the Player owner SHALL consume that slot through the same slot-based queue operation used for an Emby item
- **AND** SHALL preserve any other slot containing the same Feed entry

#### Scenario: Slot is retained by policy

- **WHEN** playback completes a slot that the active consume policy retains
- **THEN** the slot SHALL remain in the canonical queue regardless of item kind

#### Scenario: Out-of-process owner consumes a completed slot

- **WHEN** a daemon Player owner completes a slot its consume policy removes
- **THEN** that owner's canonical queue SHALL no longer contain the slot
- **AND** attached Clients SHALL observe the shortened queue through the next owner snapshot

#### Scenario: Completion arrives with no Client attached

- **WHEN** a slot completes on a Player owner while no Client is attached
- **THEN** the consume policy SHALL be applied to the canonical queue
- **AND** a Client attaching afterwards SHALL observe the shortened queue

### Requirement: Playback submission uses one lifecycle-capable boundary

Every Player owner SHALL receive item-generic queue submissions through the same semantic boundary. The boundary SHALL start a cold Player, reuse or replace an active Player as required, enforce the destination owner's item and Service capabilities before binding, and report submission failure through the existing user-visible error path.

#### Scenario: Cold local owner

- **WHEN** a valid QueueItem is submitted to a capable in-process Player owner with no running playback process
- **THEN** the owner SHALL start playback for that item without requiring a pre-existing command channel

#### Scenario: Compatible directly controlled owner

- **WHEN** a valid QueueItem is submitted through a ctrl connection whose owner advertises every capability required by that item
- **THEN** the remote owner SHALL apply the same queue and lifecycle semantics as a local owner

#### Scenario: Submission cannot reach a capable owner

- **WHEN** the selected owner lacks an item-kind or Service capability required by the submission, or its command channel is unavailable
- **THEN** the submission SHALL fail visibly
- **AND** no component SHALL report the item as accepted into that owner's Bound queue

### Requirement: A Player owner binds only playable items

Owner admission SHALL evaluate every `QueueItem` through canonical media-kind and required-Service classification. An owner SHALL never bind an item whose media kind or required Remote Service capability it cannot play. A daemon Player owner (Local daemon or packaged `mbvd`) SHALL admit Audiobookshelf `QueueItem` variants only when its owner-scoped Audiobookshelf setup is installed and it has negotiated Audiobookshelf transport capability with the submitting client. Existing Composed-to-Bound stripping and explicit-submission behavior SHALL apply at binding without constraining Composed queue editing.

#### Scenario: Audio Feed entry submitted to an audio-only owner

- **WHEN** a Feed entry classified as Audio is submitted to an audio-only owner
- **THEN** it SHALL be eligible for that owner's Bound queue under the same rules as an audio Emby item

#### Scenario: Video Feed entry submitted to an audio-only owner

- **WHEN** a Feed entry classified as Video is explicitly submitted while directly controlling an audio-only owner
- **THEN** it SHALL follow the same local fall-through behavior as a video Emby item
- **AND** SHALL NOT enter the audio-only owner's queue

#### Scenario: Feed MIME is absent

- **WHEN** a Feed entry has no usable enclosure MIME type
- **THEN** its queued snapshot SHALL retain the subscription's `FeedKind` as its canonical media kind

#### Scenario: Owner lacks an item's Remote Service capability

- **WHEN** a queue containing an item from a Remote Service binds to an owner without that Service capability
- **THEN** that item SHALL be unplayable and SHALL NOT enter the owner's Bound queue
- **AND** other playable items SHALL remain eligible

#### Scenario: Audiobookshelf episode submitted to daemon owner with installed setup and transport capability

- **WHEN** an Audiobookshelf podcast episode is submitted to a daemon owner that has installed Audiobookshelf setup and has negotiated Audiobookshelf transport capability
- **THEN** the episode SHALL be eligible for that owner's Bound queue under the same canonical queue semantics as every other admitted QueueItem

#### Scenario: Audiobookshelf episode submitted to daemon owner without installed setup

- **WHEN** an Audiobookshelf podcast episode is submitted to a daemon owner that has no installed Audiobookshelf setup
- **THEN** the submission SHALL fail visibly without Bound queue mutation

### Requirement: The Player branches only at source and reporting boundaries

The playback pipeline SHALL treat all admitted queue slots uniformly through ordering, lifecycle, status, and queue management. Item-kind branching SHALL occur only to resolve the active media source and to select progress-reporting behavior. Resolution MAY be just in time when the source requires an active server lifecycle.

#### Scenario: Resolve an Emby item

- **WHEN** an Emby item reaches the play boundary
- **THEN** the Player owner SHALL resolve its authenticated Emby stream URL
- **AND** SHALL use Emby playback reporting

#### Scenario: Resolve a Feed entry

- **WHEN** a Feed entry reaches the play boundary
- **THEN** the Player owner SHALL resolve its enclosure URL or fallback link directly
- **AND** SHALL NOT report progress to Emby

#### Scenario: Resolve an Audiobookshelf episode

- **WHEN** an Audiobookshelf podcast episode becomes active on the eligible in-process Player owner
- **THEN** that owner SHALL create and own its Audiobookshelf playback session before resolving its source
- **AND** SHALL use Audiobookshelf playback-session progress reporting

### Requirement: Bound queue state synchronizes atomically

A Player owner SHALL publish its canonical queue revision, ordered slots, observed active slot, playback status, and pending transition as one coherent snapshot. Initial connection, mutation, playback observation, transition settlement, and reconnect SHALL use that representation. A Client SHALL replace its prior Bound-queue snapshot atomically and SHALL NOT reconcile independently delivered queue and playback coordinates into a second answer.

#### Scenario: Reconnect to a mixed Bound queue

- **WHEN** a compatible Client reconnects to an owner holding a mixed queue
- **THEN** it SHALL reconstruct the same slots, order, observed active slot, playback status, and pending transition from one owner snapshot

#### Scenario: Playback changes during a queue mutation

- **WHEN** the queue revision and observed active slot both change during one owner event-loop turn
- **THEN** Clients SHALL receive values from the same resulting owner state
- **AND** SHALL NOT temporarily pair the new queue with the previous active coordinate

#### Scenario: Client receives an owner snapshot

- **WHEN** a Client receives an owner snapshot in delivery order, regardless of the revision number carried by the owner
- **THEN** it SHALL atomically replace its previous Bound-queue snapshot and mint a Client-local queue revision for the adopted state
- **AND** SHALL NOT compare the owner's revision to order snapshots
- **AND** SHALL NOT preserve an optimistic active slot from the previous snapshot

#### Scenario: Player reports a slot change

- **WHEN** mpv advances to any slot in a mixed queue
- **THEN** the Player owner and connected Client SHALL report that slot using the canonical Queue slot identity
- **AND** the observation SHALL be published with the matching queue revision and playback status

### Requirement: Queue persistence round-trips every QueueItem

Persisted queue state SHALL serialize the canonical tagged `QueueItem` sequence and restore every supported item kind in the same order. Persisted items SHALL exclude Service credentials and ephemeral playback state. Legacy untagged Emby-only state SHALL remain readable.

#### Scenario: Restore a mixed queue

- **WHEN** persisted state contains Emby items, Feed entries, and Audiobookshelf podcast episodes
- **THEN** restoration SHALL preserve each slot's item kind, provider-qualified content identity, ordering, and playback fields
- **THEN** owner admission SHALL run before restored slots enter a Bound queue

#### Scenario: Restore legacy state

- **WHEN** persisted state contains the legacy untagged Emby-item shape
- **THEN** restoration SHALL interpret those values as Emby queue items without error

#### Scenario: Inspect persisted Audiobookshelf item

- **WHEN** an Audiobookshelf podcast episode is persisted
- **THEN** its representation SHALL contain no Service credential, playback `sessionId`, resolved URL, or request header

### Requirement: Unified ctrl behavior is capability-gated and additive

The ctrl protocol SHALL advertise additive capabilities for every QueueItem kind transported through unified queue state and operations without changing `CTRL_PROTOCOL_VERSION`. A QueueItem kind without a negotiated transport capability SHALL remain ineligible for that peer's Bound queue. Compatibility handling SHALL remain confined to the ctrl boundary and SHALL NOT create a second internal queue model.

#### Scenario: Both peers support an item's queue transport

- **WHEN** both ctrl peers advertise the capabilities required by every submitted QueueItem
- **THEN** queue state and operations SHALL carry the tagged QueueItem values and their canonical order

#### Scenario: Audiobookshelf transport is not negotiated

- **WHEN** a queue contains an Audiobookshelf episode and no Audiobookshelf transport capability is negotiated
- **THEN** that episode SHALL NOT be submitted to or represented as Bound by that owner
- **AND** no Audiobookshelf credential SHALL cross ctrl

#### Scenario: Legacy peer connects

- **WHEN** a peer does not advertise the unified queue capability
- **THEN** it SHALL retain its existing representable behavior through a compatibility adapter
- **AND** the owner SHALL continue to hold one canonical internal queue

### Requirement: Audiobookshelf queue transport is separately capability-gated
Audiobookshelf podcast items SHALL cross the unified ctrl queue boundary only when both peers support the additive Audiobookshelf queue capability. The capability SHALL describe static protocol support and SHALL NOT make a daemon owner eligible to bind or play the item.

#### Scenario: Both peers support Audiobookshelf queue transport
- **WHEN** a unified-queue operation or snapshot contains an Audiobookshelf podcast episode and both peers advertise Audiobookshelf queue support
- **THEN** the wire representation SHALL carry the provider-qualified episode in canonical slot order

#### Scenario: Audiobookshelf queue capability is absent
- **WHEN** either peer lacks Audiobookshelf queue support
- **THEN** the episode SHALL NOT be sent to or represented as Bound by that peer
- **THEN** every previously supported QueueItem kind SHALL retain existing behavior

### Requirement: Every queue transport direction applies compatibility gating
Audiobookshelf capability checks SHALL apply to incoming unified queue commands, initial owner snapshots, later owner broadcasts, and reconnect adoption. A compatible internal queue SHALL remain canonical and SHALL NOT be replaced by an item-kind-specific queue model.

#### Scenario: Older unified peer connects to an owner holding an episode
- **WHEN** a peer supports unified queues but not Audiobookshelf queue transport
- **THEN** it SHALL receive no Audiobookshelf QueueItem variant
- **THEN** the owner SHALL retain one canonical internal queue

#### Scenario: Older peer submits an unsupported episode
- **WHEN** a peer without negotiated Audiobookshelf queue support submits an Audiobookshelf QueueItem
- **THEN** the owner SHALL reject the unsupported operation without mutating its Bound queue

### Requirement: Audiobookshelf queue transport carries no lifecycle secrets
Audiobookshelf unified queue commands and snapshots SHALL contain provider-qualified media identity and ordinary queue metadata but SHALL NOT contain an API key, Authorization header, resolved source URL, or playback `sessionId`.

#### Scenario: Capable client receives an episode slot
- **WHEN** a capable client receives queue state containing an Audiobookshelf episode
- **THEN** it SHALL receive stable episode and slot identity without owner credentials or ephemeral playback state

### Requirement: Transport does not enable daemon owner admission
During this change every daemon Player owner SHALL continue treating Audiobookshelf podcast episodes as unplayable even when queue transport is negotiated. Transported values MAY be decoded and compatibility-filtered but SHALL NOT enter a daemon Bound queue or start playback.

#### Scenario: Capable peers negotiate transport before activation
- **WHEN** a client submits an Audiobookshelf episode to a daemon owner after this change
- **THEN** the owner SHALL visibly reject admission without source preparation or Bound queue mutation

### Requirement: A later client adopts the live daemon Audiobookshelf queue and progress
A capable client attaching to a daemon that owns active Audiobookshelf playback SHALL adopt the daemon's live canonical queue, active slot, playback status, and last-acknowledged Audiobookshelf progress as authoritative, and SHALL NOT overwrite that daemon authority with a saved local or shared queue snapshot. Adopted Audiobookshelf slots SHALL carry provider-qualified identity in canonical slot order and SHALL reconcile browse state on adoption.

#### Scenario: Client attaches while the daemon holds an Audiobookshelf queue
- **WHEN** a capable client attaches to a daemon whose canonical queue contains one or more Audiobookshelf episodes
- **THEN** the client SHALL adopt the live queue, active slot, and status rather than its persisted snapshot

#### Scenario: A stale saved snapshot is present at attach
- **WHEN** the attaching client holds a saved local or shared queue snapshot that differs from the daemon's live Audiobookshelf queue
- **THEN** the daemon's live queue SHALL win and the client SHALL NOT push its snapshot as authoritative

#### Scenario: Incapable peer attaches to an Audiobookshelf-holding owner
- **WHEN** a peer that did not negotiate Audiobookshelf queue transport attaches to an owner holding Audiobookshelf slots
- **THEN** it SHALL receive no Audiobookshelf QueueItem variant and every previously supported queue behavior SHALL continue

### Requirement: Service-specific refresh preserves unrelated queue-item kinds
A refresh sourced from one Service SHALL update or prune only queue slots that belong to that Service. It SHALL preserve every slot belonging to another Service or to Feeds, including both Audiobookshelf podcast episodes and Audiobookshelf books, without attempting to resolve their identities through the refreshing Service.

#### Scenario: Emby refresh observes an Audiobookshelf book
- **WHEN** an Emby refresh merges into a queue containing an inactive Audiobookshelf book
- **THEN** the book slot SHALL remain in the same canonical position
- **AND** SHALL NOT be reported as pruned because it has no Emby identity

#### Scenario: Emby refresh observes both Audiobookshelf shapes
- **WHEN** an Emby refresh merges into a mixed queue containing an Audiobookshelf podcast episode and a book
- **THEN** both Audiobookshelf slots SHALL remain unchanged
- **AND** Emby-owned slots SHALL continue to reconcile normally

### Requirement: Desired and observed playback remain distinct
A request to play a Queue slot SHALL create desired transition state and SHALL NOT itself change the observed active slot. The Player owner SHALL change the observed active slot only from a Playback-run observation naming an owner-assigned slot. User-visible playback state SHALL identify observed playback separately from any pending desired slot. When a queue replacement installs a new canonical queue, the Player owner SHALL clear the observed active slot so that stale slot identities from the prior queue do not influence subsequent navigation resolution.

#### Scenario: Slot selection is accepted
- **WHEN** a valid request selects a different Queue slot
- **THEN** the Player owner SHALL record the requested slot as pending
- **AND** the previously observed slot SHALL remain observed until the Playback run reports a transition

#### Scenario: Requested slot starts
- **WHEN** the Playback run reports the requested slot under the matching transition identity
- **THEN** the Player owner SHALL make that slot the observed active slot
- **AND** SHALL settle the request as applied

#### Scenario: Intermediate slot is observed
- **WHEN** a superseded transition briefly starts before the latest requested transition
- **THEN** the owner snapshot SHALL report that slot as observed playback
- **AND** SHALL retain the newer desired transition as pending

#### Scenario: Queue replacement clears observed state
- **WHEN** a queue replacement installs a new canonical queue while playback from the prior queue was observed
- **THEN** the Player owner SHALL clear the observed active slot
- **AND** navigation resolution SHALL fall back to the new queue's active slot until a Playback-run observation from the new queue arrives

### Requirement: Playback transitions are serialized with latest-wins queuing

Each explicit playback transition SHALL have monotonic request identity. A Player owner SHALL dispatch at most one transition to its Playback run at a time and SHALL retain at most one undispatched transition, replacing that queued transition when a newer request arrives. A Playback-run observation SHALL carry the identity of the single dispatched transition it settles; an observation with an older identity SHALL NOT settle or overwrite a newer request.

#### Scenario: Two requests arrive before confirmation

- **WHEN** a second transition request arrives while the first is in flight
- **THEN** the first SHALL remain the sole dispatched transition
- **AND** the second SHALL become the latest queued transition

#### Scenario: Three rapid requests arrive

- **WHEN** two newer transition requests arrive while one transition is in flight
- **THEN** only the newest undispatched request SHALL remain queued
- **AND** every displaced queued request SHALL be reported as superseded

#### Scenario: In-flight transition settles after a newer request

- **WHEN** the Playback run reports the identity and target of an older in-flight transition
- **THEN** that observation MAY update observed playback
- **AND** SHALL NOT settle the newer queued request
- **AND** the Player owner SHALL then dispatch the newest queued transition

#### Scenario: Same slot is requested again

- **WHEN** requests form an A-to-B-to-A sequence before all transitions settle
- **THEN** a late observation of the first A request SHALL carry its older identity
- **AND** SHALL NOT be accepted as confirmation of the newer A request

### Requirement: Stale slot addressing is rejected, not reinterpreted

A queue command or report that names a slot the receiving component no longer holds SHALL be rejected without mutating the canonical queue. A component SHALL NOT substitute a neighbouring slot, clamp to the nearest position, or fall back to a remembered position when the named slot is absent. Client-initiated rejection SHALL be observable through the existing command-rejection path; stale internal observations SHALL be discarded without user-facing noise.

#### Scenario: Mutation and command cross in flight

- **WHEN** a slot is removed from the canonical queue while a command addressing that slot is in flight
- **THEN** the command SHALL be rejected
- **AND** no other slot SHALL be removed, moved, or activated as a result

#### Scenario: Queue shrinks beneath an in-flight report

- **WHEN** a Playback run reports a slot that the Player owner's queue no longer contains
- **THEN** the owner SHALL discard the report without changing its observed active slot
- **AND** SHALL NOT clamp the report onto the nearest surviving slot

#### Scenario: Rejected mutation is surfaced

- **WHEN** a Client's queue mutation is rejected because its target slot is gone
- **THEN** the Client SHALL be told the mutation did not apply
- **AND** the Client SHALL adopt the Player owner's current snapshot

### Requirement: Near-end completion uses one rule

The decision that playback finished close enough to the end to count as completed SHALL be evaluated by one rule applied to the completed occurrence's own runtime. Every completion path, including ordinary advance, end of queue, quit, and process shutdown, SHALL reach the same verdict for the same completed occurrence, position, and media kind.

#### Scenario: Same completion, different exit path

- **WHEN** the same occurrence at the same position ends through natural advance, quit, or owner shutdown
- **THEN** each path SHALL produce the same near-end verdict
- **AND** the same watched-state and Consume outcome SHALL follow

#### Scenario: Runtime belongs to the completed occurrence

- **WHEN** live playback status already describes the next occurrence when completion is evaluated
- **THEN** the near-end verdict SHALL use the completed occurrence's runtime
- **AND** SHALL NOT use the replacement occurrence's runtime

### Requirement: Queue loading SHALL not flash a wrong track
When a Player loads a queue at a non-zero start index, the Playback run SHALL NOT briefly play or report a different slot before the intended start slot begins. The mpv playlist construction SHALL ensure that the intended start slot is the one that plays from the first audible moment.

#### Scenario: Queue loaded at middle position
- **WHEN** a queue of 100 items is loaded at start index 50
- **THEN** the Playback run SHALL begin playback of item 50 without first starting item 0
- **AND** observers SHALL not receive a transient active-slot report for any slot other than 50

#### Scenario: Queue loaded at index 0
- **WHEN** a queue is loaded at start index 0
- **THEN** playback SHALL begin at item 0 with no behavioral difference from a non-zero start

### Requirement: A Player owner refreshes progress on a cold-adopted persisted queue

When a Player owner (Local daemon or packaged `mbvd`) has no queue of its own
and adopts a client's persisted queue snapshot, it SHALL treat that
snapshot's per-item progress as provisional. It SHALL asynchronously refresh
progress for the adopted items against the owning Service without blocking
playback of the adopted queue, apply the refreshed values to its own
canonical queue (not only to a client-side snapshot), and broadcast the
refreshed queue to attached clients once applied.

#### Scenario: Cold daemon adopts a persisted queue with server-side progress

- **WHEN** a Local daemon with no existing queue receives an adoption request
  carrying a persisted snapshot whose items have resume progress on the
  owning Service
- **THEN** the daemon fetches current progress for those items from the
  Service asynchronously, merges it into its own canonical queue, and
  broadcasts the refreshed queue to attached clients — without requiring any
  item to be played first

#### Scenario: Adopted item is played before the refresh completes

- **WHEN** the user plays an item from the adopted queue before the daemon's
  asynchronous refresh has finished
- **THEN** the play-driven progress-application path is authoritative for
  that item's canonical position, and a refresh result that arrives
  afterward does not overwrite it with older data

#### Scenario: Refresh is scoped to the owning Service

- **WHEN** the adopted queue contains items from more than one Service, or
  Feed entries
- **THEN** the refresh updates or prunes only the slots belonging to the
  Service it queried, and leaves every other slot's progress untouched

#### Scenario: Refresh does not regress adopted positions

- **WHEN** the asynchronous adoption-time refresh returns UserData for an
  adopted slot whose stored position is greater than the fetched one
- **THEN** the slot keeps its greater stored position — unless the fetched
  item reports the slot as played, in which case the fetched state is adopted
  verbatim — and a fetched position greater than the stored one still updates
  the slot

### Requirement: Stay-alive replacement stops playback before publishing the new queue

When a Client loads a playlist into the Stay-alive process's queue without requesting playback, that owner SHALL stop and finalize the prior playing item, replace its entire Bound queue and Queue source with the admitted playlist and source, clear the observed active slot and pending playback transitions, and publish a stopped snapshot with no playing row. The replacement SHALL preserve a valid selected queue cursor for subsequent explicit Play but SHALL NOT start playback. The owner SHALL NOT publish a state in which an item is playing outside its Bound queue. A load rejected before acceptance SHALL NOT replace or stop the existing queue. If delivery is uncertain because the connection fails in flight, the Client SHALL reconcile from the owner rather than assume either outcome.

#### Scenario: Load while old item plays

- **WHEN** the owner is playing an old queue and a Client loads a different playlist without autostart
- **THEN** the owner SHALL stop the old item and replace its queue and source with the loaded playlist
- **AND** every Client SHALL observe the new queue as stopped with no playing slot
- **AND** the old item SHALL NOT resume or appear as playing in the new queue

#### Scenario: Play after a successful idle load

- **WHEN** a Client explicitly plays a slot from the newly loaded queue
- **THEN** the owner SHALL start that owner-assigned slot without resubmitting a private copy of the queue

#### Scenario: A late observation from the old Playback run arrives

- **WHEN** a prior run reports progress, completion, or a track change after the replacement is accepted
- **THEN** the report SHALL NOT mark a new-queue slot as playing, completed, or consumed

#### Scenario: Empty playlist load

- **WHEN** a Client loads an empty playlist into the Stay-alive queue
- **THEN** the owner SHALL stop playback, leave its queue empty, and publish no active slot

#### Scenario: Rejected replacement

- **WHEN** a requested idle replacement is rejected before acceptance
- **THEN** the existing owner queue and playback SHALL remain authoritative
- **AND** the Client SHALL display an error rather than claiming the playlist was loaded

### Requirement: The Stay-alive process holds the queue source

The Stay-alive process SHALL hold the queue source as part of its queue state. Every whole-queue replacement it accepts, every clear, and every source-only update SHALL set its source; a clear SHALL reset it to Unknown. A source-only update SHALL apply only to the queue lineage the owner held when the update was requested; a delayed update from an earlier queue SHALL NOT rename a later queue. A Client attached to that owner SHALL display the owner's source and SHALL NOT maintain an independent authoritative source for it. Saving the queue as a new playlist (Save As) and overwriting an existing playlist with the queue SHALL both reach the owner as source-only updates carrying the lineage observed when the save was requested; the Client SHALL report the queue clean only once an owner snapshot shows the new source. A Client that has not yet received an owner snapshot SHALL refuse to save the queue to a playlist, and SHALL create or change no server playlist.

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
- **THEN** the Client SHALL send the owner a source-only update naming the replacement playlist, carrying the lineage observed when the overwrite was requested
- **AND** the queue SHALL stay dirty until an owner snapshot with that source arrives

#### Scenario: No owner snapshot refuses a playlist save

- **WHEN** a Client attached to the Stay-alive process has received no owner queue snapshot and the user saves, saves as, or overwrites a playlist
- **THEN** the Client SHALL show an error
- **AND** no server playlist SHALL be created, updated, or deleted

### Requirement: User-initiated queue replacements confirm before replacing a populated queue
When the user explicitly picks content to replace the playback queue, the system SHALL ask for confirmation before replacing that queue if it is populated. An empty target queue SHALL be replaced without the prompt. This applies to album and artist track plays, playlist loads, shuffle-folder plays, context-menu Play and Shuffle, and grouped-tree track plays. It SHALL NOT apply to library autoplay, single-item play, or replays that restore existing session state. Cancelling SHALL leave the queue and playback unchanged. After confirmation, the replacement SHALL behave exactly as it did before this gate existed, including any later "play locally instead" or unsaved-playlist prompt.

#### Scenario: Album track play over a populated queue
- **WHEN** the queue is populated and the user plays a track from an album
- **THEN** confirmation is requested before the queue is replaced
- **AND** cancelling leaves the queue and playback unchanged

#### Scenario: Playlist load over a populated queue
- **WHEN** the queue is populated and the user loads a playlist
- **THEN** confirmation is requested before the playlist replaces the queue

#### Scenario: Empty queue plays immediately
- **WHEN** the queue is empty and the user plays an album track, shuffles a folder, or loads a playlist
- **THEN** no replace-queue confirmation is shown

#### Scenario: Confirmation precedes later prompts
- **WHEN** the queue is populated and the chosen items cannot play on the attached owner
- **THEN** the replace-queue confirmation is shown first
- **AND** only after confirming is the "play locally instead" prompt shown

#### Scenario: Autoplay and single-item play are not gated
- **WHEN** the queue is populated and library autoplay starts, or the user plays a single movie or episode
- **THEN** no replace-queue confirmation is shown

### Requirement: Recorded progress follows one rule per observation kind

Each Player owner and each client copy of the canonical queue SHALL decide the position to record for a finished occurrence using one rule per observation kind, and every copy SHALL reach the same result for the same observation:

- A **completion** observation (the occurrence ended and playback moved on) SHALL record position zero when the occurrence counts as played. Otherwise it SHALL record the observed position only for non-audio media at or beyond 30 seconds, and SHALL keep the previously recorded position in every other case.
- A **stop** observation (the user stopped playback) SHALL record position zero when the occurrence counts as played. Otherwise it SHALL record any positive observed position for non-audio media, and SHALL keep the previously recorded position for audio or a non-positive position.

#### Scenario: Video completes under 30 seconds

- **WHEN** a video occurrence with a recorded position of 20 minutes ends by moving on at 12 seconds without counting as played
- **THEN** every copy of the queue SHALL keep 20 minutes as that occurrence's position

#### Scenario: Audio completes mid-track

- **WHEN** an audio occurrence ends by moving on at 3 minutes without counting as played
- **THEN** every copy of the queue SHALL keep that occurrence's previous position

#### Scenario: Video stopped under 30 seconds

- **WHEN** the user stops a video occurrence at 12 seconds without it counting as played
- **THEN** every copy of the queue SHALL record 12 seconds for that occurrence

#### Scenario: Played occurrence

- **WHEN** an occurrence counts as played when its completion or stop is observed
- **THEN** every copy of the queue SHALL record position zero for it

### Requirement: An accepted stop report protects reported progress until the server confirms it

When a Playback run's stop or completion report for an Emby queue slot is accepted, the queue holding that slot SHALL record the reported position and watched state as awaiting server confirmation. This SHALL hold for the Shell's queue and for every Player owner's canonical queue alike. Until the server confirms it, an Emby refresh SHALL NOT replace that slot's progress with the fetched values and SHALL NOT prune the slot. A refresh SHALL confirm the recorded progress only when the fetched position is within three seconds of it and the fetched watched state is equal. Confirmation SHALL end the protection and adopt the fetched item. A report that was not accepted SHALL still apply its progress to the slot, but SHALL NOT arm protection. Feed and Audiobookshelf slots SHALL never be armed. Replacing a slot's item with different content SHALL end that slot's protection; replacing the same content's metadata SHALL keep it. No other path SHALL end protection.

#### Scenario: Refresh lands before the server applies an accepted stop

- **WHEN** an accepted stop report records a position for an inactive Emby slot
- **AND** an Emby refresh then returns that item with the older server position
- **THEN** the slot SHALL keep the reported position
- **AND** the refresh SHALL NOT prune the slot even if the item is missing from the fetched results

#### Scenario: Server confirms the reported progress

- **WHEN** a later refresh returns the item within three seconds of the reported position and with the same watched state
- **THEN** the slot SHALL adopt the fetched item
- **AND** a subsequent refresh SHALL merge that slot normally

#### Scenario: Position matches but watched state does not

- **WHEN** a near-end stop reported the item as watched
- **AND** a refresh returns it unwatched at a position within three seconds
- **THEN** the refresh SHALL NOT count as confirmation and the slot SHALL keep its reported state

#### Scenario: Player owner receives a report that was not accepted

- **WHEN** the Stay-alive process or `mbvd` observes a stop or completion whose report was not accepted
- **THEN** it SHALL apply the observed progress to its canonical queue
- **AND** SHALL NOT protect the slot from a later refresh

#### Scenario: Non-Emby slot stops with an accepted report

- **WHEN** a Feed or Audiobookshelf slot's stop is observed
- **THEN** the slot's progress SHALL be applied
- **AND** the slot SHALL NOT be recorded as awaiting server confirmation

#### Scenario: Slot metadata is rewritten while protected

- **WHEN** a protected slot's item is replaced by the same content with different metadata
- **THEN** the slot SHALL remain protected with the same recorded progress

### Requirement: A slot resumes the same however it is reached

The resume position used when an occurrence starts SHALL come from the Player owner's canonical queue, and SHALL be the same whether the occurrence is reached by Next, Previous, a direct jump, or an on-screen Next-Up accept. This applies to full-playlist and active-file playback alike. Audiobookshelf items SHALL resume from the Audiobookshelf playback session's position, which remains authoritative for that service.

#### Scenario: Previous back to a video left under 30 seconds

- **WHEN** a video occurrence with a recorded position of 20 minutes is played for 12 seconds, playback moves to the next slot, and the user invokes Previous
- **THEN** the video SHALL resume at 20 minutes, the same position a direct jump to that slot would use

#### Scenario: Previous back to an audio track

- **WHEN** an audio occurrence is left mid-track by Next and the user invokes Previous
- **THEN** it SHALL start from the same position a direct jump to that slot would use

#### Scenario: Non-Audiobookshelf item in an active-file queue

- **WHEN** a queue plays in active-file mode and the user jumps to a non-Audiobookshelf occurrence
- **THEN** that occurrence SHALL resume from the canonical queue's position for it

### Requirement: Relative navigation is resolved by the daemon Player owner

Next and Previous SHALL be resolved by the Player owner against its canonical queue, whatever sent them: keyboard, mouse, MPRIS, the tray, or an Emby remote-control command. This SHALL hold for the local daemon, whatever `stay_alive` is set to, and for packaged `mbvd`. The owner SHALL turn the step into a slot jump with its own request identity, dispatched and settled like any other slot jump. The Playback run SHALL NOT pick a neighbouring slot itself.

The neighbour SHALL be found from the latest requested target: the queued transition's target, else the in-flight transition's target, else the observed active slot. Next SHALL do nothing at the last slot and Previous SHALL do nothing at the first slot.

#### Scenario: MPRIS Next while attached to a Stay-alive daemon

- **WHEN** the TUI is attached to a Stay-alive daemon and the user invokes Next through MPRIS
- **THEN** the daemon's Player owner SHALL advance to the next canonical slot
- **AND** the request SHALL NOT be refused for lacking a ctrl wire form

#### Scenario: Emby remote Next under Stay-alive

- **WHEN** an Emby remote-control Next arrives at a Stay-alive daemon
- **THEN** the daemon's Player owner SHALL resolve the neighbour and dispatch a slot jump
- **AND** the target SHALL resume from the canonical queue

#### Scenario: Tray Next

- **WHEN** the user selects Next from the Local daemon's tray
- **THEN** the daemon's Player owner SHALL resolve and dispatch the step
- **AND** every attached client SHALL observe the resulting track change

#### Scenario: Keyboard Next

- **WHEN** the user presses Next in a Client attached to the local daemon
- **THEN** the local daemon SHALL resolve the neighbour and dispatch a slot jump with canonical resume

#### Scenario: Previous right after Next

- **WHEN** Next has been requested and not yet confirmed, and the user invokes Previous
- **THEN** Previous SHALL resolve from the requested Next target
- **AND** playback SHALL return to the slot that was playing before Next

#### Scenario: Next at the end of the queue

- **WHEN** the latest requested target is the last canonical slot and Next is invoked
- **THEN** no transition SHALL be dispatched

### Requirement: Client-initiated slot jumps are requested from the Player owner

A Client SHALL request every user-initiated jump to a canonical slot from the Player owner that
holds the queue, using the owner-resolved slot request. The terminal UI process is never a Player
owner, so a Client SHALL NOT resolve a jump locally. A Client SHALL NOT construct or transmit a
command that the ctrl transport has no wire form for. On completion of a requested jump, the
Client SHALL NOT set its own active slot from the requested slot; the active slot SHALL continue
to follow the owner's queue snapshot.

#### Scenario: Owner accepts the on-screen Next-Up accept action

- **WHEN** the mpv Next-Up accept affordance is activated
- **THEN** the Client SHALL request the jump from the Player owner
- **AND** the requested slot SHALL become active only through the owner's queue snapshot
- **AND** the Client process SHALL remain running

#### Scenario: Every client-initiated jump uses the same dispatch

- **WHEN** any client-initiated action jumps to an existing canonical slot
- **THEN** it SHALL be requested from the Player owner rather than constructing a local jump
  command directly

### Requirement: Queue edits reach the owner that holds the queue

A Client SHALL dispatch every canonical-queue edit (slot removal, slot move, item append or insert, clear, refresh) to the Player owner that holds the queue being edited: the local daemon for Local scope, or the directly controlled remote Player owner for Remote scope. A Client has no canonical queue of its own: its displayed Local queue is the local daemon's Bound queue in every state, including while an attached Emby session or a cast receiver is the playback target, so its Local-scope edits SHALL reach that owner. Direct-remote queue management is unchanged: edits addressed to Remote scope reach that owner.

#### Scenario: Direct-remote edits still reach the owner

- **WHEN** the user edits a directly controlled remote Player owner's queue in Remote scope
- **THEN** the edit SHALL still be dispatched to that owner as a slot-addressed command

#### Scenario: Local edit while a session or cast is the target

- **WHEN** a Client is watching an attached Emby session or casting to a receiver and edits the Local queue
- **THEN** the edit SHALL be dispatched to the local daemon
- **AND** SHALL NOT be applied only to a private Client copy

### Requirement: Clients hold no editable queue

A Client's displayed queue for each queue scope SHALL change only by adopting a snapshot from the Player owner that holds that queue. A Client SHALL NOT insert, remove, move, replace, clear, refresh-merge, consume, or mark progress on a queue it displays, SHALL NOT predict an edit's or a playback transition's outcome, and SHALL NOT persist, restore, or seed a queue.

#### Scenario: Edit awaiting the owner

- **WHEN** the user removes a queue entry and the owner has not yet answered
- **THEN** the displayed queue SHALL still show the owner's last accepted state

#### Scenario: Owner rejects an edit

- **WHEN** the owner rejects a queue edit
- **THEN** the Client SHALL report that the edit did not apply
- **AND** the displayed queue SHALL be the owner's current state, with nothing to roll back

#### Scenario: Queue refresh

- **WHEN** the user refreshes the queue
- **THEN** the Client SHALL ask the owner to refresh its queue
- **AND** the refreshed items SHALL appear only through the owner's resulting snapshot

### Requirement: Queue edits are answered before the next input

Every queue edit a Client sends, other than loading a playlist without starting playback, SHALL carry an identity, and the Player owner SHALL answer the sending Client with that identity and either the resulting snapshot or a rejection; other attached Clients SHALL receive the resulting snapshot as usual. The Client SHALL adopt the answer before handling its next input, so an accepted edit is visible in the next frame after the owner applies it. The Client SHALL wait for the answer for a bounded time only; when the bound passes, it SHALL report that the owner did not respond, keep showing the owner's last accepted state, and continue to adopt later snapshots, including a late answer to that edit. Loading a playlist without starting playback SHALL keep its own load result: the Client SHALL NOT wait for it before handling input, and SHALL NOT show the load as applied until that result or an owner snapshot contains it. A remote Player owner that does not advertise answered queue edits SHALL still receive the edit, and the Client SHALL NOT wait for an answer from it.

#### Scenario: Rapid repeated removals

- **WHEN** the user presses the remove key three times in quick succession on consecutive entries of the Local queue
- **THEN** each removal SHALL be sent against the queue as updated by the previous removal's answer
- **AND** three entries SHALL be removed

#### Scenario: Other Clients see the edit

- **WHEN** one Client's queue edit is accepted while a second Client is attached
- **THEN** the second Client SHALL display the resulting queue from the owner's snapshot

#### Scenario: Owner does not answer in time

- **WHEN** the owner does not answer a queue edit within the bound
- **THEN** the Client SHALL report that the owner did not respond
- **AND** SHALL NOT show the edit as applied until an owner snapshot contains it

#### Scenario: Late answer arrives

- **WHEN** the owner's answer to an edit arrives after the Client stopped waiting
- **THEN** the Client SHALL adopt that answer's state like any other owner snapshot

#### Scenario: Idle load while an item plays

- **WHEN** the user loads a playlist without starting playback while the owner is still stopping the playing item
- **THEN** the Client SHALL keep handling input
- **AND** SHALL show the loaded playlist once the owner's load result or snapshot contains it

#### Scenario: Older remote owner

- **WHEN** the user edits the Remote scope queue of a directly controlled owner that does not advertise answered queue edits
- **THEN** the edit SHALL be sent and the Client SHALL NOT wait for an answer
- **AND** the displayed queue SHALL follow that owner's later snapshots

### Requirement: Queue selection follows the selected slot

A Client's queue selection SHALL identify the selected queue slot, not a position. When the Client adopts the answer to its own queue edit, the selection SHALL stay on the selected slot wherever it moved; when that slot is gone, the selection SHALL move to the entry now at its former position, or to the last entry when that position no longer exists. The answer to the Client's own edit SHALL NOT move the selection to the playing entry. A snapshot the Client did not cause SHALL keep today's rule: the selection follows the active slot unless the user navigated the queue recently, in which case it stays on the user's slot as above. A whole-queue replacement SHALL move the selection to the replacement's start entry.

#### Scenario: Removing the selected entry

- **WHEN** the selected entry is removed and the owner's snapshot is adopted
- **THEN** the selection SHALL be on the entry that followed it, or on the new last entry when the removed entry was last

#### Scenario: Moving the selected entry

- **WHEN** the selected entry is moved and the owner's snapshot is adopted
- **THEN** the selection SHALL remain on that entry at its new position

#### Scenario: Track advance while browsing the queue

- **WHEN** the owner advances to the next track shortly after the user navigated to a different entry
- **THEN** the selection SHALL remain on the user's entry

#### Scenario: Track advance while not browsing

- **WHEN** the owner advances to the next track and the user has not navigated the queue recently
- **THEN** the selection SHALL move to the newly active entry

#### Scenario: Whole-queue replacement

- **WHEN** an adopted snapshot comes from a whole-queue replacement
- **THEN** the selection SHALL move to the replacement's start entry

### Requirement: Queue undo is an owner operation

Undoing a queue edit SHALL send the inverse edit to the Player owner and SHALL follow the same answered-edit rule as any other edit. Undoing a removal SHALL insert the removed item before the entry now at the removed item's former position, or at the end when that position no longer exists; the restored entry is a new occurrence. Undoing a move SHALL move the same slot back to its former position. Undo history SHALL belong to the Client that made the edits.

#### Scenario: Undo a removal

- **WHEN** the user removes the third entry and then undoes
- **THEN** the owner SHALL hold that item at the third position again
- **AND** every attached Client SHALL display it there

#### Scenario: Undo a move whose slot is gone

- **WHEN** the user undoes a move after another Client removed the moved entry
- **THEN** the owner SHALL reject the undo as stale slot addressing
- **AND** the Client SHALL report that the undo did not apply
