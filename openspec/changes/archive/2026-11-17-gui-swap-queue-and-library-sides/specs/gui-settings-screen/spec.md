# Spec Delta

## MODIFIED Requirements

### Requirement: The Settings screen replaces the two panels while it is open

While the Settings screen is open, the GUI Client SHALL show the Settings sidebar at the left window edge and the selected section's content to the right of the sidebar. The player panel, the queue, and the Library panel SHALL NOT show. Playback SHALL continue, and closing the screen SHALL show the two panels as they were before it opened.

#### Scenario: Opening hides the player and the Library

- **GIVEN** the window shows the Queue panel beside the Library panel and an item plays
- **WHEN** the user opens the Settings screen
- **THEN** the window shows the Settings sidebar and the section content, and no player panel, queue, or Library panel
- **AND** the item keeps playing

#### Scenario: The sidebar stays at the left window edge

- **GIVEN** the window shows the Queue panel at the right window edge
- **WHEN** the user opens the Settings screen
- **THEN** the Settings sidebar starts at the left window edge
- **AND** the section content is to the right of the sidebar

#### Scenario: Closing restores the panels

- **GIVEN** the Library showed a destination with a picked item and the Queue panel had Panel focus before the Settings screen opened
- **WHEN** the user closes the Settings screen
- **THEN** the Library shows the same destination and picked item, and the Queue panel has Panel focus
