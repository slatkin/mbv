## ADDED Requirements

### Requirement: A local Client never attaches to an Owner from a different build
A Client attaching to the local Owner process SHALL compare the Owner's application version
and ctrl protocol version, as reported in the Owner's hello, with its own. When either differs,
the Client SHALL NOT attach: it SHALL stop before sending its own hello, so that no control
credential is transmitted to the Owner, no client is admitted, and no queue or playback state is
received. Instead it SHALL ask the user, before any terminal UI starts, whether to stop the
Owner and relaunch it from this binary. This rule SHALL apply only to the local Owner process;
a Client launched against an explicit `unix://` or `tcp://` endpoint SHALL NOT apply it.

The prompt SHALL show the Owner's version and this terminal's version (and both ctrl protocol
versions when those differ), SHALL state that restarting stops playback and closes any other
mbv terminals attached to the Owner, and SHALL offer exactly two choices: restart, or quit.
There SHALL be no choice to continue against the mismatched Owner.

#### Scenario: Owner and Client are the same build
- **WHEN** the Owner's application version and ctrl protocol version equal the Client's
- **THEN** the Client SHALL attach as it would without this requirement
- **THEN** no prompt SHALL be shown

#### Scenario: Application versions differ
- **WHEN** a Client attaches to a running Owner whose application version differs from its own
- **THEN** the Client SHALL NOT send its hello or control credential to that Owner
- **THEN** the terminal SHALL show the mismatch prompt with both versions before any UI starts

#### Scenario: Only the ctrl protocol version differs
- **WHEN** the application versions are equal but the Owner reports a different ctrl protocol version
- **THEN** the Client SHALL treat it as a mismatch and show the same prompt
- **THEN** the prompt SHALL also show both ctrl protocol versions

#### Scenario: User chooses restart
- **WHEN** the user chooses restart at the mismatch prompt
- **THEN** mbv SHALL ask the Owner to stop the same way `mbv -q` does, so the Owner persists its state and exits
- **THEN** mbv SHALL start a fresh Owner from this binary once the old one has released the lock, and attach to it
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
- **THEN** the message SHALL state both versions and name `mbv -q`

#### Scenario: Explicit endpoint reports a different version
- **WHEN** a Client launched with an explicit `unix://` or `tcp://` endpoint receives a hello from a different application version
- **THEN** the Client SHALL NOT show the mismatch prompt and SHALL NOT signal any process
- **THEN** ctrl protocol compatibility SHALL be decided by the `ctrl-protocol` capability alone
