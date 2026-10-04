## MODIFIED Requirements

### Requirement: Stay Alive is the sole configuration policy for TUI-exit lifetime

The `stay_alive` configuration setting SHALL be the only setting determining whether this
machine's local daemon automatically outlives a TUI attached to it. No command-line flag
SHALL enable Stay Alive. Explicit lifecycle controls such as `mbv -q`, tray Quit, and
operating-system termination remain independent of this setting. A TUI launched without an
explicit daemon endpoint SHALL always reach playback through this machine's local daemon;
`stay_alive` SHALL decide only that daemon's lifetime, how many TUIs it admits, and whether its
tray is forced on, never whether it exists. No other Client or Player-owner behaviour SHALL
depend on `stay_alive`: in particular, playback route switching and system notifications SHALL
behave the same whatever it is set to.

#### Scenario: Stay Alive enabled

- **WHEN** `stay_alive` is true and the user starts mbv with no local daemon running
- **THEN** a local daemon SHALL own playback and the TUI SHALL attach to it
- **THEN** quitting the TUI SHALL leave the daemon and playback running

#### Scenario: Stay Alive disabled with no daemon running

- **WHEN** `stay_alive` is false and no local daemon is running
- **THEN** mbv SHALL start a local daemon that owns playback and the TUI SHALL attach to it
- **THEN** the TUI process SHALL NOT own a Player
- **THEN** quitting the TUI SHALL stop playback and leave no mbv-owned process running

#### Scenario: Legacy daemon flag is rejected

- **WHEN** the user invokes `mbv -d`
- **THEN** mbv SHALL exit with guidance to enable `stay_alive` in configuration or the
  settings overlay
- **THEN** mbv SHALL NOT silently treat `-d` as an ordinary foreground invocation

#### Scenario: Switching away from the home link keeps local playback

- **WHEN** local playback is active on this machine's local daemon, with `stay_alive` either
  true or false
- **WHEN** the user switches the playback route to a direct remote, a Library route, or another
  daemon endpoint
- **THEN** the TUI SHALL NOT stop the local daemon's playback
- **THEN** returning to the local route SHALL show the local queue still playing, or finished
  if it ran out
