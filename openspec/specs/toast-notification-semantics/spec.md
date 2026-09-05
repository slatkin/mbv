# toast-notification-semantics Specification

## Purpose
Defines the semantic severity model for TUI toast notifications: severity classes, display duration, desktop-notification behavior, and toast-row colors.
## Requirements
### Requirement: Four-class severity model

The system SHALL classify every TUI toast as exactly one of: **Neutral** (progress, information, lifecycle events), **Success** (a user-requested action completed), **Warning** (an operation failed but recovered through a working fallback), or **Error** (an operation failed without recovery). The clear-queue confirmation is an interactive prompt, not a toast, and SHALL remain unclassified. The TUI SHALL NOT present a skip-intro or next-up prompt; those decisions are offered by the mpv on-screen buttons only.

#### Scenario: Progress is neutral
- **WHEN** a loading, scanning, connecting, or requested-action message is shown
- **THEN** it is classified Neutral

#### Scenario: Completed action is success
- **WHEN** a user-requested action completes (save, clear, connect, rename, enqueue)
- **THEN** the toast is classified Success

#### Scenario: Recovered failure is warning
- **WHEN** an operation fails but a working fallback engages automatically (e.g., falling back to local playback)
- **THEN** the toast is classified Warning

#### Scenario: Unrecovered failure is error
- **WHEN** an operation fails and nothing happens (load/connect/save failure, empty selection, refusal)
- **THEN** the toast is classified Error

#### Scenario: Intro boundary reached without auto-skip
- **WHEN** an intro boundary is reported and the client is not configured to always skip
- **THEN** the TUI shows no prompt and claims no key
- **THEN** the mpv on-screen Skip Intro button is the only offered affordance

#### Scenario: Next episode is offered
- **WHEN** the player reports the next queued episode
- **THEN** the TUI shows no prompt and claims no key
- **THEN** the mpv on-screen next-up card is the only offered affordance

### Requirement: Duration derives from severity

Toast display duration SHALL be determined by severity class: Neutral and Success toasts display for 2 seconds; Warning and Error toasts display for 5 seconds.

#### Scenario: Duration follows class
- **WHEN** a Success toast and an Error toast with equal-length messages are shown
- **THEN** the Success toast displays for 2 seconds and the Error toast for 5 seconds

### Requirement: Silent neutral toasts, no bell

Toasts SHALL NOT ring the terminal bell. Neutral toasts SHALL NOT emit a desktop notification and SHALL always render in-app. Success, Warning, and Error toasts SHALL attempt a desktop notification when system notifications are enabled; when the desktop notification succeeds, the in-app toast row is hidden.

#### Scenario: No bell
- **WHEN** any toast is displayed
- **THEN** the terminal bell does not ring

#### Scenario: Neutral is silent and in-app
- **WHEN** a Neutral toast is displayed with system notifications enabled
- **THEN** no desktop notification is emitted and the toast renders in-app

#### Scenario: Colored toast notifies
- **WHEN** a Success, Warning, or Error toast is displayed with system notifications enabled
- **THEN** a desktop notification is attempted

### Requirement: Severity-colored toast row

The toast-row background SHALL reflect severity: Success green, Warning yellow, Error red. Neutral toasts SHALL use the standard status-bar styling. The status bar SHALL NOT carry an interactive prompt: a message occupying it is always a toast with a severity-derived duration.

#### Scenario: Background matches severity
- **WHEN** a toast is rendered
- **THEN** its background is the palette color for its severity class, or the standard status-bar styling for Neutral toasts

#### Scenario: Status bar never awaits an answer
- **WHEN** any message occupies the status bar
- **THEN** it expires on its severity-derived duration and claims no key

### Requirement: Truthful remote-submission copy

A toast shown when submitting playback to a remote session SHALL describe the request (e.g., "Requesting playback: …") and SHALL NOT claim playback has already started.

#### Scenario: Submission says requested
- **WHEN** the user plays an item on an attached remote session
- **THEN** the toast says playback was requested, not that it is playing

