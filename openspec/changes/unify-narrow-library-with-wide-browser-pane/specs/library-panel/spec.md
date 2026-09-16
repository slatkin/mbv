## ADDED Requirements

### Requirement: The non-Wide library panel is the Wide browser pane without a Hero

In non-Wide geometry the Library panel SHALL compose the Wide browser pane's own rows — the Selector
row, the optional List controls row, and the list box — through the same browser-pane composition and
the same surface identities the Wide list pane uses, with the Hero pane absent. The non-Wide list box
SHALL be filled with the `LibraryPanel` pair and SHALL stripe with the `MainContentBox` pair, exactly
as the Wide Browser-pane list does; it SHALL NOT carry a body fill or a scrollbar-column fill of its
own. The non-Wide library column body — the panel placement, the Selector row's spacer row, the status
band's padding rows and the list's scrollbar column — SHALL resolve the fixed app backdrop
(`#2d353b`) in every focus state, as the Wide library column does. The status row inside the status
band SHALL remain the status bar's own surface. No non-Wide-specific library surface identity SHALL
exist.

#### Scenario: A focused non-Wide library keeps the column backdrop

- **WHEN** the library panel renders in non-Wide geometry with the library panel focused
- **THEN** the panel placement, the Selector row's spacer row, the status band's padding rows and the
  list's scrollbar column all carry `#2d353b`
- **AND** the status row keeps the status bar's own surface

#### Scenario: The non-Wide list box matches the Wide Browser-pane list

- **WHEN** a non-Wide library list renders in either focus state
- **THEN** its list box carries the `LibraryPanel` fill for that state
- **AND** its alternating rows carry the `MainContentBox` fill for that state
- **AND** its scrollbar column carries the same fill the Wide Browser-pane list's column carries

#### Scenario: The non-Wide column body does not follow focus

- **WHEN** the library panel renders in non-Wide geometry with the library panel focused and without it
- **THEN** the column body's painted fill is identical in both frames

#### Scenario: Wide keeps its own column fill

- **WHEN** the library panel renders in Wide geometry, focused or not
- **THEN** the library column paints its fixed backdrop in both focus states
- **AND** no part of the Wide skeleton resolves a non-Wide body identity

#### Scenario: Mini follows the non-Wide presentation

- **WHEN** the terminal is narrow enough for the mini view
- **THEN** the library pane paints the same fixed backdrop and the same list-box pair as Narrow
- **AND** there is no Mini-specific paint bit or resting-only override

## REMOVED Requirements

### Requirement: The non-Wide library panel body has one fill authority

**Reason**: The non-Wide library no longer has a palette of its own. Its column body is the Wide
library column's fixed backdrop, so there is no non-Wide body identity left to resolve.

**Migration**: Any site that resolved the non-Wide body identity now resolves the library column's
fixed backdrop; the unified non-Wide browser-pane requirement asserts the resulting fills.

### Requirement: The non-Wide list panel is inset inside its own surface

**Reason**: The non-Wide list is the Wide browser pane, so its claim and row-flow rectangles are the
Wide pane's rather than a non-Wide-specific inset.

**Migration**: The non-Wide list takes the Wide browser pane's claim and row-flow geometry; retained
geometry, hit resolution and menu placement read those rectangles.