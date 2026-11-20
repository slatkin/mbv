# local-owner-mpris Specification

## Purpose
mbv publishes MPRIS from the process that holds the playback the user controls: the local Owner
process for local playback, and a Client for the remote Player owner it controls. Local playback
wins over remote playback.

## Requirements

### Requirement: The local Owner process publishes MPRIS

While the local Owner process runs, it SHALL hold `org.mpris.MediaPlayer2.mbv` and publish its own
playback status, now-playing metadata, track art resolved from the image cache by the same keys
any other MPRIS publisher in mbv uses, and transport commands dispatched to its Player. It SHALL
do so whatever Clients are attached, including none. While it plays nothing it SHALL publish the
`Stopped` playback status. No other process SHALL take the name from it.

#### Scenario: The last Client exits while playback runs

- **WHEN** stay-alive keeps the Owner playing and the last Client process exits
- **THEN** `org.mpris.MediaPlayer2.mbv` SHALL stay on the session bus without a gap
- **THEN** an MPRIS consumer SHALL see the Owner's playback state and now-playing metadata
- **THEN** a transport command from an MPRIS consumer SHALL act on the Owner's Player

#### Scenario: Clients attach and exit in any order

- **WHEN** several Clients attach to the Owner and exit in any order
- **THEN** the Owner SHALL keep `org.mpris.MediaPlayer2.mbv` throughout

#### Scenario: Remote playback output

- **WHEN** the Owner's playback output is a Cast receiver or an Emby Session
- **THEN** the Owner SHALL publish the presented playback state of that output

### Requirement: A Client attached to the local Owner publishes no MPRIS

A Client whose Player owner is the local Owner process SHALL NOT hold any MPRIS bus name.

#### Scenario: Two local Clients

- **WHEN** a terminal Client and the pinned panel are both attached to the local Owner
- **THEN** MPRIS consumers SHALL see one mbv player, published by the Owner

### Requirement: A Client that controls a remote Player owner publishes it

A Client attached to a remote Player owner, through Direct remote control or a Library route,
SHALL hold `org.mpris.MediaPlayer2.mbv.instance<pid>`, where `<pid>` is its process id, subject to
the local-wins rule below. Under that name it SHALL publish the remote Player owner's playback
status, now-playing metadata, and track art, and it SHALL dispatch transport commands to that
Player owner.

#### Scenario: The Client returns to the local Owner

- **WHEN** a Client that holds its instance name returns to the local Owner or exits
- **THEN** its instance name SHALL leave the session bus

### Requirement: Local playback wins over remote publishing

A Client that controls a remote Player owner SHALL hold its instance name only while the local
Owner's playback status is not `Playing`. While that status is `Playing`, the Client SHALL NOT
hold the name.

#### Scenario: Remote playback while local playback is idle

- **WHEN** a Client controls a remote Player owner and the local Owner is stopped or paused
- **THEN** the Client SHALL hold `org.mpris.MediaPlayer2.mbv.instance<pid>`
- **THEN** a transport command sent to that name SHALL act on the remote Player owner

#### Scenario: Local playback starts

- **WHEN** a Client controls a remote Player owner and the local Owner's status becomes `Playing`
- **THEN** the Client SHALL release its instance name
- **THEN** the Client SHALL claim it again when the local Owner's status stops being `Playing`

#### Scenario: The local Owner's status is unknown

- **WHEN** a Client controls a remote Player owner and has no live link to the local Owner
- **THEN** the Client SHALL treat the local status as not `Playing` and hold its instance name

### Requirement: The packaged daemon publishes no MPRIS

The packaged daemon `mbvd` SHALL NOT start an MPRIS publisher. It runs as a system service
without a user desktop session. A Client attached to it publishes MPRIS as a Client that controls
a remote Player owner.

#### Scenario: mbvd runs headless

- **WHEN** `mbvd` starts as a system service
- **THEN** it SHALL NOT hold any MPRIS bus name
- **THEN** its playback behavior SHALL be unaffected

### Requirement: A missing MPRIS is not an error

When the Owner or a Client cannot claim or serve its MPRIS name — no session bus, or the build
runs in a test process — that process SHALL keep running normally. mbv SHALL NOT warn the user on
the terminal, on either the Owner or the Client side.

#### Scenario: Headless host

- **WHEN** the Owner starts over SSH or on a bare terminal with no session bus
- **THEN** the Owner SHALL run and play normally with no MPRIS publisher
