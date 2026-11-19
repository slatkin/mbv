# Spec Delta

## Purpose

The local Owner process publishes MPRIS whenever no Client holds the bus name, so playback keeps
its media-key and desktop-widget surface while stay-alive plays without any Client window.

## ADDED Requirements

### Requirement: The local Owner publishes MPRIS while no Client holds the name

While the local Owner process runs and no Client holds `org.mpris.MediaPlayer2.mbv`, the Owner
SHALL claim the name and publish the same MPRIS surface a Client publishes: the Owner's own
playback status, now-playing metadata, track art resolved from the image cache by the same keys
any other MPRIS publisher in mbv uses, and transport commands dispatched to the Owner's Player.

#### Scenario: The last Client exits while playback runs

- **WHEN** stay-alive keeps the Owner playing and the last Client process exits
- **THEN** the Owner SHALL claim `org.mpris.MediaPlayer2.mbv`
- **THEN** an MPRIS consumer SHALL see the Owner's playback state and now-playing metadata
- **THEN** a transport command from an MPRIS consumer SHALL act on the Owner's Player

#### Scenario: No Client ever attached

- **WHEN** the Owner process plays while no Client has attached since it started
- **THEN** the Owner SHALL publish MPRIS as above

#### Scenario: Remote playback output

- **WHEN** the Owner's playback output is a Cast receiver or an Emby Session and no Client holds
  the name
- **THEN** the Owner SHALL publish the presented playback state of that output

### Requirement: A starting Client wins the bus name

A Client starting while the Owner's fallback holds `org.mpris.MediaPlayer2.mbv` SHALL become the
name's owner through its normal claim, and the Owner SHALL stop publishing and return to waiting
for the name to be free. The Owner's fallback claim SHALL NOT replace any current name owner, so a
Client publishing a target other than the local Owner is never displaced by the fallback.

#### Scenario: Client starts while the fallback holds the name

- **WHEN** a Client process starts and claims `org.mpris.MediaPlayer2.mbv` while the Owner's
  fallback holds it
- **THEN** the Client SHALL become the name's owner
- **THEN** the Owner SHALL stop publishing until the name is free again

#### Scenario: The fallback never displaces a Client

- **WHEN** any process holds `org.mpris.MediaPlayer2.mbv` and does not allow replacement
- **THEN** the Owner's fallback claim SHALL fail without queueing
- **THEN** the Owner SHALL keep running and SHALL retry only once no Client holds the name

### Requirement: The packaged daemon publishes no MPRIS

The packaged daemon `mbvd` SHALL NOT start an MPRIS publisher. It runs as a system service
without a user desktop session. A Client attached to it publishes MPRIS as any Client does.

#### Scenario: mbvd runs headless

- **WHEN** `mbvd` starts as a system service
- **THEN** it SHALL NOT claim `org.mpris.MediaPlayer2.mbv`
- **THEN** its playback behavior SHALL be unaffected

### Requirement: A missing MPRIS is not an error

When the Owner cannot claim or serve the MPRIS name — no session bus, or the build runs in a test
process — the Owner SHALL keep running normally. mbv SHALL NOT warn the user on the terminal, on
either the Owner or the Client side.

#### Scenario: Headless host

- **WHEN** the Owner starts over SSH or on a bare terminal with no session bus
- **THEN** the Owner SHALL run and play normally with no MPRIS publisher
