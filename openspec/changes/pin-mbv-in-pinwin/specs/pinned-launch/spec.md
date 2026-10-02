# Spec Delta

## Purpose

Defines mbv's pin-as-panel feature: a setting that, when enabled, docks mbv's TUI in a GTK
layer-shell panel on the next launch, with its layout editable in the settings screen. The panel is
part of mbv, not a separate program.

## ADDED Requirements

### Requirement: Pin app as UI panel setting
mbv SHALL provide a setting `pin_as_panel` (default off) in the `[display]` section of
`config.toml` and as a toggle in the F2 settings screen. Changing it SHALL be saved immediately and
SHALL take effect the next time mbv launches; it SHALL NOT open or close a panel in a running mbv.
The setting SHALL be offered in F2 only when pinning is built in and the session is Wayland
(`WAYLAND_DISPLAY` set).

#### Scenario: Enable from F2
- **WHEN** the user turns "Pin app as UI panel" on in F2
- **THEN** `pin_as_panel = true` is saved, the current session is unchanged, and the next launch of mbv opens docked in the panel

#### Scenario: Disable from inside the panel
- **WHEN** mbv is running in the panel and the user turns the setting off in F2
- **THEN** the panel stays until mbv exits, and the next launch opens in the terminal as before

#### Scenario: Not offered
- **WHEN** pinning is not built in or `WAYLAND_DISPLAY` is unset
- **THEN** F2 shows no pinning rows, and `pin_as_panel` in `config.toml` is preserved but has no effect

### Requirement: Launch in the panel when enabled
When `pin_as_panel` is true, pinning is built in, `WAYLAND_DISPLAY` is set and the panel starts
successfully, an interactive mbv launch SHALL run its TUI inside the panel in the same process, with
no other program started by the user and no re-launch of mbv. The panel SHALL close when mbv exits.
Otherwise the TUI SHALL start in the current terminal exactly as without the feature. A panel
start-up failure SHALL be recorded in mbv's log, SHALL NOT be fatal and SHALL NOT print a warning on
the terminal. The non-interactive flags (`-h`, `-V`, `-q`, `--__local-daemon`, `--connect-daemon`)
SHALL NOT start a panel.

#### Scenario: Pinned launch
- **WHEN** `pin_as_panel` is true on a Wayland session with layer-shell and the user starts mbv
- **THEN** mbv's TUI appears docked in the panel, and quitting mbv removes the panel

#### Scenario: Panel cannot start
- **WHEN** `pin_as_panel` is true but the compositor lacks layer-shell
- **THEN** mbv starts in the current terminal, and the reason is only in the log

#### Scenario: Setting off
- **WHEN** `pin_as_panel` is false
- **THEN** mbv starts in the current terminal as before

### Requirement: Panel layout settings
The `[panel]` section SHALL hold `side` (`"left"` or `"right"`, default `"left"`), `cols` (integer 1
through 65535, default 40) and `gutter_top`, `gutter_bottom`, `gutter_left`, `gutter_right`
(integers in pixels, may be negative, default 0). The F2 settings screen SHALL provide a Panel page
with one row per value. Layout semantics (docking, reservation, negative gutters, validation) are
those of the `pinwin-panel` capability. A layout the validator rejects SHALL be shown as an inline
error in F2 and SHALL NOT be saved or applied; an invalid value in `config.toml` SHALL fall back to
its default with a logged warning.

#### Scenario: Live change while pinned
- **WHEN** mbv runs in the panel and the user changes the width in F2's Panel page
- **THEN** the panel resizes without restarting mbv, and the value is saved

#### Scenario: Change while not pinned
- **WHEN** mbv runs in a terminal and the user changes a Panel row
- **THEN** the value is saved and used the next time the panel opens

#### Scenario: Rejected layout
- **WHEN** the user enters values whose reservation would leave no output width
- **THEN** F2 shows an inline error, and neither the panel nor `config.toml` changes

### Requirement: GTK stays out of the daemon crates
GTK, gtk4-layer-shell and the pinwin library SHALL be linked only into the `mbv` TUI binary and only
when the `pinning` cargo feature is enabled. `mbv-core`, `mbv-daemon` and `mbvd` SHALL NOT depend on
them, directly or transitively. mbv built without the feature SHALL NOT require Zig or any GUI
toolkit to build or run.

#### Scenario: Daemon build
- **WHEN** `mbvd` or the `.deb` is built
- **THEN** no GTK, gtk4-layer-shell or Zig dependency is involved

#### Scenario: Feature off
- **WHEN** mbv is built without the `pinning` feature
- **THEN** it builds and runs without Zig or GTK, and shows no pinning settings
