# Spec Delta

## Purpose

Defines the separate program that hosts the per-user Owner process: how every local Client finds and starts it, what it contains, and how it launches TUI Clients of its own build.

## ADDED Requirements

### Requirement: The Owner process runs from its own program

mbv SHALL ship a program named `mbv-owner` that hosts the per-user Owner process. The Owner process SHALL run only from `mbv-owner`. The `mbv` program and the `mbv-gui` program SHALL NOT host the Owner process. `mbv-owner` SHALL contain no Client: its dependency graph SHALL contain no terminal UI library and no GUI toolkit. The packaged `mbvd` daemon SHALL stay a separate program.

#### Scenario: The Owner program carries no Client

- **WHEN** the `mbv-owner` program is built
- **THEN** its dependency graph contains no terminal UI library and no GUI toolkit

#### Scenario: The TUI program does not host the Owner

- **WHEN** the `mbv` program is started with any argument
- **THEN** it never runs as the Owner process

### Requirement: Clients start the Owner program beside them

A local Client that finds no Owner process SHALL start `mbv-owner` from the directory of its own program when `mbv-owner` is there, and else SHALL start the `mbv-owner` name on `PATH`. The TUI Client and the GUI Client SHALL use the same rule. The started Owner process SHALL detach from the Client's terminal and session.

#### Scenario: The TUI Client starts the Owner

- **WHEN** the user starts `mbv` and no Owner process is running
- **THEN** mbv starts `mbv-owner` from the directory of the `mbv` program and attaches to it

#### Scenario: The GUI Client starts the Owner

- **WHEN** the user starts `mbv-gui` and no Owner process is running
- **THEN** the GUI Client starts `mbv-owner` from the directory of the `mbv-gui` program and attaches to it

#### Scenario: A TUI Client and a GUI Client share one Owner

- **GIVEN** Stay-alive is enabled and the GUI Client started the Owner process
- **WHEN** the user starts `mbv`
- **THEN** the TUI Client attaches to that Owner process and starts no second Owner

### Requirement: Every release artifact installs the Owner program

Every release artifact (the tarball, the `.deb`, and the Arch packages) SHALL install `mbv-owner` in the same directory as `mbv` and `mbv-gui`. All three programs SHALL come from one build.

#### Scenario: A package installs the three programs

- **WHEN** the tarball, the `.deb`, or an Arch package is installed
- **THEN** `mbv`, `mbv-gui`, and `mbv-owner` are installed in the same directory
