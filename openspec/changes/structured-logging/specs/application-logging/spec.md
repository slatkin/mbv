## MODIFIED Requirements

### Requirement: Log level flag
`mbv` and `mbvd` SHALL accept `--log-level <spec>`, where `<spec>` is a comma-separated list
of directives. A directive is either a bare level (`error`, `warn`, `info`, `debug`,
`trace`), which sets the default level, or `<target>=<level>`, which sets the level for one
log target and the targets nested under it. Messages SHALL be recorded when their level is
at or above the most specific matching directive. A spec made of a single bare level SHALL
behave exactly as that level did before this change. An unrecognised level, an empty
directive, or a malformed directive SHALL be rejected with a usage error. The local daemon
SHALL use the whole spec of the `mbv` process that spawns it.

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
- **WHEN** `mbv --log-level trace` runs
- **THEN** trace-level lines are recorded and labelled `level=trace`

#### Scenario: Invalid value
- **WHEN** `mbvd --log-level loud` runs
- **THEN** it prints usage and exits non-zero

#### Scenario: Malformed directive
- **WHEN** `mbvd --log-level info,player=` runs
- **THEN** it prints usage and exits non-zero

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

## ADDED Requirements

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
- ctrl playback intents: `request` (playback request id) and `generation`
- ctrl queue loads: `queue_request` (queue load request id)
- playback sessions: `slot` (queue slot id), `item` (content id, when there is one) and,
  for Emby, `play_session`
- HTTP requests to a Service: `service`, `http.method`, `url.path`, and once known
  `http.status` and `duration_ms`
Every line logged inside such an operation SHALL carry that operation's fields, including
lines from code that does not know about the operation.

#### Scenario: One intent across processes
- **WHEN** the TUI sends a playback intent and the daemon accepts and starts it
- **THEN** the TUI's send line in `mbv.log` and the daemon's accept and start lines in its
  own log all carry the same `request=` value

#### Scenario: Nested line inherits playback context
- **WHEN** an Emby progress report fails during playback of slot 7
- **THEN** the failure line carries `slot=7` even though the report code does not log it

### Requirement: Credentials are never logged
Log output SHALL NOT contain access tokens, API keys, passwords or the ctrl control
credential. URL fields SHALL have credential-carrying query parameters (`api_key`,
`X-Emby-Token`, `token`) replaced with `REDACTED`.

#### Scenario: Stream URL with token
- **WHEN** a playback event logs an Emby stream URL that contains `api_key=abc123`
- **THEN** the logged value contains `api_key=REDACTED` and no `abc123`

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
