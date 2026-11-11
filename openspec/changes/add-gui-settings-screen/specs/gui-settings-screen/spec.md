# Spec Delta

## Purpose

The GUI Client's one Settings screen: the window arrangement that replaces the player and Library panels while it is open, its sidebar sections, and the settings, Service setup, feed subscriptions, key bindings, playback targets, and playlists it presents.

## ADDED Requirements

### Requirement: The Settings screen replaces the two panels while it is open

While the Settings screen is open, the GUI Client SHALL show the Settings sidebar in the Queue column's place and the selected section's content in the Library column's place. The player panel, the queue, and the Library panel SHALL NOT show. Playback SHALL continue, and closing the screen SHALL show the two panels as they were before it opened.

#### Scenario: Opening hides the player and the Library

- **GIVEN** the window shows the Queue panel beside the Library panel and an item plays
- **WHEN** the user opens the Settings screen
- **THEN** the window shows the Settings sidebar and the section content, and no player panel, queue, or Library panel
- **AND** the item keeps playing

#### Scenario: Closing restores the panels

- **GIVEN** the Library showed a destination with a picked item and the Queue panel had Panel focus before the Settings screen opened
- **WHEN** the user closes the Settings screen
- **THEN** the Library shows the same destination and picked item, and the Queue panel has Panel focus

### Requirement: The Settings screen fills the window

While the Settings screen is open, the sidebar SHALL take its column width and the section content SHALL take the rest of the window's width and height. Outside Mini, no part of the window SHALL stay empty beside the content.

#### Scenario: The content takes the remaining width

- **GIVEN** a window wider than the Mini threshold
- **WHEN** the Settings screen opens
- **THEN** the content spans from the sidebar's edge to the window's right edge

### Requirement: Every Settings request reaches the Client core

Every request that the Settings screen sends SHALL reach the Client core. The window's permissions SHALL grant every command that the GUI Client registers, so no Settings action is refused before the core sees it.

#### Scenario: A sidebar press reaches the core

- **WHEN** the user presses a sidebar entry
- **THEN** the core receives the section request, and the next frame shows that section

### Requirement: Every Settings control works by pointer

A pointer press on any Settings control (sidebar entries, rows, marks, routes, Service and form controls, feed, playlist, Keys, and target controls) SHALL have the same effect as the keyboard path to it, and SHALL move the keyboard position to it. A control that can act SHALL show hover feedback. A control that cannot act SHALL show as unavailable, and a press on it SHALL do nothing.

#### Scenario: A press on a toggle

- **WHEN** the user presses the Consume audio row in the Queue section
- **THEN** the setting changes as Enter on that row changes it, and the content cursor rests on the row

#### Scenario: A press starts a capture

- **WHEN** the user presses an action's chord in Keys
- **THEN** a capture starts for that action, as the keyboard path starts it

#### Scenario: A press on an unavailable control

- **WHEN** the user presses a control that shows as unavailable
- **THEN** nothing is sent and nothing changes

#### Scenario: A press does not act behind the screen

- **GIVEN** the Settings screen is open
- **WHEN** the user presses where a queue row painted before the screen opened
- **THEN** no queue action runs

### Requirement: The Settings screen shows the toasts

While the Settings screen is open, it SHALL show the prefix-armed state and the toasts of the shared Status bar projection, because neither the footer line nor the Library Status bar shows.

#### Scenario: A refusal notice shows on the screen

- **GIVEN** the Settings screen is open
- **WHEN** a setting change is refused with a notice
- **THEN** the notice shows on the Settings screen

### Requirement: The sidebar lists the sections in a fixed order

The Settings sidebar SHALL list these sections in this order: Services, Playback, Display, Session, Library, Queue, Mpv, Feeds, Actions, Keys, Sessions, and Playlists. A section whose every entry the GUI hides SHALL NOT be listed. Selecting a sidebar entry SHALL show that section's content.

#### Scenario: Selecting a section

- **GIVEN** the Settings screen shows Services
- **WHEN** the user selects Keys in the sidebar
- **THEN** the content shows the Keys section

### Requirement: The open actions open the screen at their section

The `settings_open` action SHALL open the Settings screen at Services, the `sessions_open` action at Sessions, and the `playlists_open` action at Playlists, through the user's configured chords. While the screen is open, an open action SHALL close it when the screen shows that action's sections, and SHALL show its section otherwise. The F2 sections are every section except Sessions and Playlists.

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
- **WHEN** the user runs `playlists_open`
- **THEN** the screen stays open and shows Playlists

