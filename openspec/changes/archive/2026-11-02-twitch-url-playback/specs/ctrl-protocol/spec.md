# Spec Delta

## ADDED Requirements

### Requirement: Twitch item transport is additively negotiated
The ctrl hello SHALL advertise an additive capability for Twitch item queue transport without a change to `CTRL_PROTOCOL_VERSION`. A Player owner SHALL send Twitch slots only to peers that advertised the capability. It SHALL refuse a queue operation that carries a Twitch item from a peer that did not advertise it, and SHALL NOT drop the item silently.

#### Scenario: Capable peers exchange a Twitch slot
- **WHEN** both the Owner and the Client advertise the Twitch queue capability and the queue holds a Twitch item
- **THEN** the Client receives the Twitch slot in queue state

#### Scenario: Older peer receives queue state
- **WHEN** a peer without the Twitch queue capability receives queue state that holds a Twitch item
- **THEN** the Twitch slot is omitted for that peer and every other slot arrives unchanged

#### Scenario: Peer without the capability submits a Twitch item
- **WHEN** a peer that did not advertise the Twitch queue capability submits a queue operation that carries a Twitch item
- **THEN** the Owner rejects the operation with a reason and the queue does not change

#### Scenario: Client attached to an Owner without the capability
- **WHEN** the Client's Player owner did not advertise the Twitch queue capability and the user submits a Twitch URL
- **THEN** the Client does not send the item and tells the user that this Player owner cannot play Twitch items
