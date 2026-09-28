# Spec Delta

## ADDED Requirements

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
