## MODIFIED Requirements

### Requirement: Queue occurrences have stable slot identity

Each occurrence of a `QueueItem` SHALL have stable runtime slot identity independent of its provider-qualified content identity or source URL. Operations on an existing queue occurrence SHALL target its slot identity. A slot identity assigned by a Player owner SHALL be the identity every component of that owner uses for the same occurrence, including its Playback run: a Playback run SHALL adopt the owner's slot identities rather than assigning its own. An ordinal index MAY be used only as a presentation coordinate within the component that resolved it, and SHALL NOT address a queue occurrence across a component boundary or after the queue may have been mutated.

#### Scenario: Duplicate content occurrences

- **WHEN** the same `QueueItem` is appended twice
- **THEN** the queue SHALL contain two independently addressable slots

#### Scenario: Play an existing slot

- **WHEN** the user plays an item already present in the queue
- **THEN** playback SHALL select that slot
- **AND** SHALL NOT append another occurrence as a side effect

#### Scenario: Playback run reports the active occurrence

- **WHEN** a Playback run reports which occurrence became active, completed, or stopped
- **THEN** it SHALL name the slot identity the Player owner assigned to that occurrence
- **AND** the owner SHALL resolve that report without inferring the occurrence from an ordinal position

#### Scenario: Occurrence is moved before a report is observed

- **WHEN** a queue occurrence changes ordinal position while a command or report naming it is in flight
- **THEN** the occurrence SHALL still resolve to the same slot
- **AND** no other occurrence SHALL be activated, completed, consumed, or removed in its place

### Requirement: Completion and consumption address the canonical slot

Natural completion and explicit consumption SHALL identify the affected canonical queue slot and apply the queue's existing consume policy without branching by item kind. Content identity SHALL NOT be used to remove other occurrences. Consume SHALL be applied by the Player owner that holds the Bound queue, so that the same completion produces the same queue outcome regardless of whether the owner is in-process, a Local daemon, or a packaged `mbvd`, and regardless of whether any Client is attached.

#### Scenario: Feed slot completes naturally

- **WHEN** playback naturally completes a Feed entry whose slot is eligible for consumption
- **THEN** the owner SHALL consume that slot through the same slot-based queue operation used for an Emby item
- **AND** SHALL preserve any other slot containing the same Feed entry

#### Scenario: Slot is retained by policy

- **WHEN** playback completes a slot that the active consume policy retains
- **THEN** the slot SHALL remain in the canonical queue regardless of item kind

#### Scenario: Out-of-process owner consumes a completed slot

- **WHEN** a daemon Player owner completes a slot its consume policy removes
- **THEN** that owner's canonical queue SHALL no longer contain the slot
- **AND** attached Clients SHALL observe the shortened queue through ordinary queue state rather than each applying removal locally

#### Scenario: Completion arrives with no Client attached

- **WHEN** a slot completes on a Player owner while no Client is attached
- **THEN** the consume policy SHALL be applied to the canonical queue
- **AND** a Client attaching afterwards SHALL observe the same queue as one that had been attached throughout

## ADDED Requirements

### Requirement: Stale slot addressing is rejected, not reinterpreted

A queue command or report that names a slot the receiving component no longer holds SHALL be rejected without mutating the canonical queue. A component SHALL NOT substitute a neighbouring slot, clamp to the nearest position, or fall back to a remembered position when the named slot is absent. Rejection SHALL be observable to the sender through the existing command-rejection path.

#### Scenario: Mutation and command cross in flight

- **WHEN** a slot is removed from the canonical queue while a command addressing that slot is in flight
- **THEN** the command SHALL be rejected
- **AND** no other slot SHALL be removed, moved, or activated as a result

#### Scenario: Queue shrinks beneath an in-flight report

- **WHEN** a Playback run reports a slot that the owner's queue no longer contains
- **THEN** the owner SHALL discard the report without changing its active slot
- **AND** SHALL NOT clamp the report onto the nearest surviving slot

#### Scenario: Rejected mutation is surfaced

- **WHEN** a Client's queue mutation is rejected because its target slot is gone
- **THEN** the Client SHALL be told the mutation did not apply
- **AND** the Client's view SHALL reconcile to the owner's canonical queue

### Requirement: Near-end completion uses one rule

The decision that playback finished close enough to the end to count as completed SHALL be evaluated by one rule applied to the completed occurrence's own runtime. Every completion path — ordinary advance, end of queue, quit, and process shutdown — SHALL reach the same verdict for the same completed occurrence, position, and media kind.

#### Scenario: Same completion, different exit path

- **WHEN** the same occurrence at the same position ends through natural advance, through a quit, or through owner shutdown
- **THEN** each path SHALL produce the same near-end verdict
- **AND** the same watched-state and consume outcome SHALL follow

#### Scenario: Runtime is taken from the completed occurrence

- **WHEN** playback has already advanced its live status to the next occurrence at the moment completion is evaluated
- **THEN** the near-end verdict SHALL use the completed occurrence's runtime
- **AND** SHALL NOT use the runtime of the occurrence that replaced it
