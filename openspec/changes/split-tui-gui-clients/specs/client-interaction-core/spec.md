# Spec Delta

## ADDED Requirements

### Requirement: The Client core is built without any Client toolkit

The Client core (the shared Client state, command dispatch, keyboard router, Client tick, and Owner find, start, and attach) SHALL build as its own unit. Its dependency graph SHALL contain no terminal library, no terminal UI framework, and no GUI toolkit.

#### Scenario: The Client core is built alone

- **WHEN** the Client core is built alone
- **THEN** its dependency graph contains no terminal library, terminal UI framework, or GUI toolkit

### Requirement: Shared Client state uses neutral values

Shared Client state and the calls that both Clients make SHALL use neutral values only: positions in logical units or items, never terminal cells or toolkit types. Each Client SHALL keep its own size facts (terminal size, window size) and its own placement of shared content, such as where a context menu opens.

#### Scenario: A context menu opens at the pointer

- **WHEN** the user opens a context menu with a pointer press in either Client
- **THEN** the Client core builds the same menu entries for the same target in both Clients
- **AND** each Client places the menu from its own pointer position

### Requirement: No Client contains another Client's code

The TUI Client SHALL NOT depend on GUI Client code, and the GUI Client SHALL NOT depend on TUI Client code. Code that both Clients use SHALL live once, in the Client core or in a shared crate below it, and both Clients SHALL import it from there. Code and state that only one Client uses SHALL live in that Client. The shared Client state SHALL hold no field and no event that only one Client reads.

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
