## MODIFIED Requirements

### Requirement: The core focus stack is the one focus authority

The Client core SHALL own which surfaces are mounted, which surface has focus, and the overlay stack. The TUI Client SHALL present focus from this stack. A Client's own framework SHALL follow the stack and SHALL NOT decide focus by itself. The GUI Client SHALL present no Panel focus: for each key it routes, the Client core SHALL set the stack's Panel focus from the key's pointer target before the keyboard router resolves the key.

#### Scenario: An overlay closes

- **WHEN** a blocking overlay is dismissed in either Client
- **THEN** the core focus stack returns focus to the surface that held it before the overlay opened
- **AND** the TUI Client presents that surface as focused on the next frame, and the GUI Client presents no Panel focus

#### Scenario: Panel focus moves

- **WHEN** the user moves Panel focus from Library to Queue in the TUI Client
- **THEN** the core focus stack records Queue as focused
- **AND** the TUI framework's focused component follows that record

#### Scenario: A GUI key sets Panel focus from its target

- **GIVEN** the GUI Client routed its last key with the Queue list box as its target
- **WHEN** the next key arrives with no target
- **THEN** the core focus stack records Library Panel focus before the router resolves that key
- **AND** the Queue surface receives no part of the key
