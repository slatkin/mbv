## MODIFIED Requirements

### Requirement: Enter opens the selected item's Hero in the Library pane

In every geometry where the Wide Hero arrangement does not apply, pressing Enter on a selected hero-bearing canonical browser row SHALL open a Library Hero overlay for that item. A Grouped Music album leaf SHALL retain that Enter behavior; an artist root SHALL instead open the overlay when Right is pressed on the already expanded root, because Enter toggles its expansion. The overlay SHALL present the same Hero header, metadata, overview, artwork, provider links, and optional Workspace content that the selected target's Wide Hero pane presents. Inline Search results and an active Grouped Music tree filter SHALL retain their specified activation behavior and SHALL NOT open the overlay.

For an item, artist root, or album leaf with a Workspace, opening the overlay SHALL give focus to its constituent media list. For an item without a Workspace, opening SHALL give focus to the Hero overlay and a subsequent Enter SHALL perform the item's existing activation behavior.

Audiobookshelf podcast episodes are not hero-bearing browser rows: the podcast tab lists downloaded episodes directly, its hero has no Workspace and no inline or overlay presentation, and Enter on a selected episode performs its play activation immediately.

#### Scenario: Parent with constituent media opens focused Workspace

- **WHEN** the user presses Enter on a selected series, album, or audiobook book in non-Wide geometry
- **THEN** the Library Hero overlay opens for that selected item
- **AND** its episode, track, or chapter media list holds focus

#### Scenario: Grouped Music artist root opens focused Workspace

- **WHEN** the user presses Right on an already expanded artist root in non-Wide Grouped Music with no tree filter active
- **THEN** the Library Hero overlay opens for that artist root
- **AND** its grouped artist-track Workspace holds focus

#### Scenario: Filtered Grouped Music album dismisses into its overlay

- **WHEN** the user presses Enter on an album leaf while a non-Wide Grouped Music tree filter is active
- **THEN** the same key action dismisses the filter and focuses that album in the unfiltered tree
- **AND** its Library Hero overlay opens with the album-track Workspace focused

#### Scenario: Leaf requires a second Enter

- **WHEN** the user presses Enter on a selected hero-bearing item without a Workspace in non-Wide geometry
- **THEN** the first Enter opens the Library Hero overlay without activating the item
- **AND** a subsequent Enter while the overlay holds Library focus performs the item's existing activation

#### Scenario: Browser double-click opens detail first

- **WHEN** the user double-clicks a canonical browser row in non-Wide geometry
- **THEN** its Library Hero overlay opens without directly activating the item
- **AND** a later Enter or double-click inside a leaf Hero performs its existing activation

#### Scenario: Grouped Music tree double-click opens detail first

- **WHEN** the user double-clicks an artist root or album leaf in non-Wide Grouped Music with no tree filter active
- **THEN** its Library Hero overlay opens without directly running a root playback action or album activation
- **AND** the corresponding artist-track or album-track Workspace holds focus

#### Scenario: Inline Search is unchanged

- **WHEN** Inline Search is active in non-Wide geometry and the user presses Enter on a result
- **THEN** the result performs its existing navigation or activation behavior
- **AND** no Library Hero overlay opens

#### Scenario: Podcast episode plays without an overlay

- **WHEN** the user presses Enter on a selected Audiobookshelf podcast episode in non-Wide geometry
- **THEN** the episode performs its play activation
- **AND** no Library Hero overlay opens
