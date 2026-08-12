## Purpose

Defines how the left panel identifies and dispatches Home, Emby, Audiobookshelf, and Feeds behavior without allowing one Service's browse actions to operate on another Service's state.

## ADDED Requirements

### Requirement: Left-panel destinations dispatch exhaustively by browse target
Every selected left-panel destination SHALL resolve to exactly one of Home, an Emby library, an Audiobookshelf library, or Feeds before destination-specific behavior executes. A destination-specific action SHALL NOT be selected by excluding the other destination kinds.

#### Scenario: Keyboard input reaches its selected destination
- **WHEN** the library panel has focus and the user invokes a destination-specific keyboard action
- **THEN** mbv SHALL dispatch the action only to the selected Home, Emby, Audiobookshelf, or Feeds handler
- **THEN** no other destination's cursor, selection, or browse state SHALL change

#### Scenario: Mouse input reaches its selected destination
- **WHEN** the user scrolls, clicks, double-clicks, or right-clicks a destination-specific left-panel surface
- **THEN** mbv SHALL interpret the gesture using only that destination's hit targets and actions
- **THEN** no numeric row or library index SHALL be interpreted as belonging to a different destination kind

#### Scenario: Destination has no applicable action
- **WHEN** the user invokes an action that the selected destination does not support
- **THEN** mbv SHALL leave destination, queue, playback, and Service state unchanged
- **THEN** the action SHALL NOT fall through to an Emby operation

### Requirement: Destination-specific handlers operate on explicit destination identity
An Emby browse operation SHALL receive an explicitly selected Emby library identity, and an Audiobookshelf browse operation SHALL receive an explicitly selected Audiobookshelf library identity. A missing or mismatched identity SHALL NOT default to another library.

#### Scenario: Emby action executes
- **WHEN** an Emby library action is dispatched
- **THEN** mbv SHALL apply it to the explicitly selected Emby library
- **THEN** the action SHALL NOT derive an Emby library from an Audiobookshelf or Feeds position

#### Scenario: Destination identity is absent or stale
- **WHEN** a selected destination no longer resolves to current browse state
- **THEN** mbv SHALL normalize the selection or perform no destination-specific action
- **THEN** mbv SHALL NOT substitute Emby library zero or any other destination

### Requirement: Mixed tab positions preserve destination identity
Tab navigation and restoration SHALL map each visible Home, Emby library, Audiobookshelf library, and Feeds destination to a unique position in the displayed order.

#### Scenario: User navigates a mixed tab strip
- **WHEN** Home, one or more Emby libraries, one or more Audiobookshelf libraries, and Feeds are visible
- **THEN** keyboard and mouse navigation SHALL select the same destination at each displayed position
- **THEN** no Emby and Audiobookshelf destinations SHALL share a position

#### Scenario: Last selected destination is restored
- **WHEN** mbv restores a saved tab position against the current destination counts
- **THEN** it SHALL restore the destination represented by that current ordered position or use the defined safe fallback
- **THEN** it SHALL NOT reinterpret an Audiobookshelf position as an Emby library position

### Requirement: Refresh targets the selected browse destination
Refreshing the library panel SHALL invoke the refresh behavior belonging to the selected destination. Refreshing the queue panel SHALL remain a queue refresh and SHALL NOT depend on the selected browse destination.

#### Scenario: User refreshes a browse destination
- **WHEN** the library panel is focused on Home, an Emby library, an Audiobookshelf library, or Feeds and the user invokes refresh
- **THEN** mbv SHALL refresh only the selected destination through its provider-specific refresh behavior

#### Scenario: User refreshes the queue panel
- **WHEN** the queue panel is focused and the user invokes refresh
- **THEN** mbv SHALL refresh the visible queue without refreshing or indexing the selected browse destination

### Requirement: Help and context actions reflect the selected destination
Destination-specific help and context actions SHALL describe and expose only behavior valid for the selected Home, Emby, Audiobookshelf, or Feeds destination.

#### Scenario: User opens help from a destination
- **WHEN** the library panel is focused and the user opens help
- **THEN** mbv SHALL prioritize the selected destination's valid shortcuts
- **THEN** it SHALL NOT present Emby-only shortcuts as Audiobookshelf or Feeds behavior

#### Scenario: User requests a context menu
- **WHEN** the selected destination or row kind has no supported context actions
- **THEN** mbv SHALL not open an Emby context menu for that destination or row
- **THEN** selection and browse state SHALL remain usable

### Requirement: Browse models remain Service-specific until explicit playback submission
Emby, Audiobookshelf, and Feed catalog identities and browse state SHALL remain concrete and separate. Shared playback state SHALL be entered only by an explicit supported play or enqueue action that constructs the corresponding provider-native queue item.

#### Scenario: User navigates provider-specific content
- **WHEN** the user navigates, filters, refreshes, selects, or opens help for an Emby, Audiobookshelf, or Feeds destination
- **THEN** mbv SHALL retain that Service's native browse identity and state
- **THEN** it SHALL NOT convert the selection into another Service's browse model or a queue item

#### Scenario: Audiobookshelf episode activation precedes playback support
- **WHEN** the user activates or requests enqueue for an Audiobookshelf episode before its playback capability is applied
- **THEN** mbv SHALL retain the existing read-only behavior without entering Emby, Feed, queue, or playback handling
