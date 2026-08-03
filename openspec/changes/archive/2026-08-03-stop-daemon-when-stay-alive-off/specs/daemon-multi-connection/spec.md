## MODIFIED Requirements

### Requirement: Multiple concurrent ctrl client connections

The daemon SHALL accept multiple simultaneous ctrl client connections over Unix socket and
TCP. Each connection SHALL be independent and SHALL NOT evict or disconnect other ctrl
clients.

The sole lifecycle exception is deliberate daemon shutdown after an authorized trigger:
an accepted local coordinated request, `mbv -q`, operating-system termination, or tray
Quit. Shutdown disconnects every client, but the daemon SHALL first send the structured
deliberate-shutdown announcement so clients exit cleanly.

A rejected or timed-out coordinated request SHALL NOT be treated as this exception and
SHALL NOT disconnect any client.

#### Scenario: Additional clients connect

- **WHEN** one or more ctrl clients are connected and another client connects
- **THEN** every client SHALL remain connected and receive state broadcasts

#### Scenario: Ordinary commands preserve other connections

- **WHEN** one client sends a playback or queue command
- **THEN** every other client SHALL remain connected

#### Scenario: Accepted shutdown disconnects every client cleanly

- **WHEN** a permitted local client obtains shutdown acceptance while multiple clients are connected
- **THEN** every client SHALL receive the deliberate-shutdown announcement before its
  connection closes

#### Scenario: Rejected shutdown preserves every client

- **WHEN** a shutdown request is rejected because its transport is not permitted or queue
  persistence failed
- **THEN** every client SHALL remain connected and playback SHALL continue
