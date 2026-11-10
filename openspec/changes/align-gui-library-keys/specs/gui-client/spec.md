# Spec Delta: gui-client

## MODIFIED Requirements

### Requirement: GUI keyboard interaction matches the TUI Client

For every action the GUI Client presents, each key chord SHALL have the same effect as in the TUI Client. The GUI Client SHALL use the same `[keys]` configuration and the same keyboard router. A GUI key that names a registry action SHALL read the configured chord for that action, not a literal. A key the GUI Client takes for an action the TUI Client binds to another chord in the same context is an authorised exception, and the change that introduces it records the authorisation.

#### Scenario: A configured binding is used in both Clients

- **GIVEN** the user rebinds a queue action in `[keys]`
- **WHEN** the user presses the new chord in the GUI Client
- **THEN** the GUI Client runs that queue action, as the TUI Client does

#### Scenario: Prefix mode

- **WHEN** the user arms prefix mode in the GUI Client
- **THEN** the footer line shows the armed state and the next chord resolves in the prefix namespace, as in the TUI Client

#### Scenario: A registry chord is rebound

- **GIVEN** the user rebinds an action whose chord the Library panel's window content must match
- **WHEN** the window content next paints
- **THEN** the front end resolves the action's new chord, and the old chord no longer takes the action

## ADDED Requirements

### Requirement: The Library window keys match the TUI's keys

While the GUI Library panel shows, the window keys SHALL be the TUI's: `Tab` and `BackTab` cycle the tab selection, the digits resolve tab positions through the shared tab-jump action, `c` opens the clear-queue prompt, `q` quits, `z` cycles subtitles, and `Esc` with no panel-owned field, menu, or overlay open SHALL reach the Client core for the leaf back and the deferred stop. The front end SHALL hold no second copy of a registry action's chord.

#### Scenario: Tab cycles the tab selection

- **WHEN** the user presses `Tab` while the Library panel shows
- **THEN** the next destination tab becomes active, as in the TUI Client, and no pane focus moves

#### Scenario: A digit resolves a tab position

- **WHEN** the user presses `2`
- **THEN** the destination at tab position 2 becomes active if the Client presents it, and nothing opens when it does not

#### Scenario: q quits and z cycles subtitles

- **WHEN** the user presses `q` or `z` while the Library panel shows
- **THEN** the shared actions `quit` and `cycle_subtitle` run, and no GUI-only literal answers first

### Requirement: The Library leaf chords match the TUI's leaves

Where the TUI's Library leaf binds a chord, the GUI Library panel SHALL take the same chord for the same action: `Ctrl+A` enqueues where the TUI leaf binds enqueue, `r` refreshes the destination, `[` and `]` cycle the Selector row, and `.` opens the item context menu for the cursor row. The front end SHALL send the command the TUI leaf's chord produces through the shared `App` paths.

#### Scenario: Ctrl+A enqueues the cursor item

- **GIVEN** a destination whose TUI leaf binds `Ctrl+A` to enqueue
- **WHEN** the user presses `Ctrl+A`
- **THEN** the item under the cursor is added to the queue, and bare `a` binds nothing

#### Scenario: r refreshes

- **WHEN** the user presses `r` while a destination shows
- **THEN** the destination refreshes through the same refresh path the TUI leaf's refresh request runs

#### Scenario: Selector row cycles

- **WHEN** the user presses `[` or `]` while a Selector row shows
- **THEN** the row cycles to the previous or next letter range, group, or section, as in the TUI Client

#### Scenario: The context menu opens from the keyboard

- **WHEN** the user presses `.` while a row holds the cursor
- **THEN** the item menu opens for that row, as a right-click does

#### Scenario: Esc with nothing panel-owned open

- **GIVEN** no panel-owned field, menu, or overlay is open
- **WHEN** the user presses `Esc`
- **THEN** the key reaches the Client core and the leaf back or the deferred stop runs, as in the TUI Client

#### Scenario: q quits and z cycles subtitles

- **WHEN** the user presses `q` or `z` while the Library panel shows
- **THEN** the shared actions `quit` and `cycle_subtitle` run, and no GUI-only literal answers first

### Requirement: The GUI search keys match the TUI's search keys

The Search field SHALL arm with the configured `search_open` chord, and `Ctrl+K` SHALL NOT be a GUI-fixed alias. While the field is focused, `Tab` and `BackTab` SHALL cycle the shared search type filter, and `Ctrl+P`, `Ctrl+S`, and `Ctrl+A` on the highlighted result SHALL play, shuffle, and add, as the TUI's search result chords do. The field's visible hint SHALL show the configured `search_open` chord read from the keybinds, and a rebind SHALL move the hint with it.

#### Scenario: The configured chord opens Search

- **GIVEN** the user rebinds `search_open` in `[keys]`
- **WHEN** the user presses the configured chord while the Library panel shows
- **THEN** the Search field takes focus, and `Ctrl+K` does not

#### Scenario: The hint follows the keybinds

- **GIVEN** the user rebinds `search_open`
- **WHEN** the field next paints
- **THEN** the hint names the configured chord, not a literal

#### Scenario: The search result chords act

- **WHEN** the user presses `Ctrl+P`, `Ctrl+S`, or `Ctrl+A` while a result is highlighted
- **THEN** that result plays, shuffles, or adds to the queue, as in the TUI Client

#### Scenario: Tab cycles the type filter

- **WHEN** the user presses `Tab` or `BackTab` while the Search field is focused
- **THEN** the type filter cycles and the results reflect it, as the TUI's Search sidebar does

### Requirement: The confirm overlay shares the TUI's confirm chords

The GUI Client's confirm modal SHALL answer the same chord set the TUI's Confirm component answers: `Enter`, `Esc`, and, for the three-way dirty-playlist prompt, `d` with or without a shift modifier. One shared chord set and one shared button-hint parser SHALL serve the TUI component, the GUI overlay's key branch, and the GUI's button press.

#### Scenario: Shift+D discards the dirty playlist

- **GIVEN** the dirty-playlist prompt is open in the GUI Client
- **WHEN** the user presses `Shift+D`
- **THEN** the prompt answers Discard, as in the TUI Client

#### Scenario: A button hint names two chords

- **GIVEN** a confirm button's hint names two chords
- **WHEN** the GUI resolves a press on that button
- **THEN** the shared parser answers with the hint's first chord
