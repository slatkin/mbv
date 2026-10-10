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
`mbv` and `mbvd` SHALL accept `--log-level <spec>`, where `<spec>` is a comma-separated list
of directives. A directive is either a bare level (`error`, `warn`, `info`, `debug`,
`trace`), which sets the default level, or `<target>=<level>`, which sets the level for one
log target and the targets nested under it. Messages SHALL be recorded when their level is
at or above the most specific matching directive. A bare level sets the default threshold.
Module-path targets containing `::` outside `mbv` and `mbv_*` SHALL be capped at `warn`
unless a matching target directive names them. An unrecognised level, an empty directive,
or a malformed directive SHALL be rejected with a usage error. The local daemon SHALL use
the whole spec of the `mbv` process that spawns it.

#### Scenario: Debug opt-in
- **WHEN** `mbvd --log-level debug` runs
- **THEN** debug-level lines are recorded

#### Scenario: Local daemon inherits level
- **WHEN** `mbv --log-level info,player=debug` spawns the local daemon
- **THEN** the local daemon records debug-level lines from the `player` target and only
  info-level and above from every other mbv target

#### Scenario: Per-target override
- **WHEN** `mbv --log-level warn,api=debug` runs
- **THEN** debug-level `api` lines are recorded and info-level lines from other targets are not

#### Scenario: Trace opt-in
- **WHEN** `mbv --log-level trace` runs and an `mbv` trace-level event is emitted
- **THEN** the line is recorded and labelled `level=trace`

#### Scenario: Invalid value
- **WHEN** `mbvd --log-level loud` runs
- **THEN** it prints usage and exits non-zero

#### Scenario: Malformed directive
- **WHEN** `mbvd --log-level info,player=` runs
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
its level: `<3>` error, `<4>` warn, `<6>` info, `<7>` debug and trace. Log file lines SHALL
NOT carry the prefix.

#### Scenario: Warning under journald
- **WHEN** the systemd `mbvd` logs a warning
- **THEN** journald records it at priority 4

#### Scenario: Trace under journald
- **WHEN** the systemd `mbvd` runs with `--log-level trace` and logs a trace-level line
- **THEN** journald records it at priority 7

### Requirement: Systemd daemon logs only to journald
The system-instance `mbvd` SHALL write its log only to stderr (journald) and SHALL NOT
create or write a log file. Other instances keep their log files.

#### Scenario: No daemon log file
- **WHEN** the system-instance `mbvd` runs
- **THEN** no `mbv.log` is created in its data directory

### Requirement: Log lines are logfmt with named fields
Each log line SHALL be one line of logfmt that starts with `ts`, `level` and `source` (the
log target), in that order. When the event has a name, `event` comes next. Then come the
event's own fields, then the fields of each span enclosing the event from outermost to
innermost, and `msg` comes last. Values containing spaces, `=`, `"` or control
characters SHALL be double-quoted, with `\`, `"` and newlines escaped. Other values SHALL
be written bare. Context values an event records (identifiers, URLs, statuses, durations,
counts, errors) SHALL be separate fields, not text inside `msg`. Messages from third-party
libraries SHALL use the same line shape.

#### Scenario: Context is filterable by key
- **WHEN** playback of queue slot 12 fails to load
- **THEN** the error line contains `slot=12` as its own field, and filtering the log for
  `slot=12` finds it

#### Scenario: Message with quotes and newline
- **WHEN** an event's message contains `"` and a newline
- **THEN** the line stays on one line and `msg` is double-quoted with both characters escaped

### Requirement: Timestamps are full RFC 3339 with milliseconds
The `ts` field SHALL be the local wall-clock time in RFC 3339 form with the date,
millisecond precision and the numeric UTC offset, e.g. `2026-09-28T14:03:12.345+02:00`.
The same format SHALL be used in log files and on stderr.

#### Scenario: Burst ordering
- **WHEN** two events are logged 5 ms apart in the same second
- **THEN** their `ts` values differ and sort in the order the events happened

#### Scenario: Cross-day correlation
- **WHEN** a log file spans midnight
- **THEN** every line's `ts` carries its calendar date

### Requirement: Converted events are named
Every event emitted by mbv code SHALL carry an event name in dot notation
`<component>.<operation>.<state>` (for example `player.load.failed`), written as the
`event` field. Event names SHALL be stable identifiers: the same situation SHALL produce the
same name in every process.

#### Scenario: Grouping by event
- **WHEN** a user filters a log for `event=queue.load.rejected`
- **THEN** every queue-load rejection is found, and no other event

