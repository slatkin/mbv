## MODIFIED Requirements

### Requirement: Silent neutral toasts, no bell

Toasts SHALL NOT ring the terminal bell. Neutral toasts SHALL NOT emit a desktop notification and SHALL always render in-app. Success, Warning, and Error toasts SHALL attempt a desktop notification when system notifications are enabled; when the desktop notification succeeds, the in-app toast row is hidden. Whether system notifications are enabled SHALL be decided by the system notifications setting alone, the same at startup and after a mid-session toggle, whatever stay-alive is set to.

#### Scenario: No bell
- **WHEN** any toast is displayed
- **THEN** the terminal bell does not ring

#### Scenario: Neutral is silent and in-app
- **WHEN** a Neutral toast is displayed with system notifications enabled
- **THEN** no desktop notification is emitted and the toast renders in-app

#### Scenario: Colored toast notifies
- **WHEN** a Success, Warning, or Error toast is displayed with system notifications enabled
- **THEN** a desktop notification is attempted

#### Scenario: Stay-alive does not suppress notifications
- **WHEN** a TUI starts with stay-alive enabled and system notifications enabled
- **WHEN** a Success, Warning, or Error toast is displayed
- **THEN** a desktop notification is attempted
