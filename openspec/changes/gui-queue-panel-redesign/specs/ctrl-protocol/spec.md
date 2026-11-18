# Spec Delta

## ADDED Requirements

### Requirement: The Player status carries the buffered position additively

The Player status SHALL carry the buffered position: the media position up to which the local player holds cached media, in the same units as the playback position. Zero SHALL mean that no buffered position is known. The field SHALL be additive: a status without it SHALL read as zero, and the protocol version SHALL NOT change. A Player owner that sends the field SHALL advertise a buffered-position capability in its hello.

#### Scenario: The local Owner reports its cache

- **GIVEN** the local Owner plays a file and mpv holds cached media to 40 minutes
- **WHEN** the Owner sends a Player status to a Client
- **THEN** the status carries the buffered position at 40 minutes
- **AND** the Owner's hello advertises the buffered-position capability

#### Scenario: An older Owner sends no buffered position

- **GIVEN** a Player owner that does not know the field
- **WHEN** a Client decodes its Player status
- **THEN** the buffered position reads as zero and the connection proceeds as before

#### Scenario: An older Client receives the field

- **GIVEN** a Client that does not know the field
- **WHEN** it decodes a Player status that carries the buffered position
- **THEN** it ignores the field and the connection proceeds as before

#### Scenario: The output reports no cache

- **GIVEN** the Owner plays through a Cast receiver or an Emby Session
- **WHEN** the Owner sends a Player status
- **THEN** the buffered position is zero
