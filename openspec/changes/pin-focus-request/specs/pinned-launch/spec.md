# Spec Delta

## MODIFIED Requirements

### Requirement: Pin flag
mbv SHALL accept `--pin`. It runs the TUI in the pinned panel in the same process. The user starts
no other program, and mbv does not re-launch itself. Without `--pin`, mbv SHALL start in the
current terminal exactly as before. No config setting starts the panel. `--pin` SHALL combine with
`--log-level`.
`-h`, `-V`, `-q`, `--focus` and `--__local-daemon` SHALL behave as specified and never start a
panel. When mbv exits, the panel SHALL close.
At most one pinned launch SHALL run per user at any time. mbv SHALL enforce this with an exclusive
lock. The pinned launch takes the lock before it opens the panel and holds it until the process
exits. The lock SHALL be released on any process exit, including a crash, so a dead pinned launch
never blocks the next one. A `--pin` launch that cannot take the lock SHALL NOT open a panel. It
SHALL make the focus request in "Pinned panel focus request" instead. If the panel of a pinned
launch fails to start and mbv falls back to the terminal, mbv SHALL release the lock before it runs
in the terminal. This rule does not limit plain terminal launches.

#### Scenario: Pinned launch from the desktop
- **WHEN** the user starts mbv from its desktop entry on a Wayland session with layer-shell
- **THEN** mbv's TUI appears docked in the panel, and quitting mbv removes the panel

#### Scenario: Pinned launch from a terminal
- **WHEN** the user types `mbv --pin` in a terminal on a Wayland session with layer-shell
- **THEN** the TUI appears in the panel, and the terminal waits until mbv exits

#### Scenario: Plain launch
- **WHEN** the user types `mbv` in a terminal, including over ssh or inside tmux
- **THEN** mbv starts in that terminal as before, whatever `WAYLAND_DISPLAY` is

#### Scenario: Second pinned launch
- **WHEN** a pinned launch is running and the user starts the desktop entry or `mbv --pin` again
- **THEN** no second panel opens, the running panel takes keyboard focus, and the new process exits
  with status 0

#### Scenario: Pinned launch after a crash
- **WHEN** a pinned launch was killed and the user starts `mbv --pin`
- **THEN** the panel opens as a normal first pinned launch

#### Scenario: Plain launch beside a pinned launch
- **WHEN** a pinned launch is running and the user types `mbv` in a terminal
- **THEN** mbv starts in that terminal as before, and the pinned launch is unchanged

### Requirement: Panel start failure
When `--pin` is given and the panel cannot start (`WAYLAND_DISPLAY` unset, no layer-shell, GTK
init failure, a saved layout that does not fit the output, the pin lock file failing to open or
lock for a reason other than another pinned launch holding it, or the terminal hand-over failing),
mbv SHALL record the reason in its log. If stdin
is a terminal, mbv SHALL print one line naming the reason to stderr and run in that terminal. If
stdin is not a terminal, mbv SHALL send a desktop notification naming the reason and exit with
status 1. A fatal start-up error after the panel has opened SHALL be logged and sent as a desktop
notification.

#### Scenario: Terminal fallback
- **WHEN** the user types `mbv --pin` in a terminal on a compositor without layer-shell
- **THEN** one warning line is printed and mbv runs in that terminal

#### Scenario: Saved layout does not fit
- **WHEN** `mbv --pin` starts with a `[panel]` layout that leaves the output no width
- **THEN** the panel does not open, and mbv takes the same terminal or notification path

#### Scenario: Desktop launch without layer-shell
- **WHEN** the desktop entry is launched on GNOME or X11
- **THEN** a notification says the panel could not open, and mbv exits with status 1

## ADDED Requirements

### Requirement: Pinned panel focus request
A running pinned launch SHALL accept a focus request from other local processes of the same user.
On a request it SHALL ask the panel for keyboard focus. The panel then takes focus in the way the
pinwin library defines for its `on-demand` keyboard mode: the reserved space does not change, so
tiled windows do not move, and the normal `on-demand` rules release focus later. If the panel accepted the
request, the pinned launch SHALL answer success. Otherwise it SHALL answer failure. After either
answer, the pinned launch SHALL keep running.
mbv SHALL accept `--focus`, which sends one focus request to the running pinned launch and exits.
On success, it SHALL exit with status 0. A failure includes no running pinned launch and no
answer within a bounded time. On a failure, it SHALL record the reason in its log and exit with
status 1. If stdin is a terminal, it SHALL print one line naming the reason to stderr. Otherwise
it SHALL send a desktop notification naming the reason. `--focus` SHALL NOT start a panel, a TUI, or a Player
owner. A second `--pin` launch (see "Pin flag") SHALL behave exactly as `--focus`.
`mbv --help` and the README SHALL describe `--focus` and how to bind it to a compositor hotkey.

#### Scenario: Hotkey focuses the panel
- **WHEN** a pinned launch is running without keyboard focus and the user presses a compositor
  hotkey bound to `mbv --focus`
- **THEN** the panel takes keyboard focus, the tiled windows keep their position and size, and
  `mbv --focus` exits with status 0

#### Scenario: No pinned launch
- **WHEN** no pinned launch is running and the user runs `mbv --focus` in a terminal
- **THEN** one line says that no pinned mbv is running, and the command exits with status 1

#### Scenario: No pinned launch from a hotkey
- **WHEN** no pinned launch is running and a compositor hotkey runs `mbv --focus`
- **THEN** a desktop notification says that no pinned mbv is running, and the command exits with
  status 1

#### Scenario: Panel refuses the request
- **WHEN** the pinned launch answers that the panel did not accept the request
- **THEN** `mbv --focus` reports the failure and exits with status 1, and the pinned launch keeps
  running
