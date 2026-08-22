## Purpose

Finds Google Cast receivers on the local network and presents them as playback targets
beside Emby sessions, identifying each one durably so a target survives address changes
between mbv launches.

## ADDED Requirements

### Requirement: Cast receivers are discovered on demand

mbv SHALL browse the local network for Google Cast receivers when the target panel is
opened, and SHALL present discovered receivers in that panel alongside Emby sessions.
Discovery SHALL NOT run continuously in the background.

#### Scenario: Target panel is opened

- **WHEN** the user opens the target panel
- **THEN** mbv browses the local network for cast receivers
- **AND** presents each discovered receiver as a selectable playback target

#### Scenario: Target panel is closed

- **WHEN** the target panel is not open
- **THEN** mbv SHALL NOT hold an active network browse for cast receivers

#### Scenario: No receivers respond

- **WHEN** the browse completes with no receivers found
- **THEN** mbv presents the panel with its Emby session targets and no cast targets
- **AND** SHALL NOT report an error

### Requirement: Discovery does not delay Emby targets

mbv SHALL present Emby session targets as soon as they are available, without waiting for
the network browse to complete.

#### Scenario: Browse is slower than the session fetch

- **WHEN** the Emby session list arrives before the network browse completes
- **THEN** mbv presents the Emby session targets immediately
- **AND** adds cast targets to the panel when the browse completes

### Requirement: Discovery channel determines target kind

Each target in the panel SHALL carry the kind established by the channel that produced
it. Targets obtained from the Emby session list SHALL be classified as Emby-session or
daemon targets; targets obtained from network discovery SHALL be classified as cast
targets. mbv SHALL NOT probe a target to determine how to control it.

#### Scenario: A device offers both channels

- **WHEN** one physical device appears both in the Emby session list and in cast discovery
- **THEN** mbv presents two distinct targets, each labelled with its kind
- **AND** selecting either one uses only that target's control channel

### Requirement: Cast targets are identified independently of network address

mbv SHALL identify a cast receiver by the stable identifier the receiver advertises, not
by its network address. When a persisted cast target is restored, mbv SHALL resolve its
current address by discovery rather than reusing a stored address.

#### Scenario: Receiver address changes between launches

- **WHEN** a persisted cast target is reachable at a different network address than when
  it was persisted
- **THEN** mbv resolves the receiver by its advertised identifier and connects to the
  current address

#### Scenario: Persisted receiver is absent

- **WHEN** a persisted cast target does not appear in discovery
- **THEN** mbv SHALL report the target as unavailable and SHALL NOT attempt to connect to
  a stored address

### Requirement: Discovery failure is isolated from the panel

If network discovery cannot start, times out, or fails partway, mbv SHALL log the
diagnostic, present whatever targets it has, and keep the panel and input handling
running.

#### Scenario: Discovery cannot start

- **WHEN** the network browse cannot be started
- **THEN** mbv logs the diagnostic, presents its Emby session targets, and keeps the
  panel usable
