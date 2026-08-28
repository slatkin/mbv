## Purpose

Narrow the interactive-prompt carve-outs in the toast severity model. The
skip-intro and next-up TUI prompts are removed; mpv's on-screen buttons are
their sole interface (`docs/architecture/mpv-owned-playback-prompts.md`). The
clear-queue confirmation remains, as a modal.

## MODIFIED Requirements

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

### Requirement: Severity-colored toast row

The toast-row background SHALL reflect severity: Success green, Warning yellow, Error red. Neutral toasts SHALL use the standard status-bar styling. The status bar SHALL NOT carry an interactive prompt: a message occupying it is always a toast with a severity-derived duration.

#### Scenario: Background matches severity
- **WHEN** a toast is rendered
- **THEN** its background is the palette color for its severity class, or the standard status-bar styling for Neutral toasts

#### Scenario: Status bar never awaits an answer
- **WHEN** any message occupies the status bar
- **THEN** it expires on its severity-derived duration and claims no key
