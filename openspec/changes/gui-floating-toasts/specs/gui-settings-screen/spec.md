# Spec Delta

## MODIFIED Requirements

### Requirement: The Settings screen shows the toasts

While the Settings screen is open, it SHALL show the prefix-armed state of the shared Status bar projection, because neither the footer line nor the Library Status bar shows. The Settings screen's status line SHALL show no toast: toasts show as floating cards over the screen (`gui-toasts`).

#### Scenario: A refusal notice shows on the screen

- **GIVEN** the Settings screen is open
- **WHEN** a setting change is refused with a notice
- **THEN** the notice shows as a floating card over the Settings screen
- **AND** the Settings screen's status line does not show it
