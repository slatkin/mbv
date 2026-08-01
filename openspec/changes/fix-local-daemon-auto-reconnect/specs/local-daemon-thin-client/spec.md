## MODIFIED Requirements

### Requirement: Clients persist the state bare mode persists

A client of a same-host local daemon SHALL perform the same session-state persistence a bare-mode
mbv performs: it SHALL attempt to restore a saved auto-reconnect record on startup, and it SHALL
write the auto-reconnect record at teardown — including writing a clear when no remote connection
is tracked at exit — exactly as a bare-mode instance does. This persistence SHALL be governed by
how the client was launched (attached to a same-host local daemon, vs. an explicit genuinely remote
daemon endpoint), not by which connection type it happens to be using at the moment of exit.
Persistence SHALL be skipped only for clients launched against a genuinely remote daemon.

#### Scenario: Local-daemon client starts with a saved auto-reconnect record

- **WHEN** a client attaches to a same-host local daemon and auto-reconnect is enabled
- **WHEN** a previous session saved an auto-reconnect record (a library route or a direct session)
- **THEN** the client SHALL attempt to restore that connection on startup, as a bare-mode instance
  would

#### Scenario: Local-daemon client exits with a tracked connection

- **WHEN** a client of a same-host local daemon exits with a library route, direct session, or
  direct-remote connection currently tracked
- **THEN** the client SHALL write that connection as the auto-reconnect record, as a bare-mode
  instance would

#### Scenario: Local-daemon client exits with nothing tracked

- **WHEN** a client of a same-host local daemon exits with no remote connection tracked, having
  attempted to restore a saved record on startup
- **THEN** the client SHALL write a clear auto-reconnect record, as a bare-mode instance would —
  this reflects that restoration was genuinely attempted and found nothing to keep, not that
  restoration never ran

#### Scenario: Local-daemon client reconnects to a genuinely remote target mid-session

- **WHEN** a client launched attached to a same-host local daemon restores or establishes a
  connection to a genuinely remote daemon during its run
- **THEN** the client SHALL still write that connection as the auto-reconnect record at teardown,
  even though it is no longer connected to the same-host local daemon it was launched against

#### Scenario: Remote-daemon client exits

- **WHEN** a client launched against a daemon on another machine exits
- **THEN** the client SHALL NOT write the auto-reconnect record and SHALL NOT clear an existing one