#### Scenario: A rebound action opens the screen

- **GIVEN** `settings_open` is bound to another chord in `[keys]`
- **WHEN** the user presses that chord
- **THEN** the Settings screen opens at Services

### Requirement: Esc closes the screen from the sidebar

Esc SHALL close the Settings screen while the sidebar holds the keyboard. While the section content holds the keyboard, Esc SHALL first cancel the open edit, capture, or form, and otherwise return the keyboard to the sidebar.

#### Scenario: Esc from the content returns to the sidebar

- **GIVEN** the Settings screen is open, the content holds the keyboard, and no edit is open
- **WHEN** the user presses Esc
- **THEN** the sidebar holds the keyboard and the screen stays open

#### Scenario: Esc from the sidebar closes the screen

- **GIVEN** the sidebar holds the keyboard
- **WHEN** the user presses Esc
- **THEN** the Settings screen closes

### Requirement: The screen opens at Services when nothing is set up

At start, when no Remote Service is configured and Feeds has no subscriptions, the GUI Client SHALL open the Settings screen at Services.

#### Scenario: First start

- **GIVEN** no Remote Service is configured and Feeds has no subscriptions
- **WHEN** the GUI Client attaches to the Owner process
- **THEN** the Settings screen is open at Services

### Requirement: A narrow window shows the sidebar or the content

Below the Mini threshold the GUI Client SHALL show only one of the sidebar and the section content. Opening a section SHALL show its content, and Esc from the content SHALL show the sidebar. The section shown SHALL NOT change when the window crosses the threshold.

#### Scenario: Mini shows the content of the opened section

- **GIVEN** the window is below the Mini threshold and the sidebar shows
- **WHEN** the user opens the Playback section
- **THEN** only the Playback content shows

#### Scenario: A resize keeps the section

- **GIVEN** the window is wide and the screen shows the Queue section
- **WHEN** the window shrinks below the Mini threshold
- **THEN** the screen still has the Queue section selected

### Requirement: Settings with no effect on the GUI are hidden

The Settings screen SHALL NOT show a setting that has no effect on the GUI Client: the pinned-panel page with its geometry, reserve, and accent settings, the image protocol, and mouse support. It SHALL NOT show the Log out entry. The hidden settings SHALL keep their stored values, which the TUI Client still edits.

#### Scenario: Display hides the terminal settings

- **WHEN** the user opens the Display section
- **THEN** it shows the system notifications setting and no image protocol or mouse support setting

#### Scenario: No pinned-panel settings

- **WHEN** the user looks through every section
- **THEN** no section shows a pinned-panel width, gutter, reserve, side, or accent setting

### Requirement: A setting change applies and is saved as in the TUI

Changing a shown setting SHALL have the same effect, the same refusal rules, and the same persistence as the TUI Client's F2 change of that setting. The section SHALL show the new value on the next frame.

#### Scenario: A toggle is saved

- **WHEN** the user turns on the Consume audio setting in the Queue section
- **THEN** the setting shows on and is saved to the configuration as the TUI saves it

#### Scenario: A refused toggle stays unchanged

- **GIVEN** Stay alive is on
- **WHEN** the user turns off the systray icon setting
- **THEN** the setting stays as it was and the GUI shows the same refusal notice as the TUI

### Requirement: Choice-list settings are edited in place

The Hidden libraries, Feed view libraries, and My languages settings SHALL each show their choices as a list the user marks in the section content. Applying the marks SHALL save them with the same effects as the TUI's multiselect commit.

#### Scenario: Hiding a library

- **WHEN** the user marks a library in Hidden libraries and applies
- **THEN** the library is saved as hidden and leaves the Library panel's destinations

### Requirement: Library routes are edited in the Library section

The Library section SHALL list the routable libraries with each one's assigned device and SHALL let the user assign one device or clear the route. A device with no direct-connect endpoint SHALL show as not routable and SHALL NOT be assignable by pointer or key. The route SHALL be saved and take effect as the TUI's route editor saves it.

#### Scenario: Assigning a route

- **WHEN** the user assigns a routable device to a library
- **THEN** the route is saved and playback of that library goes to the device

#### Scenario: Selecting a library lists its devices

- **GIVEN** the Library section shows the routable libraries
- **WHEN** the user selects a library in the routes list
- **THEN** the section lists that library's candidate devices

#### Scenario: A device with no endpoint

