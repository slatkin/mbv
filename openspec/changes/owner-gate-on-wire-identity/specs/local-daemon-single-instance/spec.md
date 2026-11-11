# Spec Delta

## REMOVED Requirements

### Requirement: A local Client never attaches to an Owner from a different build
**Reason**: The build identity changes with every commit and every dirty build, so a Client-only rebuild that leaves the wire as it was is refused. The gate now compares the application version and the wire identity.
**Migration**: See "A local Client never attaches to an Owner with a different version or wire" and "The mismatch prompt offers restart or quit". The build identity stays in the hello and shows in Settings.

## ADDED Requirements

### Requirement: A local Client never attaches to an Owner with a different version or wire

A Client attaching to the local Owner SHALL compare the application version and wire identity from the Owner's hello with its own. The build identity SHALL NOT take part. A missing Owner wire identity SHALL count as a mismatch. On a mismatch the Client SHALL stop before it sends its hello, so that no control credential leaves it, and SHALL show the mismatch prompt. The rule SHALL apply only to the local Owner, and SHALL NOT alter the `ctrl-protocol` version check.

#### Scenario: Owner and Client have the same version and wire
- **WHEN** the Owner's application version and wire identity both equal the Client's
- **THEN** the Client SHALL attach as it would without this requirement
- **THEN** no prompt SHALL be shown

#### Scenario: Same version and wire, different build identity
- **WHEN** a Client attaches to a running Owner whose application version and wire identity equal its own but whose build identity differs
- **THEN** the Client SHALL attach as it would without this requirement
- **THEN** no prompt, toast, or status text SHALL report the different build identity

#### Scenario: Application versions differ
- **WHEN** a Client attaches to a running Owner whose application version differs from its own
- **THEN** the Client SHALL NOT send its hello or control credential to that Owner
- **THEN** the Client SHALL show the mismatch prompt before any user interface starts

#### Scenario: Same version, different wire identity
- **WHEN** a Client attaches to a running Owner whose application version equals its own but whose wire identity differs
- **THEN** the Client SHALL NOT send its hello or control credential to that Owner
- **THEN** the Client SHALL show the mismatch prompt before any user interface starts

#### Scenario: The Owner's hello reports no wire identity
- **WHEN** a Client attaches to a running Owner whose hello reports no wire identity
- **THEN** the Client SHALL NOT send its hello or control credential to that Owner
- **THEN** the Client SHALL show the mismatch prompt, which names the Owner's wire identity as unknown, before any user interface starts

#### Scenario: Only the ctrl protocol version differs
- **WHEN** the application versions and wire identities are equal but the Owner reports a different ctrl protocol version
- **THEN** the Client SHALL NOT show the mismatch prompt
- **THEN** the refusal SHALL be decided by the `ctrl-protocol` capability exactly as without this requirement

#### Scenario: Explicit endpoint reports a different version
- **WHEN** a Client launched with an explicit `unix://` or `tcp://` endpoint receives a hello from a different application version or wire identity
- **THEN** the Client SHALL NOT show the mismatch prompt and SHALL NOT signal any process
- **THEN** ctrl protocol compatibility SHALL be decided by the `ctrl-protocol` capability alone

### Requirement: The mismatch prompt offers restart or quit

The mismatch prompt SHALL show the application version, build identity, and wire identity of the Owner and of this Client. It SHALL state that restarting stops playback and closes any other mbv Clients attached to the Owner. It SHALL offer exactly two choices, restart and quit. There SHALL be no choice to continue against the mismatched Owner.

#### Scenario: The prompt shows both sides
- **WHEN** the mismatch prompt is shown
- **THEN** it SHALL show the application version, build identity, and wire identity of the Owner and of this Client

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
- **THEN** the message SHALL state the version information of both sides and name `mbv -q`

### Requirement: Settings shows the version information of the Client and the Owner

The Session section of Settings SHALL show two read-only rows in the TUI Client and in the GUI Client. One row SHALL show this Client's application version, build identity, and wire identity. The other SHALL show the same three values from the connected Owner's hello. Both rows SHALL always show, also when the values are equal. Activating a row SHALL change nothing and save nothing.

#### Scenario: Settings is opened while attached
- **WHEN** the user opens the Session section of Settings while the Client is attached to an Owner
- **THEN** the Client row SHALL show this Client's application version, build identity, and wire identity
- **THEN** the Owner row SHALL show the application version, build identity, and wire identity from that Owner's hello

#### Scenario: The Owner is a different build of the same version and wire
- **WHEN** the attached Owner reports a build identity different from this Client's
- **THEN** the two rows SHALL show the two different build identities
- **THEN** no other surface SHALL report the difference

#### Scenario: The Owner predates the wire identity
- **WHEN** the attached Owner's hello reports no build identity or no wire identity
- **THEN** the Owner row SHALL show that value as unknown

#### Scenario: A version row is activated
- **WHEN** the user activates either version row
- **THEN** no setting SHALL change and no configuration save SHALL be scheduled

#### Scenario: The Client is attached through an explicit endpoint
- **WHEN** the Client is attached to a packaged mbvd or an explicit `unix://` or `tcp://` endpoint
- **THEN** the Owner row SHALL show the values from that Owner's hello
