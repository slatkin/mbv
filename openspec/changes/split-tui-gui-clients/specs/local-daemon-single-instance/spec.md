# Spec Delta

## MODIFIED Requirements

### Requirement: The mismatch prompt offers restart or quit

The mismatch prompt SHALL show the application version, build identity, and wire identity of the Owner and of this Client. It SHALL state that restarting stops playback and closes any other mbv Clients attached to the Owner. It SHALL offer exactly two choices, restart and quit. There SHALL be no choice to continue against the mismatched Owner.

#### Scenario: The prompt shows both sides
- **WHEN** the mismatch prompt is shown
- **THEN** it SHALL show the application version, build identity, and wire identity of the Owner and of this Client

#### Scenario: User chooses restart
- **WHEN** the user chooses restart at the mismatch prompt
- **THEN** mbv SHALL ask the Owner to stop the same way `mbv -q` does, so the Owner persists its state and exits
- **THEN** mbv SHALL start a fresh Owner from the `mbv-owner` program beside this Client once the old one has released the lock, and attach to it
- **THEN** mbv SHALL NOT show the prompt again for the Owner that is stopping

#### Scenario: The old Owner does not exit in time
- **WHEN** the user chose restart and the old Owner is still running after the bounded wait
- **THEN** mbv SHALL exit with a non-zero status and a message that the Owner is still shutting down, naming `mbv -q`

#### Scenario: User chooses quit
- **WHEN** the user chooses quit, presses Enter alone, enters any other input, or input reaches end of file
- **THEN** mbv SHALL exit with a non-zero status without signalling the Owner
- **THEN** the message SHALL name `mbv -q` and relaunching as the way to proceed

#### Scenario: Standard input is not a terminal
- **WHEN** a mismatch is detected and standard input is not a terminal
- **THEN** mbv SHALL NOT prompt, SHALL NOT signal the Owner, and SHALL exit with a non-zero status
- **THEN** the message SHALL state the version information of both sides and name `mbv -q`

