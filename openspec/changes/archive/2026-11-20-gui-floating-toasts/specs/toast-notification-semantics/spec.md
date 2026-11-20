# Spec Delta

## ADDED Requirements

### Requirement: Toasts accumulate

Several toasts SHALL be live at the same time. A new toast SHALL NOT end the live toasts, and each toast SHALL expire on its own severity-derived duration. At most 5 toasts SHALL be live: when a sixth arrives, the oldest live toast SHALL end. The TUI SHALL present only the newest live toast that renders in-app.

#### Scenario: A second toast does not end the first

- **GIVEN** a Warning toast is live
- **WHEN** a Success toast arrives 1 second later
- **THEN** both toasts are live
- **AND** the Success toast expires 2 seconds after it arrived and the Warning toast 5 seconds after it arrived

#### Scenario: A sixth toast arrives

- **GIVEN** 5 toasts are live
- **WHEN** a sixth toast arrives
- **THEN** the oldest of the 5 ends and 5 toasts stay live, the new one the newest

#### Scenario: The TUI shows the newest toast

- **GIVEN** two toasts are live and both render in-app
- **WHEN** the TUI status row paints
- **THEN** it shows only the newer toast

### Requirement: A repeated toast replaces its live copy

When a toast arrives with the same text and severity as a live toast, the live toast SHALL end and the new toast SHALL become the newest live toast with a full duration.

#### Scenario: The same notice arrives twice

- **GIVEN** the toast "Emby is unavailable" is live, with another toast live after it
- **WHEN** the toast "Emby is unavailable" arrives again with the same severity
- **THEN** one "Emby is unavailable" toast is live, it is the newest toast, and its duration starts again

## MODIFIED Requirements

### Requirement: Silent neutral toasts, no bell

Toasts SHALL NOT ring the terminal bell. Neutral toasts SHALL NOT emit a desktop notification and SHALL always render in-app. Success, Warning, and Error toasts SHALL attempt a desktop notification when system notifications are enabled; when the desktop notification succeeds, that toast does not render in-app. Whether system notifications are enabled SHALL be decided by the system notifications setting alone, the same at startup and after a mid-session toggle, whatever stay-alive is set to.

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

#### Scenario: A delivered toast stays out of the window while a Neutral toast shows
- **GIVEN** system notifications are enabled and the desktop notification succeeds
- **WHEN** a Neutral toast and then an Error toast arrive
- **THEN** the Neutral toast renders in-app and the Error toast does not
