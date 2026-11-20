# Spec Delta

## MODIFIED Requirements

### Requirement: The sidebar lists the sections in a fixed order

The Settings sidebar SHALL list these sections in this order: Services, Playback, Display, Session, Library, Queue, Mpv, Feeds, Actions, Keys, and Sessions. A section whose every entry the GUI hides SHALL NOT be listed. Selecting a sidebar entry SHALL show that section's content.

#### Scenario: Selecting a section

- **GIVEN** the Settings screen shows Services
- **WHEN** the user selects Keys in the sidebar
- **THEN** the content shows the Keys section

### Requirement: The open actions open the screen at their section

The `settings_open` action SHALL open the Settings screen at Services and the `sessions_open` action at Sessions, through the user's configured chords. While the screen is open, an open action SHALL close it when the screen shows that action's sections, and SHALL show its section otherwise. The F2 sections are every section except Sessions. The `playlists_open` action SHALL NOT open the Settings screen.

#### Scenario: F3 opens Sessions

- **GIVEN** default bindings and the Settings screen is closed
- **WHEN** the user presses F3
- **THEN** the Settings screen opens at Sessions

#### Scenario: The same action closes the screen

- **GIVEN** the Settings screen shows the Keys section
- **WHEN** the user runs `settings_open`
- **THEN** the Settings screen closes

#### Scenario: Another action switches the section

- **GIVEN** the Settings screen shows Services
- **WHEN** the user runs `sessions_open`
- **THEN** the screen stays open and shows Sessions

#### Scenario: A rebound action opens the screen

- **GIVEN** `settings_open` is bound to another chord in `[keys]`
- **WHEN** the user presses that chord
- **THEN** the Settings screen opens at Services

### Requirement: Every Settings action is reachable from the keyboard

Every action on the Settings screen SHALL have a key path: moving through the sidebar and the content, opening a section, changing a setting, submitting and cancelling a form, starting and cancelling a capture, and every Sessions action. No action SHALL be available only by pointer.

#### Scenario: Toggling by key

- **GIVEN** the content of the Queue section holds the keyboard
- **WHEN** the user moves to a toggle and presses its activation key
- **THEN** the toggle changes

## REMOVED Requirements

### Requirement: Playlists offers the TUI's playlist actions

**Reason**: Playlists is no longer a Settings section. The playlist actions live in the GUI Playlists view in the Queue column.

**Migration**: See the `gui-client` requirement "The Playlists view offers the TUI's playlist actions". Add to queue now closes the view, as the `gui-client` requirement "Play, Shuffle, and Add to queue close the Playlists view" states.
