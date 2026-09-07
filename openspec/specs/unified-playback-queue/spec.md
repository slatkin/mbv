# unified-playback-queue Specification

## Purpose
Define one queue and playback-submission model shared by every QueueItem across composed editing, Player ownership, local and ctrl control, persistence, and mpv playback.
## Requirements
### Requirement: Each queue has one canonical ordered representation

Every Composed or Bound queue SHALL be represented by one ordered collection of queue slots containing `QueueItem` values. A Player owner SHALL hold the only authoritative collection for its Bound queue. A Client MAY hold a replaceable snapshot of a Bound queue, and a Playback run MAY hold an mpv execution projection, but neither SHALL independently decide canonical order, active slot, revision, or queue mutation outcome. A component SHALL NOT maintain parallel item-kind collections whose synchronization is required to determine queue contents, order, length, or current slot. An mpv projection MAY contain the full playable sequence or only the active materialized file as required by source lifecycle, but that projection SHALL NOT become queue authority.

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

### Requirement: Queue occurrences have stable slot identity

Each occurrence of a `QueueItem` SHALL have stable runtime slot identity independent of its provider-qualified content identity or source URL. Operations on an existing queue occurrence SHALL target its slot identity. A slot identity assigned by a Player owner SHALL be used by the Client snapshot and Playback-run projection for that occurrence. An ordinal index MAY be used only as a presentation or mpv-adapter coordinate within the component that resolved it, and SHALL NOT address a queue occurrence across a component boundary or after the queue may have mutated.

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

#### Scenario: Client receives a newer snapshot

- **WHEN** a Client receives an owner snapshot with a newer queue revision or transition state
- **THEN** it SHALL replace its previous Bound-queue snapshot
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
- **WHEN** an Emby refresh merges into a mixed queue containing a podcast episode and a book from Audiobookshelf
- **THEN** both Audiobookshelf slots SHALL remain unchanged
- **AND** Emby-owned slots SHALL continue to reconcile normally

### Requirement: Desired and observed playback remain distinct

A request to play a Queue slot SHALL create desired transition state and SHALL NOT itself change the observed active slot. The Player owner SHALL change the observed active slot only from a Playback-run observation naming an owner-assigned slot. User-visible playback state SHALL identify observed playback separately from any pending desired slot.

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
