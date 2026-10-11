# Spec Delta

## MODIFIED Requirements

### Requirement: Each Owner action has exactly one CLI flag

Each Owner action SHALL have exactly one `mbv` CLI flag, and `mbv --help` SHALL list every Owner
action flag. Starting `mbv` with an Owner action flag SHALL run that action only: no TUI SHALL
start. Other arguments on the same command line SHALL be ignored. `--swap-panel` SHALL run the Pin
swap. `--open` SHALL run Open.

#### Scenario: Help lists the swap flag

- **WHEN** the user runs `mbv --help`
- **THEN** the output SHALL list `--swap-panel`

#### Scenario: Help lists the open flag

- **WHEN** the user runs `mbv --help`
- **THEN** the output SHALL list `--open`

#### Scenario: An Owner action flag starts no TUI

- **WHEN** an Owner process runs and the user runs `mbv --swap-panel` in a terminal
- **THEN** no TUI SHALL start in that terminal

### Requirement: The CLI asks the running Owner process and reports acceptance

`mbv` with an Owner action flag SHALL send the action to this machine's Owner process and exit 0
once the Owner accepts it. It SHALL exit 1 with a message on stderr when no Owner process runs,
when the Owner refuses the action, or when the Owner does not support Owner actions or does not
support that action. It SHALL never start an Owner process and SHALL NOT wait for the action's
later outcome.

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

#### Scenario: Owner does not support this action

- **WHEN** the running Owner process supports Owner actions but predates `--open`, and the user
  runs `mbv --open`
- **THEN** stderr SHALL tell the user to restart mbv, and mbv SHALL exit 1
