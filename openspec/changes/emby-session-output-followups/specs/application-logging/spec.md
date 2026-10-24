# Spec Delta

## MODIFIED Requirements

### Requirement: Converted events are named
Every event emitted by mbv code SHALL carry an event name in dot notation
`<component>.<operation>.<state>` (for example `player.load.failed`), written as the
`event` field. Event names SHALL be stable identifiers: the same situation SHALL produce the
same name in every process.

#### Scenario: Grouping by event
- **WHEN** a user filters a log for `event=player.load.failed`
- **THEN** every load failure is found, and no other event

### Requirement: Correlation fields
Log lines SHALL carry correlation fields for the operation they belong to, using the same
key names in `mbv`, the local daemon and `mbvd`:
- ctrl playback intents: `request` (playback request id) and `generation`. On the Player
  owner, also `client` (the owner's id for the connection that sent it).
- playback sessions: `slot` (queue slot id), `item` (content id, when there is one) and,
  for Emby, `play_session`
- Emby playback reporting: `item` and `play_session` of the session being reported
- HTTP requests to a Service: `service`, `http.request.method`, `url.path`, and once
  known `http.response.status_code` and `duration_ms`
Every line logged inside such an operation SHALL carry that operation's fields. This
includes lines from code that does not know about the operation, lines logged on worker
threads the operation starts, and lines logged when a deferred result of the operation
is handled later.

Request ids are unique only within one client connection. So that lines can be matched
across processes:
- After a client completes the Control handshake and is registered, the Player owner SHALL
  log one line with `client` and `peer`: the peer's process id for a local socket, or the
  peer's `ip:port` for a network connection.
- Each time `mbv` connects to a Player owner, it SHALL log one line with `peer` in the
  same format: its own process id for a local socket, or its local `ip:port` for a
  network connection.
- An equal `peer=` value on the two lines SHALL identify the same connection. This is
  guaranteed for local sockets, and for network connections with no address translation
  between `mbv` and the owner.

#### Scenario: One intent across processes
- **WHEN** the TUI sends a playback intent and the daemon accepts and starts it
- **THEN** the TUI's send line in `mbv.log` and the daemon's accept and start lines in its
  own log all carry the same `request=` value, and the daemon's lines carry the `client=`
  whose connect line has the same `peer=` as the TUI's connect line

#### Scenario: Remote TUI over the network
- **WHEN** a TUI connects to `mbvd` over TCP with no address translation in between
- **THEN** the TUI's connect line `peer=` (its local `ip:port`) equals the `peer=` on
  the owner's connect line for that client

#### Scenario: Two clients reuse a request id
- **WHEN** two TUIs attached to the same `mbvd` each send a playback intent with request
  id 1
- **THEN** the owner's lines for the two intents carry different `client=` values, and
  each `client=` connect line names a different `peer`

#### Scenario: Deferred resolution keeps context
- **WHEN** a playback intent's item lookup fails on the lookup worker and the failure is
  handled back on the owner's event loop
- **THEN** the failure line carries the intent's `client=` and `request=`

#### Scenario: Reporting worker inherits session context
- **WHEN** an Emby progress report fails on a reporting worker thread
- **THEN** the failure line carries the `item=` and `play_session=` of the session being
  reported, even though the report code does not log them

#### Scenario: Deferred start is not mislabelled
- **WHEN** a start report for a new item is still resolving its play session while the
  previous session's ids are still current
- **THEN** its lines carry the new item's `item=`, and no `play_session=` until the new
  session is known. No part of those lines carries the previous session's `slot`, `item`
  or `play_session` values.

