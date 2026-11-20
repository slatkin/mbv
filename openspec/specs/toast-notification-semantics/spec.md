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

### Requirement: Status bar messages expire unanswered

The status bar SHALL NOT carry an interactive prompt: a message occupying it is always a toast with a severity-derived duration.

#### Scenario: Status bar never awaits an answer
- **WHEN** any message occupies the status bar
- **THEN** it expires on its severity-derived duration and claims no key

### Requirement: Truthful remote-submission copy

A toast shown when submitting playback to a remote session SHALL describe the request (e.g., "Requesting playback: …") and SHALL NOT claim playback has already started.

#### Scenario: Submission says requested
- **WHEN** the user plays an item on an attached remote session
- **THEN** the toast says playback was requested, not that it is playing

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
