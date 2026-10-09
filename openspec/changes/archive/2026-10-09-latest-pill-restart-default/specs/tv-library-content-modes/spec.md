# Spec Delta

## MODIFIED Requirements

### Requirement: The content mode is part of the sticky library position

The selected TV content mode SHALL be saved with the library's navigation position and restored when the library is reopened within the same session. The mode SHALL NOT persist across a restart: after a restart the library resolves its count-dependent default mode (`Latest` above the pill threshold, `All` at or below it) before the row is painted or any fetch is issued, regardless of the mode selected in the previous session. A saved mode that the reopened library's current show count no longer offers SHALL be replaced by the count's default mode before the row is painted or any fetch is issued. Keyboard cycling SHALL move the selection across every mode in row order and SHALL wrap from the last mode to the first and from the first to the last. Mouse selection SHALL select the clicked mode.

#### Scenario: The mode is restored on reopen
- **WHEN** the user selects a TV content mode, leaves the library's tab, and later reopens that library's saved position without restarting
- **THEN** the same mode is selected and its content is loaded

#### Scenario: Restart resolves the count-dependent default
- **WHEN** mbv restarts after a session in which a TV library's `Upcoming`, `All`, or alphabet-range mode was selected
- **THEN** the library resolves its count-dependent default mode (`Latest` above the pill threshold, `All` at or below it) instead of the previously selected mode

#### Scenario: A saved mode the current count no longer offers is re-clamped
- **WHEN** the user reopens a TV library whose saved position selected a mode the library's current show count no longer offers, because the count crossed the pill threshold between runs
- **THEN** the count's default mode (`Latest` above the threshold, `All` at or below it) is selected instead, before the row is painted or any fetch is issued
- **AND** no pill is highlighted over content it does not select

#### Scenario: Cycling wraps across modes
- **WHEN** the user cycles forward from the last mode, or backward from the first
- **THEN** the selection wraps to the opposite end
- **AND** every mode in the row participates in the cycle
