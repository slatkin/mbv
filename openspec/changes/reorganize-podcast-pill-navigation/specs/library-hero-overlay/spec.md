## MODIFIED Requirements

### Requirement: Enter opens the selected item's Hero in the Library pane

In every geometry where the Wide Hero arrangement does not apply, pressing Enter on a selected hero-bearing canonical browser row SHALL open a Library Hero overlay for that item. The overlay SHALL present the same Hero header, metadata, overview, artwork, provider links, and optional Workspace content that the item's Wide Hero pane presents. Inline Search results SHALL retain their existing activation behavior and SHALL NOT open the overlay.

For an item with a Workspace, opening the overlay SHALL give focus to its constituent media list. For an item without a Workspace, opening SHALL give focus to the Hero overlay and a subsequent Enter SHALL perform the item's existing activation behavior. Audiobookshelf podcast episodes are hero-bearing leaves without a Workspace; their existing activation is the episode's play action.

#### Scenario: Parent with constituent media opens focused Workspace

- **WHEN** the user presses Enter on a selected series, album, or audiobook book in non-Wide geometry
- **THEN** the Library Hero overlay opens for that selected item
- **AND** its episode, track, or chapter media list holds focus

#### Scenario: Leaf requires a second Enter

- **WHEN** the user presses Enter on a selected hero-bearing item without a Workspace in non-Wide geometry
- **THEN** the first Enter opens the Library Hero overlay without activating the item
- **AND** a subsequent Enter while the overlay holds Library focus performs the item's existing activation

#### Scenario: Podcast episode leaf opens its episode hero

- **WHEN** the user presses Enter on a selected podcast episode in non-Wide geometry
- **THEN** the Library Hero overlay opens presenting that episode's hero (episode facts, parent-show credits, and the parent show's cover, with no Workspace)
- **AND** a subsequent Enter while the overlay holds Library focus plays the episode

#### Scenario: Browser double-click opens detail first

- **WHEN** the user double-clicks a canonical browser row in non-Wide geometry
- **THEN** its Library Hero overlay opens without directly activating the item
- **AND** a later Enter or double-click inside a leaf Hero performs its existing activation

#### Scenario: Inline Search is unchanged

- **WHEN** Inline Search is active in non-Wide geometry and the user presses Enter on a result
- **THEN** the result performs its existing navigation or activation behavior
- **AND** no Library Hero overlay opens
