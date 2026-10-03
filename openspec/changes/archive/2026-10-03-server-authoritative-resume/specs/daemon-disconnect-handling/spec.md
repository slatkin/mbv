# Spec Delta

## MODIFIED Requirements

### Requirement: Recovery options behave distinctly
Each recovery option SHALL have a distinct, predictable effect.

#### Scenario: Restart and resume
- **WHEN** the user chooses to restart and resume
- **THEN** the client SHALL ensure a local daemon exists again and attach to it
- **THEN** the queue SHALL be restored from the saved queue snapshot
- **THEN** a resumed Emby or Audiobookshelf item SHALL start from the position its server holds, and a resumed feed entry SHALL start from the position in the snapshot

#### Scenario: Restart without resuming
- **WHEN** the user chooses to restart without resuming
- **THEN** the client SHALL ensure a local daemon exists again and attach to it
- **THEN** the client SHALL NOT start playback of the item that was playing when the connection was lost
- **THEN** the saved queue snapshot SHALL NOT be replayed automatically

#### Scenario: Repeated crash on the same item
- **WHEN** restarting and resuming causes the daemon to die again on the same item
- **THEN** restarting without resuming SHALL give the user a working client with playback stopped

#### Scenario: Quit
- **WHEN** the user chooses to quit
- **THEN** the client SHALL restore the terminal and exit without starting a daemon
