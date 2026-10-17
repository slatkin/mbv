# Spec Delta

## MODIFIED Requirements

### Requirement: Eligible control relationships

Fall-through SHALL apply when the client is attached to a Player owner that the
client knows cannot play the selection: a ctrl-attached daemon that advertised
audio-only, or a local owner whose Emby Session output advertises playable
media types that exclude the selection's media kind. It SHALL NOT apply through a
Library route, through a session the client does not control, or to an owner
whose playable-media capability the client does not know.

#### Scenario: Direct remote control is eligible
- **WHEN** the user explicitly plays a video while Direct remote control targets an owner that advertised audio-only
- **THEN** the client SHALL treat that play as eligible for fall-through

#### Scenario: Explicit daemon attachment is eligible
- **WHEN** a client launched against an explicit remote daemon endpoint explicitly plays a video and that owner advertised audio-only
- **THEN** the client SHALL treat that play as eligible for fall-through

#### Scenario: Emby session owner is eligible
- **WHEN** the local owner's output is an Emby Session whose advertised playable media types are audio only and the user explicitly plays a video
- **THEN** the client SHALL treat that play as eligible for fall-through

#### Scenario: Library route is ineligible
- **WHEN** a Library route targets an owner that advertised audio-only
- **THEN** explicit play and enqueue actions SHALL retain Library-route behavior
- **THEN** the client SHALL NOT raise the local-playback prompt

### Requirement: The attached owner's playable-media capability is known before routing

The client SHALL retain each attached owner's advertised playable-media
capability and consult it before routing an explicit play. A ctrl peer's
audio-only advertisement and the advertised playable media types of the owner's
Emby Session output SHALL both be available to that decision. An owner that advertises
no playable-media capability SHALL be treated as able to play the selection, and
an advertised capability the client does not recognize SHALL NOT prevent the
connection.

#### Scenario: ctrl peer advertises audio-only
- **WHEN** the client attaches to a ctrl peer whose handshake advertises audio-only
- **THEN** the client SHALL treat that owner as unable to play video for as long as the connection lasts

#### Scenario: ctrl peer advertises no playable-media capability
- **WHEN** the client attaches to a ctrl peer whose handshake advertises no playable-media capability
- **THEN** the client SHALL treat that owner as able to play the selection

#### Scenario: Emby session advertises audio playable media types only
- **WHEN** the owner presents an Emby Session output whose playable media types are audio only
- **THEN** the client SHALL treat that owner as unable to play video while that Session is the output

#### Scenario: Unrecognized advertised capability
- **WHEN** a peer's handshake advertises a capability the client does not recognize
- **THEN** the client SHALL connect
- **THEN** the client SHALL behave as if that capability were absent

### Requirement: Confirmed local playback stops the owner and ends the attachment

When the prompt is accepted, the client SHALL stop the attached owner, then end
the attachment, then play the selection as local playback. It SHALL stop the
owner rather than pause it, so the owner does not keep playing underneath the
local item, and it SHALL NOT control, command, or present that owner's Bound
queue after the fall-through. For an Emby Session output, ending the attachment
SHALL return the local owner's output to local playback.

#### Scenario: Playing a film locally
- **WHEN** the user confirms playing a film locally while attached to an audio-only owner
- **THEN** the client SHALL stop the owner before local playback starts
- **THEN** the client SHALL end the attachment
- **THEN** the client SHALL play the selection as local playback
- **THEN** the client SHALL NOT present the owner's Bound queue

#### Scenario: Emby session owner
- **WHEN** the user confirms playing a film locally while the local owner's output is an audio-only Emby Session
- **THEN** the client SHALL stop playback on the Session
- **THEN** the client SHALL select local playback as the owner's output
- **THEN** the client SHALL play the selection as local playback

#### Scenario: Former owner after the fall-through
- **WHEN** local playback from a confirmed fall-through is playing
- **THEN** the client SHALL NOT send transport or queue commands to the former owner
