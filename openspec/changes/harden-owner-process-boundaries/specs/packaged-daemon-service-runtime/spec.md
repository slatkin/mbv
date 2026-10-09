# Spec Delta

## ADDED Requirements

### Requirement: The packaged daemon holds a lock on its PID file
The packaged daemon SHALL hold an exclusive advisory lock on its PID file for its whole lifetime
and SHALL write its process ID into the file after it acquires the lock. A packaged daemon that
cannot acquire the lock because another live process holds it SHALL refuse to start. Only the
packaged daemon SHALL write this file; the local daemon SHALL NOT.

#### Scenario: A second packaged daemon starts
- **WHEN** a packaged daemon is running and `mbvd` is started again with the same paths
- **THEN** the second process SHALL exit with an error stating that a daemon is already running

#### Scenario: The packaged daemon died without cleanup
- **WHEN** the packaged daemon was killed or the host rebooted, leaving its PID file behind
- **THEN** the next `mbvd` start SHALL acquire the lock and start normally

#### Scenario: A local daemon runs for the same user
- **WHEN** a local daemon runs as a user who also runs `mbvd` without the system instance
- **THEN** the local daemon SHALL NOT create, write, or remove the packaged daemon's PID file

### Requirement: `mbvd --quit` stops only the packaged daemon
`mbvd --quit` SHALL resolve the packaged daemon's system-instance paths, as the other `mbvd`
administrative actions do. It SHALL send the shutdown signal to the process ID in the PID file
only while a live process holds that file's lock. It SHALL NOT read or signal the local daemon.

#### Scenario: Quitting a running packaged daemon
- **WHEN** the packaged daemon is running and an operator with permission to signal it runs
  `mbvd --quit`
- **THEN** the packaged daemon SHALL receive the shutdown signal and mbvd SHALL report that it
  was stopped

#### Scenario: Quitting from a desktop shell while a local daemon runs
- **WHEN** a user's local daemon is running and the user runs `mbvd --quit` without
  `MBV_SYSTEM` set
- **THEN** the local daemon SHALL NOT receive a signal

#### Scenario: Only a stale PID file remains
- **WHEN** the PID file holds a process ID but no process holds its lock
- **THEN** `mbvd --quit` SHALL NOT send a signal to any process and SHALL report that no daemon
  is running with a non-zero exit status
