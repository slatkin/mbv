## MODIFIED Requirements

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

## ADDED Requirements

### Requirement: Queue loading SHALL not flash a wrong track
When a Player loads a queue at a non-zero start index, the Playback run SHALL NOT briefly play or report a different slot before the intended start slot begins. The mpv playlist construction SHALL ensure that the intended start slot is the one that plays from the first audible moment.

#### Scenario: Queue loaded at middle position
- **WHEN** a queue of 100 items is loaded at start index 50
- **THEN** the Playback run SHALL begin playback of item 50 without first starting item 0
- **AND** observers SHALL not receive a transient active-slot report for any slot other than 50

#### Scenario: Queue loaded at index 0
- **WHEN** a queue is loaded at start index 0
- **THEN** playback SHALL begin at item 0 with no behavioral difference from a non-zero start