- **GIVEN** a library's candidate devices include one with no direct-connect endpoint
- **WHEN** the user presses that device or presses Enter on it
- **THEN** the device shows as not routable, no command is sent, and no route is saved

### Requirement: Services shows each Service and its setup

The Services section SHALL show the Emby, Audiobookshelf, and Feeds Services with each one's Service state and the actions the TUI offers for that state. Emby setup SHALL collect the server URL, username, and password, and Audiobookshelf setup the server URL and API key. Setup SHALL validate as the TUI's does and SHALL show its progress and its error.

#### Scenario: Emby setup succeeds

- **GIVEN** Emby is Not configured
- **WHEN** the user submits a valid server URL, username, and password
- **THEN** the section shows the validation in progress, and then Emby as Ready

#### Scenario: Audiobookshelf setup fails

- **WHEN** the user submits an Audiobookshelf API key that the server rejects
- **THEN** the section shows the setup error and the previous Audiobookshelf setup is unchanged

#### Scenario: Managing feeds from Services

- **WHEN** the user runs the Manage feeds action in the Services section
- **THEN** the screen shows the Feeds section

### Requirement: Secrets never return to the window

A typed password or API key SHALL go to the Client core only when the user submits it. No frame SHALL carry a password, an API key, or a Service credential back to the window.

#### Scenario: A failed setup does not echo the password

- **WHEN** an Emby setup fails
- **THEN** the frame carries the error and the server URL, and no password

### Requirement: Feeds manages the feed subscriptions

The Feeds section SHALL list the feed subscriptions and SHALL let the user add, edit, and remove one. An add SHALL fetch and validate the feed before it saves, and SHALL show that it is in progress. An edit SHALL change only the name and the kind. A remove SHALL ask for confirmation.

#### Scenario: Adding a feed

- **WHEN** the user adds a feed URL that serves valid entries
- **THEN** the section shows the add in progress, and then the new subscription in the list

#### Scenario: Adding an invalid feed

- **WHEN** the user adds a URL that serves no valid entries
- **THEN** no subscription is saved and the GUI shows the TUI's add-failure notice

#### Scenario: Removing a feed asks first

- **WHEN** the user removes a subscription
- **THEN** the confirmation modal asks before the subscription is removed

### Requirement: Keys lists the bindings that act in the GUI

The Keys section SHALL list the configurable actions that have an effect in the GUI Client, grouped by the shared section vocabulary, with each action's effective router chords and its prefix-namespace chord where one is assigned, and SHALL show the configured prefix chord. It SHALL mark each action whose binding differs from its declared default.

#### Scenario: An overridden action is marked

- **GIVEN** `[keys]` binds `toggle_play_pause` to another chord
- **WHEN** the user opens Keys
- **THEN** the action shows that chord and is marked as changed from its default

#### Scenario: A TUI-only action is not listed

- **WHEN** the user opens Keys
- **THEN** no action that has no effect in the GUI Client is listed

### Requirement: Keys captures a new chord

The user SHALL be able to start a capture on an action's router chord, its prefix-namespace chord, or the prefix chord. During a capture the next key press SHALL become the candidate chord, and no other GUI or core key handling SHALL act on it. Esc SHALL cancel the capture. A key that the chord grammar cannot name SHALL be refused.

#### Scenario: Capturing a chord

- **GIVEN** a capture is active on `next_track`
- **WHEN** the user presses Ctrl+Right
- **THEN** Ctrl+Right becomes the candidate chord for `next_track`, and no next-track or other action runs

#### Scenario: Esc cancels a capture

- **GIVEN** a capture is active
- **WHEN** the user presses Esc
- **THEN** the capture ends and the binding is unchanged

### Requirement: Keys refuses a colliding or reserved chord

A candidate chord SHALL be checked against every rule that the configuration load applies. A candidate that another action already fires on, that collides with the prefix chord or a prefix-namespace chord, or that is reserved SHALL be refused, and the refusal SHALL name the rule and any other action involved. A refused candidate SHALL change nothing.

#### Scenario: A collision names the other action

- **GIVEN** `stop` fires on its default chord
- **WHEN** the user captures that chord for `toggle_mute`
- **THEN** the edit is refused, the refusal names `stop`, and both bindings are unchanged

#### Scenario: A reserved chord is refused

- **WHEN** the user captures a reserved chord for an action
- **THEN** the edit is refused as reserved and nothing changes

### Requirement: A valid key edit is saved and acts at once

A valid candidate SHALL be saved to `[keys]` at once and SHALL fire its action in the GUI Client from the next key press. The user SHALL be able to reset an action to its declared default, and to clear a prefix-namespace chord or the prefix chord.

