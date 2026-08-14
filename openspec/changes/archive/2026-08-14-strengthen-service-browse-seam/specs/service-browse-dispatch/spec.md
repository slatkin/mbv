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
An Emby browse operation SHALL receive an explicitly selected Emby library identity, and an Audiobookshelf browse operation SHALL receive an explicitly selected Audiobookshelf library identity. A missing or mismatched identity SHALL NOT default to another library. Before browse input, refresh, rendering, help, context-menu, or tab-navigation behavior dispatches, a selected Emby or Audiobookshelf library index that no longer exists SHALL normalize to Home. The operation that encountered the stale identity SHALL perform no destination-specific action.

#### Scenario: Emby action executes
- **WHEN** an Emby library action is dispatched
- **THEN** mbv SHALL apply it to the explicitly selected Emby library
- **THEN** the action SHALL NOT derive an Emby library from an Audiobookshelf or Feeds position

#### Scenario: Destination identity is absent or stale
- **WHEN** a selected destination no longer resolves to current browse state
- **THEN** mbv SHALL select Home and perform no destination-specific part of the triggering operation
- **THEN** mbv SHALL NOT substitute Emby library zero or any other destination

### Requirement: Mixed tab positions preserve destination identity
Tab navigation SHALL map each visible Home, Emby library, Audiobookshelf library, and Feeds destination to a unique position in the displayed order.

#### Scenario: User navigates a mixed tab strip
- **WHEN** Home, one or more Emby libraries, one or more Audiobookshelf libraries, and Feeds are visible
- **THEN** keyboard and mouse navigation SHALL select the same destination at each displayed position
- **THEN** no Emby and Audiobookshelf destinations SHALL share a position

### Requirement: Refresh targets the selected browse destination
Refreshing the library panel SHALL invoke the refresh behavior belonging to the selected destination. Home SHALL reload Home content. An Emby library SHALL reload that library. An Audiobookshelf library SHALL restart its catalog request rather than stop after clearing its current catalog state. Feeds SHALL refetch subscribed feeds. Refreshing the queue panel SHALL remain a refresh of the visible queue and SHALL NOT depend on the selected browse destination.

#### Scenario: User refreshes a browse destination
- **WHEN** the library panel is focused on Home, an Emby library, an Audiobookshelf library, or Feeds and the user invokes refresh
- **THEN** mbv SHALL refresh only the selected destination through its Service-specific refresh behavior

#### Scenario: User refreshes the queue panel
- **WHEN** the queue panel is focused and the user invokes refresh
- **THEN** mbv SHALL refresh the visible queue without refreshing or indexing the selected browse destination

### Requirement: Help and context actions reflect the selected destination
When the library panel has focus, destination-specific help SHALL place the selected Home, Emby, Audiobookshelf, or Feeds section first. Shared and other destination sections MAY remain visible after it, but SHALL NOT be presented as behavior of the selected destination. The Home section SHALL list `[` / `]` for section switching, `Ctrl+W` for watched-state changes, and `Ctrl+A` for enqueue.

The Audiobookshelf section SHALL list show-mode `Up` / `Down` or `k` / `j` for row movement, `Left` / `Right` or `h` / `l` for adjacent shows, `PageUp` / `PageDown` for paging, `Home` / `End` for first/last show, and `Enter` / `Space` for entering episode selection. It SHALL list episode-mode `Up` / `Down` or `k` / `j` for episode movement, `[` / `]` for played-state filter cycling, `Esc` / `Backspace` for returning to show selection, and `Enter` / `Space` as inert until playback support is applied.

A Service-specific context menu SHALL open only after panel and destination dispatch resolves a supported action target. Existing Home and Emby browse menus SHALL remain available for supported Emby-item targets. Audiobookshelf and Feeds browse targets, non-Emby queue items, and missing or stale targets SHALL NOT open an Emby context menu.

#### Scenario: User opens help from a destination
- **WHEN** the library panel is focused and the user opens help
- **THEN** mbv SHALL place the selected destination's section before the other help sections
- **THEN** it SHALL NOT present Emby-only shortcuts as Audiobookshelf or Feeds behavior

#### Scenario: User requests a context menu
- **WHEN** the selected browse destination is Audiobookshelf or Feeds, or the resolved target is non-Emby, absent, or stale
- **THEN** mbv SHALL NOT open an Emby context menu for that destination or target
- **THEN** selection and browse state SHALL remain usable

#### Scenario: User opens help while the queue has focus
- **WHEN** the queue panel has focus and the user opens help
- **THEN** mbv SHALL place the Queue section first and retain the selected browse destination without treating it as the active help context
- **THEN** returning focus to the library panel SHALL resume destination-specific help and actions for that retained destination

### Requirement: Browse models remain Service-specific until explicit playback submission
Emby, Audiobookshelf, and Feed catalog identities and browse state SHALL remain concrete and separate. Shared playback state SHALL be entered only by an explicit supported play or enqueue action that constructs the corresponding Service-native queue item.

#### Scenario: User navigates Service-specific content
- **WHEN** the user navigates, filters, refreshes, selects, or opens help for an Emby, Audiobookshelf, or Feeds destination
- **THEN** mbv SHALL retain that Service's native browse identity and state
- **THEN** it SHALL NOT convert the selection into another Service's browse model or a queue item

#### Scenario: Audiobookshelf episode activation precedes playback support
- **WHEN** the user activates or requests enqueue for an Audiobookshelf episode before its playback capability is applied
- **THEN** mbv SHALL consume the request while retaining the selected episode and leaving destination, queue, playback, and Service state unchanged
- **THEN** mbv SHALL NOT quit or enter Emby or Feed handling
