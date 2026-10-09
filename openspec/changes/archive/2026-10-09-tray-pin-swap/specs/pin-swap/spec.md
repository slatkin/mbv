# Spec Delta

## Purpose

Lets the user move their mbv session between the pinned panel and a normal terminal from the Tray
or `mbv --swap-panel`, by replacing one Client with a new Client in the other mode while the Owner
process keeps playing.

## ADDED Requirements

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

### Requirement: `mbv --swap-panel` runs the Pin swap

`mbv --swap-panel` SHALL run the Pin swap that the Tray item would run at that moment: **Unpin**
while a pinned Client is attached, otherwise **Pin**, with the same target Client and the same
behavior. It SHALL work whether or not the Tray is shown. When the Owner process refuses to start
the swap, the CLI SHALL report the reason and exit 1.

#### Scenario: Pin from a shell

- **WHEN** one unpinned Client is attached and the user runs `mbv --swap-panel` in another terminal
- **THEN** a pinned Client SHALL start and the unpinned Client SHALL exit after it attaches
- **THEN** the CLI SHALL exit 0

#### Scenario: Swap already running

- **WHEN** a Pin swap is in progress and the user runs `mbv --swap-panel`
- **THEN** no second process SHALL start
- **THEN** the CLI SHALL say a panel swap is already running and exit 1

#### Scenario: Unpin with no terminal command

- **WHEN** a pinned Client is attached, `[panel] terminal` and `TERMINAL` are both unset, and the
  user runs `mbv --swap-panel`
- **THEN** the pinned Client SHALL stay attached and the desktop notification SHALL be sent
- **THEN** the CLI SHALL name `[panel] terminal` and `TERMINAL` on stderr and exit 1

### Requirement: Pin replaces the newest unpinned Client

Choosing **Pin** SHALL start `mbv --pin` as a new Client and then end the most recently attached
unpinned Client that supports Pin swap. With no such Client attached, **Pin** SHALL only start
`mbv --pin`. A Client that does not support Pin swap SHALL never be ended by choosing **Pin**.

#### Scenario: Two terminal Clients attached

- **WHEN** two unpinned Clients are attached and the user chooses **Pin**
- **THEN** a pinned Client SHALL start
- **THEN** the Client that attached last SHALL exit and the other SHALL stay attached

#### Scenario: No Client attached

- **WHEN** Stay-alive keeps the Owner process running with no Client attached and the user chooses
  **Pin**
- **THEN** a pinned Client SHALL start and nothing else SHALL change

### Requirement: Unpin replaces the pinned Client with a terminal Client

Choosing **Unpin** SHALL start a terminal running `mbv` as a new Client and then end the pinned
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
