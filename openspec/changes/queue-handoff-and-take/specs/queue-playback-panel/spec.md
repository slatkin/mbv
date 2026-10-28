# Spec Delta

## MODIFIED Requirements

### Requirement: Queue-visible layouts paint a now-playing header row

In every layout that renders the queue column, the left column SHALL paint one header row at the top
of its content, above the visual slot. The header SHALL always be painted, including
while playback is idle.

The header SHALL follow the effective playback target — cast, then connected session, then local —
and SHALL NOT follow local playback while a peer is connected.

#### Scenario: Active watched remote playback paints no playhead

- **WHEN** a queue-visible layout is painted while attached to a Session that reports a
  now-playing item the viewed queue does not hold (another device's own selection)
- **THEN** no queue row SHALL be painted as the playhead

#### Scenario: Header follows the playback target, not the viewed queue

- **WHEN** the layout is queue-visible while the Client is connected to a peer and the local Owner
  also plays
- **THEN** the header SHALL describe the peer's playback, not the local Owner's

#### Scenario: No header when the queue column is hidden

- **WHEN** the layout is library-only
- **THEN** no header row SHALL be painted

## ADDED Requirements

### Requirement: The queue panel names the Player owner whose queue it shows

Whenever the queue column is visible, the queue panel SHALL name the Player owner whose queue it shows: the local Owner when no peer is connected, or the connected peer by its device name. The name SHALL change in the same frame in which the shown queue changes. The name SHALL NOT be an interaction target.

#### Scenario: No peer connected

- **WHEN** the queue column is visible and the Client is not connected to a peer
- **THEN** the queue panel SHALL name the local Owner

#### Scenario: Connected to a peer

- **WHEN** the Client connects to a peer or a Hand off to it succeeds
- **THEN** the queue panel SHALL name that peer in the frame that first shows its queue

#### Scenario: Emby Session or Cast output

- **WHEN** an Emby Session or a cast receiver is the playback output and no peer is connected
- **THEN** the queue panel SHALL name the local Owner
