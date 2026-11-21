# Spec Delta

## MODIFIED Requirements


### Requirement: Pin flag
mbv SHALL accept `--pin`, which runs the TUI in a pinwin panel named `mbv`: pinwin docks the pin
terminal as the panel, and that terminal runs mbv with the hidden `--in-panel` flag. The process
that ran `--pin` SHALL wait until the panel's terminal ends. Without `--pin`, mbv SHALL start in the current terminal exactly as
before; no config setting starts the panel. `--pin` SHALL combine with `--log-level`, and the mbv
in the panel SHALL run with that log level. `-h`, `-V`, `-q` and `--toggle`
SHALL never start a panel. The panel SHALL close when mbv in it exits. At most one panel named
`mbv` SHALL run per Wayland display. When it already runs, `--pin` SHALL show it, leave a shown
panel unchanged, open no second panel, start no TUI, and exit with status 0.

#### Scenario: Pinned launch from a terminal
- **WHEN** the user types `mbv --pin` in a terminal on a Wayland session with layer-shell
- **THEN** the TUI appears in the panel, and the terminal waits until the panel ends

#### Scenario: Plain launch
- **WHEN** the user types `mbv` in a terminal, including over ssh or inside tmux
- **THEN** mbv starts in that terminal as before, whatever `WAYLAND_DISPLAY` is

#### Scenario: Second pinned launch while hidden
- **WHEN** a pinned mbv is running with its panel hidden and the user starts `mbv --pin` again
- **THEN** the running panel is shown, and the second mbv exits with status 0 without opening a panel

#### Scenario: Second pinned launch while shown
- **WHEN** a pinned mbv is running with its panel shown and the user starts `mbv --pin` again
- **THEN** the running panel is unchanged, and the second mbv exits with status 0

