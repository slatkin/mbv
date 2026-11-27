## MODIFIED Requirements

### Requirement: The sidebar lists the sections in a fixed order

The Settings sidebar SHALL list these sections in this order: Services, Playback, Display, Panel, Session, Library, Queue, Mpv, Feeds, Actions, Keys, Sessions, and Playlists. A section whose every entry the GUI hides SHALL NOT be listed. Selecting a sidebar entry SHALL show that section's content.

#### Scenario: Selecting a section

- **GIVEN** the Settings screen shows Services
- **WHEN** the user selects Keys in the sidebar
- **THEN** the content shows the Keys section

#### Scenario: The Panel section follows Display

- **WHEN** the user opens the Settings screen
- **THEN** the sidebar lists Panel directly after Display

### Requirement: Settings with no effect on the GUI are hidden

The Settings screen SHALL NOT show a setting that has no effect on the GUI Client: the pinned-panel Width and Expanded width, the image protocol, and mouse support. It SHALL NOT show the Log out entry. The hidden settings SHALL keep their stored values, which the TUI Client still edits. The Panel section SHALL show Side, Reserve space, the four gutters, Accent, Accent color, and Accent width, which apply to the pinned GUI window by `gui-pin-window`. A change to one of them while the window is not pinned SHALL be saved and used at the next pin.

#### Scenario: Display hides the terminal settings

- **WHEN** the user opens the Display section
- **THEN** it shows the system notifications setting and no image protocol or mouse support setting

#### Scenario: No pinned-panel settings

- **WHEN** the user looks through every section
- **THEN** no section shows a pinned-panel Width or Expanded width setting

#### Scenario: The Panel section shows the pinned window settings

- **WHEN** the user opens the Panel section
- **THEN** it shows Side, Reserve space, the four gutters, Accent, Accent color, and Accent width

#### Scenario: A Panel change while not pinned

- **GIVEN** the GUI window is not pinned
- **WHEN** the user changes Side to left
- **THEN** the value is saved, the window does not change, and the next pin docks at the left edge