### Requirement: Correlation fields
Log lines SHALL carry correlation fields for the operation they belong to, using the same
key names in `mbv`, the local daemon and `mbvd`:
- ctrl playback intents: `request` (playback request id) and `generation`. On the Player
  owner, also `client` (the owner's id for the connection that sent it).
- ctrl queue loads: `queue_request` (queue load request id). On the Player owner, also
  `client`.
- playback sessions: `slot` (queue slot id), `item` (content id, when there is one) and,
  for Emby, `play_session`
- Emby playback reporting: `item` and `play_session` of the session being reported
- HTTP requests to a Service: `service`, `http.request.method`, `url.path`, and once
  known `http.response.status_code` and `duration_ms`
Every line logged inside such an operation SHALL carry that operation's fields. This
includes lines from code that does not know about the operation, lines logged on worker
threads the operation starts, and lines logged when a deferred result of the operation
is handled later.

Request ids are unique only within one client connection. So that lines can be matched
across processes:
- After a client completes the Control handshake and is registered, the Player owner SHALL
  log one line with `client` and `peer`: the peer's process id for a local socket, or the
  peer's `ip:port` for a network connection.
- Each time `mbv` connects to a Player owner, it SHALL log one line with `peer` in the
  same format: its own process id for a local socket, or its local `ip:port` for a
  network connection.
- An equal `peer=` value on the two lines SHALL identify the same connection. This is
  guaranteed for local sockets, and for network connections with no address translation
  between `mbv` and the owner.

#### Scenario: One intent across processes
- **WHEN** the TUI sends a playback intent and the daemon accepts and starts it
- **THEN** the TUI's send line in `mbv.log` and the daemon's accept and start lines in its
  own log all carry the same `request=` value, and the daemon's lines carry the `client=`
  whose connect line has the same `peer=` as the TUI's connect line

#### Scenario: Remote TUI over the network
- **WHEN** a TUI connects to `mbvd` over TCP with no address translation in between
- **THEN** the TUI's connect line `peer=` (its local `ip:port`) equals the `peer=` on
  the owner's connect line for that client

#### Scenario: Two clients reuse a request id
- **WHEN** two TUIs attached to the same `mbvd` each send a playback intent with request
  id 1
- **THEN** the owner's lines for the two intents carry different `client=` values, and
  each `client=` connect line names a different `peer`

#### Scenario: Deferred resolution keeps context
- **WHEN** a playback intent's item lookup fails on the lookup worker and the failure is
  handled back on the owner's event loop
- **THEN** the failure line carries the intent's `client=` and `request=`

#### Scenario: Reporting worker inherits session context
- **WHEN** an Emby progress report fails on a reporting worker thread
- **THEN** the failure line carries the `item=` and `play_session=` of the session being
  reported, even though the report code does not log them

#### Scenario: Deferred start is not mislabelled
- **WHEN** a start report for a new item is still resolving its play session while the
  previous session's ids are still current
- **THEN** its lines carry the new item's `item=`, and no `play_session=` until the new
  session is known. No part of those lines carries the previous session's `slot`, `item`
  or `play_session` values.

### Requirement: Credentials are never logged
Log output SHALL NOT contain access tokens, API keys, passwords, bearer tokens or the ctrl
control credential. Wherever a URL appears in a log line, in a field value or in the
message, it SHALL be written without its username/password, query string and fragment.
A `Bearer <token>` value SHALL be written as `Bearer REDACTED`. Request headers, request
bodies and credential or configuration values SHALL NOT be logged.

#### Scenario: Stream URL with token
- **WHEN** a playback event logs an Emby stream URL that contains `?api_key=abc123`
- **THEN** the logged value ends at the URL's path and contains no `abc123`

#### Scenario: Unlisted credential shape
- **WHEN** a feed enclosure URL `https://host/media.mp3?password=secret` is logged
- **THEN** the logged value is `https://host/media.mp3` and contains no `secret`

#### Scenario: Credentials in the URL itself
- **WHEN** a URL `https://user:pass@host/feed` appears in an error message
- **THEN** the logged message contains `https://host/feed` and neither `user:pass` nor
  `pass`

### Requirement: Log file failures are reported
When a process configured with a log file cannot create its directory, open the file or
rotate it, it SHALL write one warning to stderr naming the path and the OS error. It SHALL
keep running, still logging to stderr if stderr logging is enabled.

#### Scenario: Unwritable state directory
- **WHEN** `mbv` starts and `mbv.log` cannot be opened
- **THEN** one warning naming the path and the error is printed to stderr and `mbv` starts
  normally

### Requirement: Log files rotate by size
A log file SHALL be rotated when writing to it would take it past 5 MB, both at startup and
while the process runs. Rotation SHALL keep the three most recent previous files as
`<name>.log.1` (newest) to `<name>.log.3` (oldest), deleting anything older.

#### Scenario: Long-running daemon rotates
- **WHEN** a local daemon runs long enough to write more than 5 MB to `local-daemon.log`
- **THEN** the file is rotated to `local-daemon.log.1` without a restart, and later lines go
  to a new `local-daemon.log`

#### Scenario: Generations are bounded
- **WHEN** a log file rotates while `.log.1` to `.log.3` already exist
- **THEN** the old `.log.3` is deleted, the others shift by one, and there are never more
  than three rotated files

### Requirement: The mbv log lives in the process's state directory
`mbv` SHALL write `mbv.log` and its crash line in the same state directory that holds the rest
of that process's state, resolved by the one shared state-directory rule. That rule includes the
system-instance directory and a fallback that is never relative to the working directory. `mbv`
SHALL create that directory if it is missing before it writes the crash line.

#### Scenario: System instance
- **WHEN** `mbv` runs with `MBV_SYSTEM=1`
- **THEN** `mbv.log` and the crash line SHALL be written under the system-instance state
  directory, next to that process's stored Service credentials

#### Scenario: HOME is unset
- **WHEN** `mbv` runs with neither `XDG_STATE_HOME` nor `HOME` set
- **THEN** `mbv.log` SHALL be written under the same absolute fallback state directory as the
  rest of its state, never under the working directory

#### Scenario: Crash before the state directory exists
- **WHEN** a `--pin` launch crashes before anything has created the state directory
- **THEN** the directory SHALL be created and the crash line SHALL be written to `mbv.log`
