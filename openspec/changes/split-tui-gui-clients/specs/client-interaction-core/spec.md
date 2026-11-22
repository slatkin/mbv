# Spec Delta

## ADDED Requirements

### Requirement: The Client core is built without any Client toolkit

The Client core (the shared Client state, command dispatch, keyboard router, Client tick, and Owner find, start, and attach) SHALL build as its own unit. Its dependency graph SHALL contain no terminal library, no terminal UI framework, and no GUI toolkit.

#### Scenario: The Client core is built alone

- **WHEN** the Client core is built alone
- **THEN** its dependency graph contains no terminal library, terminal UI framework, or GUI toolkit

### Requirement: No Client contains another Client's code

The TUI Client SHALL NOT depend on GUI Client code, and the GUI Client SHALL NOT depend on TUI Client code. Code that both Clients use SHALL live once, in the Client core or in a shared crate below it, and both Clients SHALL import it from there. Code that only one Client uses SHALL live in that Client.

#### Scenario: The GUI program carries no terminal UI

- **WHEN** the `mbv-gui` program is built
- **THEN** its dependency graph contains no terminal library and no terminal UI framework

#### Scenario: The TUI program carries no window stack

- **WHEN** the `mbv` program is built
- **THEN** its dependency graph contains no GUI toolkit and no GUI Client code

#### Scenario: Behaviour both Clients share changes once

- **WHEN** a rule that both Clients follow changes, such as how a selection is enqueued
- **THEN** the change is made once, in the Client core, and both Clients show the new behaviour

#### Scenario: A GUI-only feature changes

- **WHEN** a GUI Library panel feature changes and it uses only existing shared behaviour
- **THEN** no code in the Client core or the TUI Client changes
