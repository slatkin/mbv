# Spec Delta

## Purpose

Lets the user move their mbv session between the pinned panel and a normal terminal from the Tray,
by replacing one Client with a new Client in the other mode while the Owner process keeps playing.

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

### Requirement: Pin replaces the newest unpinned Client

Choosing **Pin** SHALL start `mbv --pin` as a new Client and then end the most recently attached
unpinned Client that supports Pin swap. With no such Client attached, **Pin** SHALL only start
`mbv --pin`. A Client that does not support Pin swap SHALL never be ended by a Pin swap.

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
naming the reason. Only one Pin swap SHALL run at a time; choosing the Tray item while one runs
SHALL do nothing.

#### Scenario: Terminal command does not exist

- **WHEN** `TERMINAL=nonexistent` and the user chooses **Unpin**
- **THEN** the pinned Client SHALL stay attached
- **THEN** a desktop notification SHALL name the failed command

#### Scenario: New Client never attaches

- **WHEN** the started process does not attach within 10 seconds
- **THEN** the old Client SHALL stay attached and a desktop notification SHALL say the swap timed
  out

#### Scenario: Second click during a swap

- **WHEN** the user chooses the Tray item again while a Pin swap is in progress
- **THEN** no second process SHALL start
