# Spec Delta

## Purpose

Defines how mbv starts from its desktop launcher: pinned to a screen edge in pinwin when pinwin is
installed, otherwise in the user's terminal. Also covers how a pinned Client makes itself known to
its Owner process.

## ADDED Requirements

### Requirement: One desktop launcher
mbv SHALL install exactly one desktop entry. It SHALL start `mbv --desktop` without asking the
desktop to provide a terminal (`Terminal=false`).

#### Scenario: Single entry
- **WHEN** the `mbv` package is installed, with or without `pinwin`
- **THEN** the application launcher lists exactly one mbv entry

### Requirement: Desktop launch chooses pinwin or a terminal
`mbv --desktop` SHALL replace itself with `pinwin --no-tray mbv` when an executable `pinwin` is on
`PATH`. Otherwise it SHALL replace itself with `xdg-terminal-exec mbv` when an executable
`xdg-terminal-exec` is on `PATH`. When neither is available, it SHALL record the reason in mbv's
log and exit with a non-zero status. `mbv --desktop` SHALL NOT load configuration, take the
single-instance lock or start an Owner process itself; the `mbv` it launches does that as usual.
Running `mbv` without `--desktop` SHALL behave exactly as before.

#### Scenario: pinwin installed
- **WHEN** the user starts mbv from the launcher and `pinwin` is on `PATH`
- **THEN** mbv opens pinned in a pinwin panel with no pinwin tray entry

#### Scenario: pinwin not installed
- **WHEN** the user starts mbv from the launcher, `pinwin` is not on `PATH` and `xdg-terminal-exec` is
- **THEN** mbv opens in the user's default terminal

#### Scenario: Nothing to run in
- **WHEN** neither `pinwin` nor `xdg-terminal-exec` is on `PATH`
- **THEN** `mbv --desktop` logs why and exits non-zero without starting an Owner process

#### Scenario: Terminal launch unchanged
- **WHEN** the user runs `mbv` in a terminal
- **THEN** mbv starts in that terminal as before, whether or not pinwin is installed

### Requirement: A pinned Client declares itself to its Owner
A Client whose environment has a non-empty absolute path in `PINWIN_SOCKET` is pinned. After
attaching to its local Owner process, a pinned Client SHALL declare itself pinned with that path,
but only when the Owner advertises the pinned-panel capability. A Client attached to `mbvd` or any
non-local endpoint SHALL NOT declare itself. A Client without `PINWIN_SOCKET`, or with an empty or
relative value, SHALL NOT declare itself.

#### Scenario: Pinned Client attaches
- **WHEN** mbv runs inside pinwin and attaches to its local Owner process
- **THEN** the Owner knows that Client is pinned and knows its pinwin socket path

#### Scenario: Remote daemon
- **WHEN** mbv runs inside pinwin connected to `mbvd` with `--connect-daemon`
- **THEN** no pinned declaration is sent

#### Scenario: Older Owner
- **WHEN** a pinned Client attaches to an Owner that does not advertise the pinned-panel capability
- **THEN** the Client sends no declaration and works as an ordinary Client

### Requirement: pinwin ships as its own package
pinwin SHALL be built from this repository's `pinwin/` directory and shipped as a separate `pinwin`
package that owns its GUI dependencies (GTK4, gtk4-layer-shell, libdbusmenu-glib). The `mbv`
package SHALL list `pinwin` and `xdg-terminal-exec` only as optional dependencies, and SHALL NOT
depend on any GUI toolkit. Building mbv's Rust workspace SHALL NOT require the Zig toolchain.

#### Scenario: mbv without pinwin
- **WHEN** a user installs only the `mbv` package
- **THEN** GTK4, gtk4-layer-shell and libdbusmenu-glib are not pulled in, and `mbv` and `mbvd` work as before

#### Scenario: pinwin package
- **WHEN** a user installs the `pinwin` package
- **THEN** `/usr/bin/pinwin` exists, and the mbv launcher opens mbv pinned
