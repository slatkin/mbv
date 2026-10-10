# Spec Delta

## ADDED Requirements

### Requirement: Silent and stalled ctrl connections are released
The daemon SHALL close an accepted ctrl connection that sends no complete hello within a bounded
deadline, over Unix socket and TCP. After a connection is admitted, the daemon SHALL NOT close it
for being idle. The daemon SHALL close an admitted connection whose peer stops reading for longer
than a bounded write timeout, and SHALL then remove it as for any send failure, so that queued
broadcasts for that peer stop growing.

#### Scenario: A peer connects and never sends a hello
- **WHEN** a peer opens a ctrl connection and sends nothing
- **THEN** the daemon SHALL close the connection once the hello deadline passes
- **THEN** no thread or socket SHALL remain held for that connection

#### Scenario: An attached client is idle
- **WHEN** an admitted client sends no command for longer than the hello deadline
- **THEN** the client SHALL remain connected and SHALL keep receiving broadcasts

#### Scenario: An attached client stops reading
- **WHEN** an admitted client's socket stops accepting data (for example, its host sleeps or the
  link drops packets) while the daemon keeps broadcasting status
- **THEN** the daemon SHALL close that connection once the write timeout passes
- **THEN** every other client SHALL remain connected and SHALL keep receiving broadcasts
