# Spec Delta

## MODIFIED Requirements

### Requirement: `mbv -q` stops the local daemon
`mbv -q` SHALL read the process ID from the lock file and request a graceful shutdown of the
local daemon. It SHALL NOT require a terminal UI to be attached and SHALL NOT affect clients other
than by stopping the daemon they are attached to. It SHALL send the shutdown signal only while a
live process holds the owner lock; a process ID left in an unheld lock file SHALL never be
signalled.

#### Scenario: Stopping a local daemon
- **WHEN** the user runs `mbv -q` while a local daemon owns playback
- **THEN** the daemon SHALL shut down gracefully, persisting its state
- **THEN** attached clients SHALL be notified of the deliberate shutdown

#### Scenario: Nothing is running
- **WHEN** the user runs `mbv -q` and no Player owner exists
- **THEN** mbv SHALL report that no running instance was found and SHALL exit with a non-zero status

#### Scenario: A stale process ID remains after a crash or reboot
- **WHEN** the previous Player owner died without cleanup, so the lock file still holds its
  process ID but no process holds the lock
- **WHEN** the user runs `mbv -q`
- **THEN** mbv SHALL NOT send a signal to any process
- **THEN** mbv SHALL report that no running instance was found and SHALL exit with a non-zero status

## ADDED Requirements

### Requirement: The runtime directory is private to the user
mbv's runtime files (the owner lock, the control socket, the mpv IPC socket and the mpv config
directory) SHALL live in `$XDG_RUNTIME_DIR` when it is set. Otherwise they SHALL live in a
per-user directory under `/tmp` named for the user's numeric ID, created with access for the
owner only. mbv SHALL refuse to start when that directory exists but is a symlink, is not a
directory, is owned by another user, or grants any group or other access.

#### Scenario: Two users on one machine without a runtime directory
- **WHEN** `XDG_RUNTIME_DIR` is unset for two different users on the same machine
- **WHEN** each user starts mbv
- **THEN** each user SHALL get a fresh Player owner in their own runtime directory
- **THEN** neither user SHALL block on, connect to, or remove the other user's runtime files

#### Scenario: The fallback directory does not exist
- **WHEN** `XDG_RUNTIME_DIR` is unset and the per-user directory does not exist
- **THEN** mbv SHALL create it with owner-only access and start normally

#### Scenario: Another user pre-created the fallback directory
- **WHEN** `XDG_RUNTIME_DIR` is unset and the per-user directory exists but is owned by another
  user, is a symlink, or grants group or other access
- **THEN** mbv SHALL exit with a non-zero status and an error naming the directory and the
  reason, before it takes the lock or opens any socket
