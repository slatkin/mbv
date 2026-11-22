# Spec Delta

## ADDED Requirements

### Requirement: Shared Client state uses neutral values

Shared Client state and the calls that both Clients make SHALL use neutral values only: positions in logical units or items, never terminal cells or toolkit types. Each Client SHALL keep its own size facts (terminal size, window size) and its own placement of shared content, such as where a context menu opens.

#### Scenario: A context menu opens at the pointer

- **WHEN** the user opens a context menu with a pointer press in either Client
- **THEN** the Client core builds the same menu entries for the same target in both Clients
- **AND** each Client places the menu from its own pointer position

### Requirement: The Client core names no Client

The Client core SHALL NOT branch on which Client runs, and SHALL hold no state that only one Client reads or writes. Where the Clients differ, the Client core SHALL read a neutral capability that each Client declares when it attaches, and each Client's behaviour SHALL stay as it is.

#### Scenario: A rule applies to one Client only

- **WHEN** a shared rule applies only to a Client that shows one picked queue entry at a time
- **THEN** the Client core gates it on that capability, not on which Client runs
- **AND** a Client that declares the capability gets the rule, and a Client that does not keeps its own behaviour

#### Scenario: A Client-only fact is not shared state

- **WHEN** a value is read or written by one Client only, such as the GUI Library panel collapse or the terminal size
- **THEN** that Client holds the value, and the shared Client state has no field for it
