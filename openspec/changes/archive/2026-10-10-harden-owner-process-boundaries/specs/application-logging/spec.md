# Spec Delta

## ADDED Requirements

### Requirement: The mbv log lives in the process's state directory
`mbv` SHALL write `mbv.log` and its crash line in the same state directory that holds the rest
of that process's state, resolved by the one shared state-directory rule. That rule includes the
system-instance directory and a fallback that is never relative to the working directory. `mbv`
SHALL create that directory if it is missing before it writes the crash line.

#### Scenario: System instance
- **WHEN** `mbv` runs with `MBV_SYSTEM=1`
- **THEN** `mbv.log` and the crash line SHALL be written under the system-instance state
  directory, next to that process's stored Service credentials

#### Scenario: HOME is unset
- **WHEN** `mbv` runs with neither `XDG_STATE_HOME` nor `HOME` set
- **THEN** `mbv.log` SHALL be written under the same absolute fallback state directory as the
  rest of its state, never under the working directory

#### Scenario: Crash before the state directory exists
- **WHEN** a `--pin` launch crashes before anything has created the state directory
- **THEN** the directory SHALL be created and the crash line SHALL be written to `mbv.log`
