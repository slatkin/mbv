## RENAMED Requirements

- FROM: `### Requirement: The tray belongs to a stay-alive local daemon`
- TO: `### Requirement: The tray belongs to the local Owner process`

## MODIFIED Requirements

### Requirement: The tray belongs to the local Owner process

The tray SHALL be owned by the local daemon, which SHALL start it through the daemon runtime's
tray hook. The tray SHALL be shown exactly while it is enabled: when stay-alive is enabled, or when
the "Show systray icon" setting is on. Stay-alive SHALL force the tray on without changing the
stored "Show systray icon" preference. When either setting changes while the daemon runs, the
daemon SHALL start or remove the tray to match, without a restart. No client SHALL start a tray.
While enabled, the tray SHALL remain present whether or not any client is attached.

#### Scenario: Daemon starts

- **WHEN** a local daemon starts with stay-alive enabled and a desktop session available
- **THEN** the daemon SHALL start the tray as part of its own startup, whatever "Show systray
  icon" is set to

#### Scenario: Daemon starts with stay-alive disabled and the tray icon enabled

- **WHEN** a local daemon starts with stay-alive disabled, "Show systray icon" on, and a desktop
  session available
- **THEN** the daemon SHALL start the tray

#### Scenario: Stay-alive disabled

- **WHEN** a local daemon starts with stay-alive disabled and "Show systray icon" off
- **THEN** no tray SHALL be started

#### Scenario: Stay-alive turned on during the session

- **WHEN** no tray is shown and the user turns stay-alive on
- **THEN** the daemon SHALL start the tray without restarting

#### Scenario: Stay-alive turned off during the session

- **WHEN** the tray is shown, "Show systray icon" is off, and the user turns stay-alive off
- **THEN** the daemon SHALL remove the tray without restarting
- **THEN** the stored "Show systray icon" preference SHALL still be off

#### Scenario: All clients exit

- **WHEN** every client exits while the local daemon keeps playing with the tray shown
- **THEN** the tray SHALL remain present and usable

#### Scenario: A client is running

- **WHEN** a client is attached to a local daemon
- **THEN** the client SHALL NOT start a tray of its own

### Requirement: A missing tray is not an error

When no tray can be shown — no desktop session, no status-icon host, or the tray not enabled —
the local daemon SHALL continue running normally without it. mbv SHALL NOT warn the user on the
terminal, on either the daemon or the client side.

#### Scenario: Headless host

- **WHEN** a local daemon starts over SSH or on a bare terminal with no desktop session
- **THEN** the daemon SHALL run and play normally with no tray
- **THEN** no warning about the missing tray SHALL be printed to any terminal
- **THEN** the condition SHALL be recorded in the daemon's log only

#### Scenario: Tray icon disabled in configuration

- **WHEN** stay-alive is disabled and "Show systray icon" is off
- **THEN** the local daemon SHALL NOT attempt to start a tray and SHALL run normally

#### Scenario: Stopping a daemon with no tray

- **WHEN** the user needs to stop a local daemon that has no tray and no attached client
- **THEN** `mbv -q` SHALL stop it

## ADDED Requirements

### Requirement: The tray setting shows whether the tray is enabled

The settings overlay's "Show systray icon" row SHALL display whether the tray is enabled, not only
the stored preference. While stay-alive is enabled, the row SHALL read as on, and toggling it SHALL
change nothing and SHALL show a toast saying the tray stays on while stay-alive is on. While
stay-alive is disabled, the row SHALL show and toggle the stored preference. The setting SHALL
default to off.

#### Scenario: Row while stay-alive is on

- **WHEN** stay-alive is enabled and the stored "Show systray icon" preference is off
- **THEN** the row SHALL read as on

#### Scenario: Toggle refused while stay-alive is on

- **WHEN** stay-alive is enabled and the user toggles "Show systray icon"
- **THEN** the stored preference SHALL NOT change
- **THEN** a toast SHALL say the tray stays on while stay-alive is on

#### Scenario: Toggle while stay-alive is off

- **WHEN** stay-alive is disabled and the user toggles "Show systray icon" on
- **THEN** the stored preference SHALL become on and the daemon SHALL start the tray

#### Scenario: Fresh configuration

- **WHEN** the configuration does not set "Show systray icon"
- **THEN** the preference SHALL be off
