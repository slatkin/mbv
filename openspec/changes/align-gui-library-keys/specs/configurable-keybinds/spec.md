# Spec Delta: configurable-keybinds

## ADDED Requirements

### Requirement: A GUI-only action's default chord stays clear of a TUI surface's chord

A GUI-only action — one whose gate keeps its chord out of the TUI Client — SHALL NOT declare a default chord that a TUI surface binds for another action in the same context. Where a GUI-only default collides today, the default SHALL move to a chord no TUI surface claims, and the action stays rebindable.

#### Scenario: The deep-seek chords move off the Selector cycle

- **GIVEN** the deep back and forward seek actions declare `[` and `]`
- **WHEN** the defaults are retuned
- **THEN** the declared chords name no chord the TUI's Library leaves bind, and `[` and `]` reach the Selector-row cycle in the GUI Client

#### Scenario: No GUI literal shadows a TUI chord

- **GIVEN** the GUI-only key context's canvas-fixed literals
- **WHEN** a user presses `q`, `z`, `c`, or `r` in the GUI Client while a Library destination shows
- **THEN** the shared actions the TUI binds for those chords run, and no GUI-only literal answers first

#### Scenario: A GUI-only action stays rebindable

- **GIVEN** a GUI-only action whose default moved
- **WHEN** the user assigns a chord of their own in `[keys]`
- **THEN** the GUI Client routes their chord and the TUI Client stays unchanged
