# Spec Delta

## ADDED Requirements

### Requirement: The Tray icon toggles the pinned panel

A left click on the Tray icon SHALL ask the Owner process to toggle the pinned panel: hide it
when it is shown, and show it when it is hidden. The request SHALL be the same one `mbv --toggle`
sends. The Owner SHALL run it only while a pinned Client is attached; with no pinned Client the
click SHALL do nothing. The Tray SHALL start no process to do this, and playback SHALL continue
across the hide and the show.

#### Scenario: Hide the panel from the Tray

- **WHEN** a pinned Client is attached, the panel is shown, and the user left-clicks the Tray icon
- **THEN** the panel hides, the TUI in it keeps running, and playback continues

#### Scenario: Show the panel from the Tray

- **WHEN** a pinned Client is attached, the panel is hidden, and the user left-clicks the Tray icon
- **THEN** the panel shows at the width it had

#### Scenario: No pinned panel

- **WHEN** no pinned Client is attached and the user left-clicks the Tray icon
- **THEN** nothing happens and no panel opens

#### Scenario: The request fails

- **WHEN** the panel request fails
- **THEN** a desktop notification names the reason
