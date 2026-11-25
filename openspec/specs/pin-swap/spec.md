# pin-swap Specification

## Purpose
Lets the user move their mbv session between the pinned panel and a normal terminal from the Tray
or `mbv-tui --swap-panel`, by replacing one Client with a new Client in the other mode while the Owner
process keeps playing.

## Requirements

### Requirement: The Tray offers one Pin swap action

The Tray SHALL show exactly one Pin swap item: **Unpin** while a pinned Client is attached to the
Owner process, otherwise **Pin**. The label SHALL reflect the attached Clients each time the menu
opens. Only Clients attached over the local transport SHALL count.

#### Scenario: Pinned Client attached

- **WHEN** the user opens the Tray menu while a pinned Client is attached
- **THEN** the menu SHALL show **Unpin** and no **Pin** item

#### Scenario: No pinned Client attached

- **WHEN** the user opens the Tray menu while no pinned Client is attached
- **THEN** the menu SHALL show **Pin** and no **Unpin** item

### Requirement: `mbv-tui --swap-panel` runs the Pin swap

`mbv-tui --swap-panel` SHALL run the Pin swap that the Tray item would run at that moment: **Unpin**
while a pinned Client is attached, otherwise **Pin**, with the same target Client and the same
behavior. It SHALL work whether or not the Tray is shown. When the Owner process refuses to start
the swap, the CLI SHALL report the reason and exit 1.

#### Scenario: Pin from a shell

- **WHEN** one unpinned Client is attached and the user runs `mbv-tui --swap-panel` in another terminal
- **THEN** a pinned Client SHALL start and the unpinned Client SHALL exit after it attaches
- **THEN** the CLI SHALL exit 0

#### Scenario: Swap already running

- **WHEN** a Pin swap is in progress and the user runs `mbv-tui --swap-panel`
- **THEN** no second process SHALL start
- **THEN** the CLI SHALL say a panel swap is already running and exit 1

#### Scenario: Unpin with no terminal command

- **WHEN** a pinned Client is attached, `[panel] terminal` and `TERMINAL` are both unset, and the
  user runs `mbv-tui --swap-panel`
- **THEN** the pinned Client SHALL stay attached and the desktop notification SHALL be sent
- **THEN** the CLI SHALL name `[panel] terminal` and `TERMINAL` on stderr and exit 1

### Requirement: Pin replaces the newest unpinned Client

Choosing **Pin** SHALL start `mbv-tui --pin` as a new Client and then end the most recently attached
unpinned Client that supports Pin swap. With no such Client attached, **Pin** SHALL only start
`mbv-tui --pin`. A Client that does not support Pin swap SHALL never be ended by choosing **Pin**.

#### Scenario: Two terminal Clients attached

- **WHEN** two unpinned Clients are attached and the user chooses **Pin**
- **THEN** a pinned Client SHALL start
- **THEN** the Client that attached last SHALL exit and the other SHALL stay attached

#### Scenario: No Client attached

- **WHEN** Stay-alive keeps the Owner process running with no Client attached and the user chooses
  **Pin**
- **THEN** a pinned Client SHALL start and nothing else SHALL change

### Requirement: Unpin replaces the pinned Client with a terminal Client

Choosing **Unpin** SHALL start a terminal running `mbv-tui` as a new Client and then end the pinned
Client. The terminal command SHALL be the `[panel] terminal` config value when set, otherwise the
`TERMINAL` environment variable followed by `-e`. mbv SHALL NOT fall back to any other terminal.

#### Scenario: Config value set

- **WHEN** `[panel] terminal = ["wezterm", "start", "--"]` and the user chooses **Unpin**
- **THEN** mbv SHALL run `wezterm start -- mbv`

#### Scenario: Only TERMINAL set

- **WHEN** `[panel] terminal` is unset, `TERMINAL=ghostty`, and the user chooses **Unpin**
- **THEN** mbv SHALL run `ghostty -e mbv`

#### Scenario: Neither set

- **WHEN** `[panel] terminal` is unset, `TERMINAL` is unset or empty, and the user chooses
  **Unpin**
- **THEN** mbv SHALL send a desktop notification naming `[panel] terminal` and `TERMINAL`
- **THEN** the pinned Client SHALL stay attached and no process SHALL start

### Requirement: The old Client leaves only after the new Client attaches

During a Pin swap, the old Client SHALL save its TUI launch state before the new Client starts,
and SHALL exit only after the new Client has attached. Playback, the queue, and the playback
position SHALL be unaffected by the swap.

#### Scenario: Successful swap while playing

