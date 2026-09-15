## MODIFIED Requirements

### Requirement: Eligible control relationships

Fall-through SHALL apply when the client is attached to a Player owner that the
client knows cannot play the selection: a ctrl-attached daemon that advertised
audio-only, or an Emby session the client controls whose advertised playable
media types exclude the selection's media kind. It SHALL NOT apply through a
Library route, through a session the client does not control, or to an owner
whose playable-media capability the client does not know.

#### Scenario: Direct remote control is eligible
- **WHEN** the user explicitly plays a video while Direct remote control targets an owner that advertised audio-only
- **THEN** the client SHALL treat that play as eligible for fall-through

#### Scenario: Explicit daemon attachment is eligible
- **WHEN** a client launched against an explicit remote daemon endpoint explicitly plays a video and that owner advertised audio-only
- **THEN** the client SHALL treat that play as eligible for fall-through

#### Scenario: Emby session owner is eligible
- **WHEN** the client controls an Emby session whose advertised playable media types are audio only and the user explicitly plays a video
- **THEN** the client SHALL treat that play as eligible for fall-through

#### Scenario: Library route is ineligible
- **WHEN** a Library route targets an owner that advertised audio-only
- **THEN** explicit play and enqueue actions SHALL retain Library-route behavior
- **THEN** the client SHALL NOT raise the local-playback prompt

### Requirement: Routing decision for explicit selections

For each eligible explicit play, the client SHALL decide before submitting or
mutating queue presentation state whether the owner can play the selection.
When the owner cannot play any item in the selection, the client SHALL raise a
confirmation prompt instead of submitting and SHALL submit nothing while that
prompt is open. The prompt SHALL name the selection and SHALL offer playing it on
this machine; `y`, `Y`, and Enter SHALL accept it, and `n`, `N`, and Esc SHALL
decline it. Declining SHALL leave the attachment, its Bound queue, playback, and
queue presentation unchanged.

#### Scenario: Wholly non-audio selection
- **WHEN** the user explicitly plays a selection whose items the eligible owner cannot play
- **THEN** the client SHALL raise the confirmation prompt
- **THEN** the client SHALL NOT submit any item of that selection to the owner

#### Scenario: Mixed selection
- **WHEN** an eligible explicit selection contains items the owner can play and items it cannot
- **THEN** the client SHALL NOT raise the prompt
- **THEN** the client SHALL submit the selection to the owner
- **THEN** the client SHALL report the number of items in it that the owner cannot play

#### Scenario: Wholly audio selection
- **WHEN** an eligible explicit selection contains only items the owner can play
- **THEN** the client SHALL submit the selection to the owner

#### Scenario: Peer without the capability
- **WHEN** the attached owner's playable-media capability is unknown to the client
- **THEN** the client SHALL submit the selection to the owner
- **THEN** the client SHALL NOT raise the local-playback prompt

#### Scenario: Declined prompt
- **WHEN** the user declines the confirmation prompt
- **THEN** the client SHALL leave the attachment, its Bound queue, and playback unchanged

### Requirement: Fall-through is per explicit action

The client SHALL evaluate every explicit play independently. Fall-through SHALL
NOT become a persistent routing mode and SHALL NOT be invoked by queue
auto-advance, resume, or owner-initiated events. An explicit enqueue SHALL NOT
raise the prompt and SHALL NOT stage the selection on the client's own queue; it
SHALL retain the enqueue behavior the client already has for the attached owner.

#### Scenario: Action after fall-through
- **WHEN** the user explicitly plays another item after an item has fallen through
- **THEN** the client SHALL evaluate that play against the then-current attachment
- **THEN** the client SHALL evaluate it as an ordinary local play when the fall-through ended the attachment

#### Scenario: Bound queue advances
- **WHEN** the attached owner advances within its Bound queue
- **THEN** the client SHALL NOT raise the prompt for that advance

#### Scenario: Explicit enqueue of an unplayable selection
- **WHEN** the user explicitly enqueues a selection the eligible owner cannot play
- **THEN** the client SHALL NOT raise the prompt
- **THEN** the client SHALL NOT append that selection to the client's own queue
- **THEN** the client SHALL submit the enqueue to the owner as it does today