#### Scenario: A rebind fires at once

- **WHEN** the user rebinds `toggle_play_pause` to a free chord
- **THEN** that chord plays and pauses on the next press, and `[keys]` holds it

#### Scenario: Reset to default

- **GIVEN** `seek_forward` has an override
- **WHEN** the user resets `seek_forward`
- **THEN** it fires on its declared default chords and `[keys]` holds no override for it

### Requirement: Sessions lists and selects the playback targets

The Sessions section SHALL list the same playback targets as the header's playback-target chooser and SHALL mark the active one. Selecting a target SHALL select it through the same path as the chooser, and the screen SHALL stay open.

#### Scenario: Selecting a cast receiver

- **WHEN** the user selects a Cast receiver in Sessions
- **THEN** the receiver becomes the playback output, Sessions marks it active, and the screen stays open

### Requirement: Playlists offers the TUI's playlist actions

The Playlists section SHALL list the saved Emby playlists and SHALL let the user open a playlist to its items, play, shuffle, or add a playlist or one item, rename and delete a playlist, and refresh, with the same effects as the TUI's Playlists sidebar. A play that replaces a populated queue SHALL ask as the TUI's does. A delete SHALL ask for confirmation.

#### Scenario: Playing a playlist

- **WHEN** the user plays a playlist
- **THEN** the queue is replaced with the playlist's playable items, after the TUI's populated-queue prompt where it applies

#### Scenario: Renaming a playlist

- **WHEN** the user renames a playlist and confirms the new name
- **THEN** the playlist has the new name on the server and in the list

#### Scenario: Adding one item

- **GIVEN** a playlist is open
- **WHEN** the user adds one of its items
- **THEN** that item is appended to the queue and the screen stays open

### Requirement: Reset UI State resets the GUI's presentation

The Actions section SHALL offer Reset UI State. In the GUI Client it SHALL return the Library to Home with no kept collection or picked item, reset the Queue panel's cursor, marks, scroll, and collapse and fold toggles, reset the panel widths, clear the GUI's saved launch location and widths, and close the Settings screen. It SHALL NOT change media, settings, key bindings, Service setup, queue contents, playback, or the TUI Client's saved state.

#### Scenario: Reset returns to Home

- **GIVEN** the Library showed a Music collection with a picked album and the Queue panel was collapsed
- **WHEN** the user runs Reset UI State
- **THEN** the screen closes, the Library shows Home with no pick, the Queue panel is expanded, and the queue and playback are unchanged

#### Scenario: The next launch starts from defaults

- **GIVEN** the user ran Reset UI State
- **WHEN** the GUI Client starts again before any other change
- **THEN** it starts at Home with default panel widths

### Requirement: Every Settings action is reachable from the keyboard

Every action on the Settings screen SHALL have a key path: moving through the sidebar and the content, opening a section, changing a setting, submitting and cancelling a form, starting and cancelling a capture, and every Playlists and Sessions action. No action SHALL be available only by pointer.

#### Scenario: Toggling by key

- **GIVEN** the content of the Queue section holds the keyboard
- **WHEN** the user moves to a toggle and presses its activation key
- **THEN** the toggle changes

### Requirement: Global keys keep working behind the screen

While the Settings screen is open and no text field or capture holds the keyboard, a key that the screen does not take SHALL reach the Client core's keyboard router, so playback and global actions keep their chords. A key that acts on the queue or the Library panel SHALL NOT act while the screen is open.

#### Scenario: Space pauses behind the screen

- **GIVEN** the Settings screen is open, an item plays, and the sidebar holds the keyboard
- **WHEN** the user presses Space
- **THEN** playback pauses

#### Scenario: A queue key does nothing

- **GIVEN** the Settings screen is open and the queue held Panel focus before it opened
- **WHEN** the user presses the clear-queue chord
- **THEN** no clear-queue prompt opens

### Requirement: The window content owns the screen's interaction state

The Settings screen's cursor, keyboard position, scroll, form drafts, choice marks before apply, and capture state SHALL live in the window content. The Client core SHALL hold whether the screen is open, the selected section, the section's data, and the actions. A window-content reload SHALL keep the screen open at its section and SHALL reset the rest.

#### Scenario: A reload keeps the section

- **GIVEN** the Settings screen shows Feeds with a half-typed URL
- **WHEN** the window content reloads
- **THEN** the screen shows Feeds with an empty form
