# Spec Delta

## REMOVED Requirements

### Requirement: Local clients persist launch-scoped reconnect state

**Reason**: the requirement governs persistence by launch mode; the explicit daemon-endpoint mode
no longer exists, so the requirement is re-added governed by the `auto_reconnect` setting alone.

## ADDED Requirements

### Requirement: Auto-reconnect record persistence

A Client SHALL attempt to restore a saved auto-reconnect record on startup when the
`auto_reconnect` setting is enabled, and it SHALL write the auto-reconnect record at teardown,
including writing a clear when no remote connection is tracked at exit. The `auto_reconnect`
setting alone SHALL govern this persistence.

#### Scenario: Client starts with a saved auto-reconnect record

- **WHEN** a Client attaches to the local daemon and auto-reconnect is enabled
- **WHEN** a previous session saved an auto-reconnect record (a library route or a direct session)
- **THEN** the Client SHALL attempt to restore that connection on startup

#### Scenario: Client exits with a tracked connection

- **WHEN** a Client of the local daemon exits with a library route, direct session, or
  direct-remote connection currently tracked
- **THEN** the Client SHALL write that connection as the auto-reconnect record

#### Scenario: Client exits with nothing tracked

- **WHEN** a Client of the local daemon exits with no remote connection tracked, having
  attempted to restore a saved record on startup
- **THEN** the Client SHALL write a clear auto-reconnect record; this reflects that restoration
  was genuinely attempted and found nothing to keep, not that restoration never ran

#### Scenario: Client reconnects to a genuinely remote target mid-session

- **WHEN** a Client restores or establishes a connection to a genuinely remote daemon during its
  run
- **THEN** the Client SHALL still write that connection as the auto-reconnect record at teardown,
  even though it is no longer connected to the local daemon it was launched against
