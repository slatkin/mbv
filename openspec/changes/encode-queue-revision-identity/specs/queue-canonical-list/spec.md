## ADDED Requirements

### Requirement: A queue revision names one queue state

Within one process, two different queue states SHALL never carry the same
queue revision. Every queue revision SHALL be issued fresh when a queue is
constructed or a projected row changes. No code path SHALL construct a queue
with a revision chosen by its caller or copied from another process. A queue
that replaces or rebuilds another, whether the Player owner replacing,
loading, or purging its queue or a Client adopting an owner snapshot, SHALL
therefore carry a revision the Queue projection has not seen. The Queue rows
SHALL follow the new contents on the next tick. An identical copy of a queue
MAY share its revision.

#### Scenario: Owner purge repaints the Queue

- **WHEN** a Service teardown removes that Service's items from the Player owner's queue while the active slot and playback state stay the same
- **THEN** the Client's Queue rows no longer show the removed items after the next tick

#### Scenario: Consecutive idle loads repaint the Queue

- **WHEN** nothing is playing and a second playlist is loaded into the queue immediately after a first one
- **THEN** the Queue rows show the second playlist's items, even when both loads produce the same slot identities and the same active slot

#### Scenario: Re-adopting an owner snapshot repaints the Queue

- **WHEN** a Client adopts an owner snapshot after connecting, switching owners, or the owner restarting
- **THEN** the Queue rows are rebuilt from that snapshot, whatever revision number the owner sent

#### Scenario: Unchanged queue still skips projection

- **WHEN** no mutation, replacement, or adoption has happened since the last projection and the other fingerprint fields are unchanged
- **THEN** the Queue projection is skipped as before
