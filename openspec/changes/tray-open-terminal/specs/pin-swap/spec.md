# Spec Delta

## ADDED Requirements

### Requirement: The Tray offers Open while no Client is attached

The Tray SHALL show an **Open** item only while no local Client is attached to the Owner process.
Owner-action and admin connections and Clients attached over TCP SHALL NOT count as attached
Clients. The item SHALL reflect the attached Clients each time the menu opens. With no Client
attached, the menu SHALL list **Open** directly above **Pin**.

#### Scenario: No Client attached

- **WHEN** Stay-alive keeps the Owner process running with no Client attached and the user opens
  the Tray menu
- **THEN** the menu SHALL show **Open** directly above **Pin**, and no **Unpin** item

#### Scenario: A terminal Client is attached

- **WHEN** one unpinned Client is attached and the user opens the Tray menu
- **THEN** the menu SHALL NOT show **Open**

#### Scenario: A pinned Client is attached

- **WHEN** one pinned Client is attached and the user opens the Tray menu
- **THEN** the menu SHALL NOT show **Open**

#### Scenario: Only a TCP Client is attached

- **WHEN** a Client is attached over TCP only and the user opens the Tray menu
- **THEN** the menu SHALL show **Open**

### Requirement: Open starts a terminal Client

Choosing **Open** SHALL start a terminal that runs `mbv` as a new Client, with the same terminal
command that **Unpin** uses. Open SHALL end no Client. The Owner process SHALL run Open also when
a Client is attached. With neither `[panel] terminal` nor `TERMINAL` set, Open SHALL start nothing
and SHALL send the same desktop notification as **Unpin**.

#### Scenario: Open with TERMINAL set

- **WHEN** no Client is attached, `[panel] terminal` is unset, `TERMINAL=ghostty`, and the user
  chooses **Open**
- **THEN** mbv SHALL run `ghostty -e mbv`
- **THEN** the new Client SHALL attach and playback SHALL be unaffected

#### Scenario: Open while a Client is attached

- **WHEN** one unpinned Client is attached and the user runs `mbv --open`
- **THEN** a second terminal Client SHALL start
- **THEN** the first Client SHALL stay attached

#### Scenario: Open with no terminal command

- **WHEN** `[panel] terminal` and `TERMINAL` are both unset and the user chooses **Open**
- **THEN** no process SHALL start
- **THEN** a desktop notification SHALL name `[panel] terminal` and `TERMINAL`

### Requirement: Open shares the Pin swap launch rules

Open and the Pin swap SHALL share one launch slot: only one of them SHALL run at a time. If the
new Client cannot be started, exits with a failure before it attaches, or does not attach within
10 seconds, the Owner process SHALL send a desktop notification that names the reason.

#### Scenario: Open while a launch runs

- **WHEN** a Pin swap or an Open is in progress and the user chooses **Open**
- **THEN** no second process SHALL start

#### Scenario: Open never attaches

- **WHEN** the started terminal does not attach a Client within 10 seconds
- **THEN** a desktop notification SHALL say that opening mbv timed out

### Requirement: Launches start the Owner's own build after its binary file changes

The Pin swap, Unpin, and Open SHALL start the binary that the Owner process itself runs, even
after the file the Owner was started from is replaced or removed. A Client attaches only to an
Owner of the same build, so a launch SHALL NOT start a different build.

#### Scenario: Binary file replaced while the Owner runs

- **WHEN** the Owner was started from `/usr/bin/mbv`, that file is replaced by a newer build, and
  the user chooses **Pin**
- **THEN** mbv SHALL start a pinned Client from the build the Owner runs
- **THEN** no "could not start" notification SHALL be sent

#### Scenario: Binary file removed while the Owner runs

- **WHEN** the file the Owner was started from is deleted, nothing replaces it, and the user
  chooses **Open**
- **THEN** mbv SHALL start a terminal Client that runs the same build as the Owner process
