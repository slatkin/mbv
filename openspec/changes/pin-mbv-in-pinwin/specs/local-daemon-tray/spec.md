# Spec Delta

## MODIFIED Requirements

### Requirement: The tray belongs to a stay-alive local daemon
The tray SHALL be owned by the local daemon and started through the daemon runtime's tray-ready
hook. The daemon SHALL start it as it comes up when stay-alive is enabled at that moment, and
otherwise the first time a pinned Client declares itself. In both cases the tray icon must be
enabled in configuration and a desktop session must be available. The tray SHALL be started at
most once per daemon. Once started, it SHALL exist for the whole life of the daemon, whether or not
any client is attached. No client SHALL start a tray. The tray menu SHALL offer the same items
whether it was started because of stay-alive or because of a pinned Client, except for the
pin-specific item defined below.

#### Scenario: Daemon starts
- **WHEN** a local daemon starts with stay-alive enabled, the tray icon enabled, and a desktop session available
- **THEN** the daemon SHALL start the tray as part of its own startup

#### Scenario: All clients exit
- **WHEN** every client exits while a stay-alive local daemon keeps playing
- **THEN** the tray SHALL remain present and usable

#### Scenario: A client is running
- **WHEN** a client is attached to a local daemon
- **THEN** the client SHALL NOT start a tray of its own

#### Scenario: Stay-alive disabled
- **WHEN** a local daemon starts while stay-alive is disabled and no pinned Client has declared itself
- **THEN** no tray SHALL be started

#### Scenario: Pinned without stay-alive
- **WHEN** stay-alive is disabled, the tray icon is enabled, and a pinned Client declares itself to the local daemon
- **THEN** the daemon SHALL start the tray, and it SHALL go away when that daemon exits with its Client

#### Scenario: Tray icon disabled while pinned
- **WHEN** the tray icon is disabled in configuration and a pinned Client declares itself
- **THEN** no tray SHALL be started

## ADDED Requirements

### Requirement: Pin options from the tray
While at least one attached Client has declared itself pinned, the tray menu SHALL show a
`Pin options...` item. Selecting it SHALL send the `options` request to the pinwin socket of the
most recently declared pinned Client that is still attached, which opens that panel's options
window. When no attached Client is pinned, the item SHALL NOT be shown. A failure to reach the
socket SHALL be recorded in the daemon's log and SHALL NOT affect playback or the tray.

#### Scenario: Open panel options
- **WHEN** mbv runs pinned and the user selects `Pin options...` from the mbv tray
- **THEN** the pinwin panel running that Client opens its options window

#### Scenario: Pinned Client leaves
- **WHEN** the only pinned Client detaches while a stay-alive daemon keeps running
- **THEN** the tray no longer shows `Pin options...`

#### Scenario: Panel unreachable
- **WHEN** the user selects `Pin options...` and the pinwin socket refuses the connection
- **THEN** the failure is logged and playback and the tray continue unaffected
