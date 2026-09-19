## ADDED Requirements

### Requirement: Canonical-queue edits follow the playback target

A Client SHALL dispatch canonical-queue edit commands (slot removal, slot move, item append) to a Player owner only when that owner is the resolved playback target for the queue being edited: the in-process bare player or the home Local daemon. When playback is directed at an attached Emby session or a cast receiver, the Client SHALL apply the edit to its own canonical queue only and SHALL NOT transmit a queue command to the home Player owner, because that owner does not hold the queue the user is editing. Direct-remote queue management (editing a directly controlled remote Player owner over ctrl) is unchanged: edits addressed to that owner's scope continue to reach it.

#### Scenario: Queue edit while an attached session plays

- **WHEN** a Client's playback target is an attached Emby session and the user removes, moves, or appends an item in the canonical queue that was submitted to that session
- **THEN** the Client SHALL mutate its own canonical queue
- **AND** SHALL NOT send a queue command to the home Player owner

#### Scenario: Visible queue survives a stale owner

- **WHEN** the home Player owner holds a Bound queue that differs from the Client's canonical queue while an attached session is the playback target
- **THEN** a canonical-queue edit SHALL NOT cause the Client to replace its visible queue with the owner's snapshot
- **AND** the user's loaded playlist source and contents SHALL remain unchanged

#### Scenario: Direct-remote edits still reach the owner

- **WHEN** the user edits a directly controlled remote Player owner's queue in Remote scope
- **THEN** the edit SHALL still be dispatched to that owner as a slot-addressed command
