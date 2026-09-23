# application-logging Specification

## Purpose
Controls how much mbv, the local daemon and mbvd log, and how their log lines are
labelled so a service manager can filter them by severity.

## Requirements

### Requirement: Info is the default log level
Every mbv binary SHALL record only info, warn and error messages unless a lower level is
requested. Debug messages SHALL NOT reach any log sink by default.

#### Scenario: Default run omits debug
- **WHEN** `mbv` or `mbvd` runs without `--log-level`
- **THEN** its log contains no debug-level lines

### Requirement: Log level flag
`mbv` and `mbvd` SHALL accept `--log-level <error|warn|info|debug>`, recording messages at
that level and above. An unrecognised value SHALL be rejected with a usage error. The
local daemon SHALL use the level of the `mbv` process that spawns it.

#### Scenario: Debug opt-in
- **WHEN** `mbvd --log-level debug` runs
- **THEN** debug-level lines are recorded

#### Scenario: Local daemon inherits level
- **WHEN** `mbv --log-level debug` spawns the local daemon
- **THEN** the local daemon records debug-level lines

#### Scenario: Invalid value
- **WHEN** `mbvd --log-level loud` runs
- **THEN** it prints usage and exits non-zero

### Requirement: Routine traffic is debug-only
Routine protocol traffic SHALL be logged at debug level: each inbound websocket message,
each progress report and its reply, each ping and its reply, and the Capabilities
request and its reply. Failures of these requests SHALL keep their warn/error level.

#### Scenario: Idle session at default level
- **WHEN** a playback session runs at the default level while the server sends
  websocket messages and the client sends progress and pings
- **THEN** none of that traffic appears in the log

#### Scenario: Ping failure still visible
- **WHEN** a ping fails at the default level
- **THEN** the failure is logged at warn or error

### Requirement: Stderr lines carry syslog priority
Every log line written to stderr SHALL begin with the systemd/syslog priority prefix for
its level: `<3>` error, `<4>` warn, `<6>` info, `<7>` debug. Log file lines SHALL NOT
carry the prefix.

#### Scenario: Warning under journald
- **WHEN** the systemd `mbvd` logs a warning
- **THEN** journald records it at priority 4

### Requirement: Systemd daemon logs only to journald
The system-instance `mbvd` SHALL write its log only to stderr (journald) and SHALL NOT
create or write a log file. Other instances keep their log files.

#### Scenario: No daemon log file
- **WHEN** the system-instance `mbvd` runs
- **THEN** no `mbv.log` is created in its data directory