## ADDED Requirements

### Requirement: The attached owner's playable-media capability is known before routing

The client SHALL retain each attached owner's advertised playable-media
capability and consult it before routing an explicit play. A ctrl peer's
audio-only advertisement and a controlled Emby session's advertised playable
media types SHALL both be available to that decision. An owner that advertises
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
- **WHEN** the client's session list reports a session whose playable media types are audio only
- **THEN** the client SHALL treat that owner as unable to play video

#### Scenario: Unrecognized advertised capability
- **WHEN** a peer's handshake advertises a capability the client does not recognize
- **THEN** the client SHALL connect
- **THEN** the client SHALL behave as if that capability were absent

### Requirement: Confirmed local playback stops the owner and ends the attachment

When the prompt is accepted, the client SHALL stop the attached owner, then end
the attachment, then play the selection as local playback. It SHALL stop the
owner rather than pause it, so the owner does not keep playing underneath the
local item, and it SHALL NOT control, command, or present that owner's Bound
queue after the fall-through.

#### Scenario: Playing a film locally
- **WHEN** the user confirms playing a film locally while attached to an audio-only owner
- **THEN** the client SHALL stop the owner before local playback starts
- **THEN** the client SHALL end the attachment
- **THEN** the client SHALL play the selection as local playback
- **THEN** the client SHALL NOT present the owner's Bound queue

#### Scenario: Emby session owner
- **WHEN** the user confirms playing a film locally while controlling an audio-only Emby session
- **THEN** the client SHALL stop that session's playback
- **THEN** the client SHALL end the session control relationship
- **THEN** the client SHALL play the selection as local playback

#### Scenario: Former owner after the fall-through
- **WHEN** local playback from a confirmed fall-through is playing
- **THEN** the client SHALL NOT send transport or queue commands to the former owner

### Requirement: Local Player preparation precedes ending the attachment

Before ending the attachment, the client SHALL prepare a local Player,
constructing one when the client has none. It SHALL end the attachment only
after that preparation succeeds. When preparation fails, the client SHALL keep
the attachment unchanged, report the failure, and play nothing locally. The
client's media-key target SHALL follow the Player that owns transport after the
fall-through.

#### Scenario: No local Player exists
- **WHEN** a client launched straight onto a daemon confirms a fall-through and holds no local Player
- **THEN** the client SHALL construct a local Player before ending the attachment
- **THEN** the client SHALL play the selection as local playback

#### Scenario: Preparation fails
- **WHEN** the client cannot prepare a local Player
- **THEN** the client SHALL keep the attachment unchanged
- **THEN** the client SHALL report the failure
- **THEN** the client SHALL NOT play the selection locally

## REMOVED Requirements

### Requirement: Playing locally preserves the attachment

**Reason**: A confirmed fall-through now ends the attachment rather than parking
the owner. Keeping the owner attached and commandable while local playback owns
transport required the parked-session arrangement this change drops.

**Migration**: After local playback the user reattaches to the owner through the
same means used before (Sessions panel, daemon endpoint, or Library route). The
owner's queue is unchanged by the fall-through and is re-adopted on reconnect.

### Requirement: Enqueuing locally does not change transport ownership

**Reason**: Enqueue is out of scope for the prompt. A selection the attached
owner cannot play is not staged on the client's own queue.

**Migration**: Enqueue retains existing behavior toward the attached owner, and
enqueuing while no owner is attached continues to target the client's own queue.

### Requirement: Queue availability is independent of transport ownership

**Reason**: With the attachment ended there is no second live Bound queue to keep
available and commandable during local playback.

**Migration**: Remote queue scope remains available only while an owner is
attached, and returns to Local scope with the ended attachment.

### Requirement: Player events retain owner origin

**Reason**: Only one Player session is live at a time under this design, so the
client does not need origin tagging to keep a parked owner's events away from the
local queue.

**Migration**: None required; the client drains its single local-or-attached
session as it does today.

### Requirement: Pinned row in the owner queue view

**Reason**: The owner's queue is no longer presented after a fall-through, so
there is no owner queue view in which to project a locally playing row.

**Migration**: None; the item that fell through appears in the client's Local
Bound queue.
