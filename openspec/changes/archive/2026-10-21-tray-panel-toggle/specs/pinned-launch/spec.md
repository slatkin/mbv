# Spec Delta

## MODIFIED Requirements

### Requirement: Panel toggle
mbv SHALL accept `--toggle`, which asks pinwin on the current Wayland display to hide the panel
named `mbv` if shown, or to show it if hidden, and exits with status 0. It SHALL NOT start mbv, a
TUI or a panel. When no such panel runs or the request fails, it SHALL report pinwin's reason
(stderr on a terminal, otherwise a desktop notification) and exit with status 1. Hiding and
showing SHALL NOT resize the TUI or change its width state. The Tray icon SHALL ask for the same
toggle while a pinned Client is attached; the Owner process sends the request then, and a failed
request is reported as a desktop notification.

#### Scenario: Hide from a compositor key
- **WHEN** the user presses a compositor key bound to `mbv --toggle` while the pinned panel is shown
- **THEN** the panel hides, playback continues, and the `mbv --toggle` process exits with status 0

#### Scenario: Show from a compositor key
- **WHEN** the user presses the same key while the panel is hidden
- **THEN** the panel shows at the same width with the TUI as it was, and takes keyboard focus

#### Scenario: Nothing pinned
- **WHEN** the user runs `mbv --toggle` in a terminal with no `mbv` panel running
- **THEN** one line gives pinwin's reason, and the process exits with status 1

#### Scenario: Nothing pinned from a key
- **WHEN** a compositor key runs `mbv --toggle` with no `mbv` panel running
- **THEN** a desktop notification gives pinwin's reason, and no mbv starts

#### Scenario: Toggle from the Tray
- **WHEN** a pinned Client is attached and the user left-clicks the Tray icon
- **THEN** the Owner process sends the same request, and the panel hides or shows as it did for
  `mbv --toggle`