- **WHEN** media is playing and the user completes a Pin swap
- **THEN** playback SHALL continue without stopping
- **THEN** the new Client SHALL open on the tab, pill, item and Panel focus the old Client showed

### Requirement: A failed Pin swap leaves the old Client in place

If the new Client cannot be started, or does not attach within 10 seconds, the Owner process SHALL
abandon the swap, leave the old Client attached and running, and send a desktop notification
naming the reason. Only one Pin swap SHALL run at a time; a second request from the Tray or the
CLI while one runs SHALL start nothing.

#### Scenario: Terminal command does not exist

- **WHEN** `TERMINAL=nonexistent` and the user chooses **Unpin**
- **THEN** the pinned Client SHALL stay attached
- **THEN** a desktop notification SHALL name the failed command

#### Scenario: New Client never attaches

- **WHEN** the started process does not attach within 10 seconds
- **THEN** the old Client SHALL stay attached and a desktop notification SHALL say the swap timed
  out

#### Scenario: Attached pinned Client does not support Pin swap

- **WHEN** the attached pinned Client does not advertise `pin-swap` and the user chooses **Unpin**
- **THEN** no process SHALL start
- **THEN** the swap SHALL time out and the pinned Client SHALL stay attached

#### Scenario: Second click during a swap

- **WHEN** the user chooses the Tray item again while a Pin swap is in progress
- **THEN** no second process SHALL start

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

### Requirement: Open starts a GUI Client

Choosing **Open** SHALL start the `mbv` program, the GUI Client, as a new Client window. Open SHALL
use no terminal: `[panel] terminal` and `TERMINAL` SHALL have no effect on it. Open SHALL end no
Client. The Owner process SHALL run Open also when a Client is attached. The GUI Client SHALL
present the launch token in its Hello, so its attach completes the Open launch, and it SHALL still
take no part in Pin swap.

#### Scenario: Open with no Client attached

- **WHEN** no Client is attached and the user chooses **Open**
- **THEN** mbv SHALL start the `mbv` program with no terminal
- **THEN** the new GUI Client SHALL attach, playback SHALL be unaffected, and no timeout
  notification SHALL be sent

#### Scenario: Open while a Client is attached

- **WHEN** one unpinned Client is attached and the user runs `mbv-tui --open`
- **THEN** a GUI Client window SHALL start
- **THEN** the first Client SHALL stay attached

#### Scenario: The opened GUI Client is never a swap target

- **WHEN** Open started a GUI Client, no other Client is attached, and the user chooses **Pin**
- **THEN** mbv SHALL start `mbv-tui --pin` and SHALL end no Client

### Requirement: Open shares the Pin swap launch rules

Open and the Pin swap SHALL share one launch slot: only one of them SHALL run at a time. If the
new Client cannot be started, exits with a failure before it attaches, or does not attach within
10 seconds, the Owner process SHALL send a desktop notification that names the reason.

#### Scenario: Open while a launch runs

- **WHEN** a Pin swap or an Open is in progress and the user chooses **Open**
- **THEN** no second process SHALL start

#### Scenario: Open never attaches

- **WHEN** the started GUI Client does not attach within 10 seconds
- **THEN** a desktop notification SHALL say that opening mbv timed out

### Requirement: Launches start the Owner's own build after its binary file changes

The Pin swap and Unpin SHALL start the `mbv-tui` program, and Open SHALL start the `mbv` program, of the build that the Owner process itself runs, even after the installed Client program file or the installed `mbv-owner` file is replaced or removed. The Owner process SHALL keep hold of the `mbv-tui` and `mbv` program files that sat beside `mbv-owner` when the Owner started, and SHALL launch Clients from those held files. A Client attaches only to an Owner of the same build, so a launch SHALL NOT start a different build.

#### Scenario: Binary file replaced while the Owner runs

- **WHEN** the Owner was started from `/usr/bin/mbv-owner`, `/usr/bin/mbv-tui` is replaced by a newer build, and the user chooses **Pin**
- **THEN** mbv SHALL start a pinned Client from the build the Owner runs
- **THEN** no "could not start" notification SHALL be sent

#### Scenario: Binary file removed while the Owner runs

- **WHEN** `/usr/bin/mbv` is deleted after the Owner started, nothing replaces it, and the user chooses **Open**
- **THEN** mbv SHALL start a GUI Client that runs the same build as the Owner process

#### Scenario: No Client program beside the Owner program
- **WHEN** no `mbv-tui` program sat beside `mbv-owner` when the Owner started, and the user chooses **Pin** or **Unpin**, or no `mbv` program sat there and the user chooses **Open**
- **THEN** the Owner SHALL start no Client, SHALL leave any attached Client in place, and SHALL send a desktop notification that names the missing program
