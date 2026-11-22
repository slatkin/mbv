# Spec Delta

## MODIFIED Requirements

### Requirement: The GUI Client is a separate program

mbv SHALL ship the GUI Client as a program separate from the `mbv` terminal program. The `mbv` program SHALL NOT load or link the windowing and rendering stack that the GUI Client uses. The `mbv-gui` program SHALL NOT load or link the terminal library or the terminal UI framework that the TUI Client uses. The GUI Client SHALL run no web engine process and no script engine.

#### Scenario: The terminal program carries no window stack

- **WHEN** the `mbv` program is built
- **THEN** its dependency graph contains no windowing or rendering library that the GUI Client uses

#### Scenario: The GUI program carries no terminal UI

- **WHEN** the `mbv-gui` program is built
- **THEN** its dependency graph contains no terminal library or terminal UI framework that the TUI Client uses

#### Scenario: The GUI Client starts on its own

- **WHEN** the user starts the GUI Client program
- **THEN** it opens one native window and presents mbv in it without a terminal

#### Scenario: The GUI Client is one process

- **WHEN** the GUI Client window is open
- **THEN** the GUI Client runs as one process with no web engine child process
