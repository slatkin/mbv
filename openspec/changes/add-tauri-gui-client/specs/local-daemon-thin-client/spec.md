# Spec Delta

## MODIFIED Requirements

### Requirement: Clients present the full terminal UI
A TUI Client of a local daemon SHALL present mbv's complete terminal UI — browsing, search, queue
editing, playback control, and rendering — on its own real terminal, and it SHALL perform its own
terminal capability detection. A GUI Client SHALL present, in its own native window, the surfaces
that the `gui-client` capability defines. No Client SHALL be a byte pipe to another process.

#### Scenario: Client renders on its own terminal
- **WHEN** a TUI Client attaches to a local daemon
- **THEN** the TUI Client SHALL detect its own terminal's graphics and font capabilities directly
- **THEN** album art and other graphics SHALL render according to that terminal's capabilities

#### Scenario: Terminals with different capabilities
- **WHEN** two TUI Clients on terminals with different graphics support are attached at the same time
- **THEN** each SHALL render according to its own terminal's capabilities

#### Scenario: A GUI Client renders in its own window
- **WHEN** a GUI Client attaches to a local daemon
- **THEN** it SHALL present its surfaces in its own native window
- **THEN** it SHALL NOT relay a terminal session or another process's output
