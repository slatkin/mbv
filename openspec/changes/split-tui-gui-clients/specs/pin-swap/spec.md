# Spec Delta

## MODIFIED Requirements

### Requirement: Launches start the Owner's own build after its binary file changes

The Pin swap, Unpin, and Open SHALL start the `mbv` program of the build that the Owner process itself runs, even after the installed `mbv` file or the installed `mbv-owner` file is replaced or removed. The Owner process SHALL keep hold of the `mbv` program file that sat beside `mbv-owner` when the Owner started, and SHALL launch TUI Clients from that held file. A Client attaches only to an Owner of the same build, so a launch SHALL NOT start a different build.

#### Scenario: Binary file replaced while the Owner runs

- **WHEN** the Owner was started from `/usr/bin/mbv-owner`, `/usr/bin/mbv` is replaced by a newer build, and the user chooses **Pin**
- **THEN** mbv SHALL start a pinned Client from the build the Owner runs
- **THEN** no "could not start" notification SHALL be sent

#### Scenario: Binary file removed while the Owner runs

- **WHEN** `/usr/bin/mbv` is deleted after the Owner started, nothing replaces it, and the user chooses **Open**
- **THEN** mbv SHALL start a terminal Client that runs the same build as the Owner process

#### Scenario: No TUI program beside the Owner program

- **WHEN** no `mbv` program sat beside `mbv-owner` when the Owner started, and the user chooses **Pin**, **Unpin**, or **Open**
- **THEN** the Owner SHALL start no Client, SHALL leave any attached Client in place, and SHALL send a desktop notification that names the missing `mbv` program
