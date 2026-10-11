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
