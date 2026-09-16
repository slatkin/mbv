# library-hero-overlay Specification

## Purpose
Defines the Library-local overlay that reveals the selected item's existing Hero content in non-Wide geometry while leaving the Queue independently usable.

## Requirements

### Requirement: Enter opens the selected item's Hero in the Library pane

In every geometry where the Wide Hero arrangement does not apply, pressing Enter on a selected hero-bearing canonical browser row SHALL open a Library Hero overlay for that item. The overlay SHALL present the same Hero header, metadata, overview, artwork, provider links, and optional Workspace content that the item's Wide Hero pane presents. Inline Search results SHALL retain their existing activation behavior and SHALL NOT open the overlay.

For an item with a Workspace, opening the overlay SHALL give focus to its constituent media list. For an item without a Workspace, opening SHALL give focus to the Hero overlay and a subsequent Enter SHALL perform the item's existing activation behavior.

#### Scenario: Parent with constituent media opens focused Workspace

- **WHEN** the user presses Enter on a selected series, album, podcast show, or audiobook book in non-Wide geometry
- **THEN** the Library Hero overlay opens for that selected item
- **AND** its episode, track, or chapter media list holds focus

#### Scenario: Leaf requires a second Enter

- **WHEN** the user presses Enter on a selected hero-bearing item without a Workspace in non-Wide geometry
- **THEN** the first Enter opens the Library Hero overlay without activating the item
- **AND** a subsequent Enter while the overlay holds Library focus performs the item's existing activation

#### Scenario: Browser double-click opens detail first

- **WHEN** the user double-clicks a canonical browser row in non-Wide geometry
- **THEN** its Library Hero overlay opens without directly activating the item
- **AND** a later Enter or double-click inside a leaf Hero performs its existing activation

#### Scenario: Inline Search is unchanged

- **WHEN** Inline Search is active in non-Wide geometry and the user presses Enter on a result
- **THEN** the result performs its existing navigation or activation behavior
- **AND** no Library Hero overlay opens

### Requirement: The overlay is confined to the Library pane

The Library Hero overlay SHALL be centered within the current Library pane and SHALL occupy 85 percent of that pane's width and height, subject to fitting within the available Library area. Its elevation, Library-only dimmed backdrop, and visible `Esc` dismissal hint SHALL make its overlay status evident. It SHALL NOT paint a border frame, occupy, cover, or dim any part of a visible Queue pane.

The overlay SHALL recompute its placement from the current Library pane whenever terminal geometry or Panel mode changes. Its internal Hero composition SHALL follow the existing Wide Hero space-allocation rules, including shrinking artwork before removing a present Workspace list viewport.

#### Scenario: Both panels remain visually distinct

- **WHEN** the Library Hero overlay is open while both Library and Queue are visible
- **THEN** the overlay and dimmed backdrop are contained within the Library pane
- **AND** the Queue pane remains uncovered and undimmed

#### Scenario: Geometry changes while open

- **WHEN** the terminal is resized or Panel mode changes while the overlay remains applicable
- **THEN** the overlay is recentered and resized from the resulting Library pane
- **AND** no overlay cell crosses into the Queue pane

### Requirement: Queue remains independently operable

A visible Queue SHALL remain focusable and fully operable while the Library Hero overlay is open. Moving panel focus to Queue SHALL leave the overlay open, preserve its internal cursor and scroll state, and render it unfocused. Returning panel focus to Library SHALL restore the overlay's previous internal focus and state.

While Queue holds focus, Queue SHALL retain its complete keyboard behavior, including Esc handling; the Library Hero overlay SHALL NOT claim those keys.

#### Scenario: Queue takes focus

- **WHEN** panel focus moves from an open Library Hero overlay to Queue
- **THEN** the overlay remains open and renders unfocused
- **AND** Queue receives its ordinary keyboard and pointer input

#### Scenario: Library focus returns

- **WHEN** panel focus returns to Library while its Hero overlay remains open
- **THEN** the overlay regains its prior internal focus, cursor, and scroll position

#### Scenario: Escape remains Queue-local

- **WHEN** the Hero overlay is open and Queue holds focus
- **THEN** pressing Esc performs Queue's existing Esc behavior
- **AND** the Hero overlay remains open

### Requirement: Dismissal is explicit and does not pass through

While Library holds focus, pressing Esc SHALL dismiss the Library Hero overlay, restore focus to the underlying browser list, and preserve that list's selected target and scroll position. Clicking the dimmed Library area outside the overlay SHALL also dismiss it, and that pointer gesture SHALL NOT select, activate, or otherwise mutate an underlying browser row. Clicking Queue SHALL operate Queue and SHALL NOT dismiss the overlay.

Changing the active Library destination SHALL dismiss the overlay. If refresh removes the stable parent target represented by the overlay, the overlay SHALL dismiss rather than bind to a different selected item.

#### Scenario: Escape dismisses from Library

- **WHEN** the Hero overlay is open and Library holds focus
- **THEN** pressing Esc closes it and restores browser-list focus
- **AND** the browser keeps the same selected target and scroll position

#### Scenario: Backdrop click dismisses without click-through

- **WHEN** the user clicks the dimmed Library area outside the Hero overlay
- **THEN** the overlay closes
- **AND** the underlying browser receives no part of that pointer gesture

#### Scenario: Queue click leaves overlay open

- **WHEN** the user clicks a target in the visible Queue while the Hero overlay is open
- **THEN** Queue handles the click normally
- **AND** the Hero overlay remains open

#### Scenario: Destination changes

- **WHEN** the active Library destination changes while the Hero overlay is open
- **THEN** the overlay closes rather than appearing over the new destination

### Requirement: Workspace interaction matches the Wide Hero pane

A Workspace shown in the Library Hero overlay SHALL use the same canonical media-list owner, rows, selectors, loading and empty states, stable-target behavior, and provider-specific intent translation as the corresponding Wide Hero Workspace. Activating a track, episode, chapter, or other constituent row SHALL perform its existing action and SHALL leave the overlay open with its list state preserved.

The overlay SHALL NOT snapshot or duplicate Workspace rows at open time. Provider completions and ordinary refresh SHALL update the existing Workspace in place and preserve its cursor by stable target where possible.

#### Scenario: Child activation leaves overlay open

- **WHEN** the user presses Enter or double-clicks a constituent row in the overlay's focused Workspace
- **THEN** the row's existing provider-specific action occurs
- **AND** the overlay remains open with the Workspace still focused

#### Scenario: Workspace data completes while open

- **WHEN** matching provider data completes while the overlay shows a loading Workspace
- **THEN** the Workspace updates to ready or empty state in place
- **AND** the overlay remains open
