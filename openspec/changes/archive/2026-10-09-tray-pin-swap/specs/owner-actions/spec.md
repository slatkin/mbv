# Spec Delta

## Purpose

Lets the user run an action of the running Owner process from a CLI flag, through the same path
the Tray uses, so every Tray action can in time be run from a shell or a compositor key.

## ADDED Requirements

### Requirement: Each Owner action has exactly one CLI flag

Each Owner action SHALL have exactly one `mbv` CLI flag, and `mbv --help` SHALL list every Owner
action flag. Starting `mbv` with an Owner action flag SHALL run that action only: no TUI SHALL
start. Other arguments on the same command line SHALL be ignored. `--swap-panel` SHALL run the Pin
swap.

#### Scenario: Help lists the swap flag

- **WHEN** the user runs `mbv --help`
- **THEN** the output SHALL list `--swap-panel`

#### Scenario: An Owner action flag starts no TUI

- **WHEN** an Owner process runs and the user runs `mbv --swap-panel` in a terminal
- **THEN** no TUI SHALL start in that terminal

### Requirement: The CLI asks the running Owner process and reports acceptance

`mbv` with an Owner action flag SHALL send the action to this machine's Owner process and exit 0
once the Owner accepts it. It SHALL exit 1 with a message on stderr when no Owner process runs,
when the Owner refuses the action, or when the Owner does not support Owner actions. It SHALL
never start an Owner process and SHALL NOT wait for the action's later outcome.

#### Scenario: No Owner process

- **WHEN** no Owner process runs and the user runs `mbv --swap-panel`
- **THEN** stderr SHALL say that no mbv is running
- **THEN** mbv SHALL exit 1 and no Owner process SHALL start

#### Scenario: Owner refuses

- **WHEN** the Owner process refuses the action
- **THEN** stderr SHALL show the Owner's reason and mbv SHALL exit 1

#### Scenario: Owner too old

- **WHEN** the running Owner process does not support Owner actions
- **THEN** stderr SHALL tell the user to restart mbv, mbv SHALL exit 1, and the Owner SHALL receive
  no action

### Requirement: An Owner action request is not a Client

The connection that carries an Owner action SHALL NOT count as a Client. It SHALL be admitted while
Stay-alive is off and a Client is attached. It SHALL never be the target of a Pin swap, SHALL NOT
keep the Owner process running, and SHALL carry no command other than the Owner action. It SHALL
be accepted only from this machine and never over TCP.

#### Scenario: Stay-alive off with a Client attached

- **WHEN** `stay_alive` is false, one TUI is attached, and the user runs `mbv --swap-panel`
- **THEN** the Owner process SHALL accept the action
- **THEN** the attached TUI SHALL NOT be refused or disconnected by the request itself

#### Scenario: Over TCP

- **WHEN** a peer connects over TCP and asks to run an Owner action
- **THEN** the Owner process SHALL refuse the connection and run nothing

### Requirement: The Tray and the CLI run the same Owner action

A Tray item that has an Owner action SHALL run it through the same Owner process path as that
action's CLI flag, with the same behavior. The Tray SHALL NOT start an `mbv` process to do so.

#### Scenario: Pin from either trigger

- **WHEN** the same Clients are attached and the user chooses **Pin** in the Tray, or runs
  `mbv --swap-panel`
- **THEN** the Owner process SHALL pick the same direction and target Client in both cases
