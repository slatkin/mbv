## ADDED Requirements

### Requirement: The non-Wide library list owns the surface under it

The non-Wide library list's paint policy SHALL carry the list's own body fill. When a list carries
one, the painter SHALL fill its claim rect with that fill before painting rows, and the scrollbar
column SHALL resolve that same fill. A list that carries no body fill SHALL keep resolving the
scrollbar column through the owning-surface identity of its selected-row surface, as before, so the
Wide lists and the Queue are unaffected.

#### Scenario: The scrollbar column is the list's own body

- **WHEN** a non-Wide library list with its own body fill overflows and paints its scrollbar column
- **THEN** that column carries the list's body fill, not the selected-row surface
- **AND** no cell of the column carries a tone the list body does not

#### Scenario: Lists without their own body fill are unchanged

- **WHEN** a Wide library list or the Queue list paints its scrollbar column
- **THEN** the column resolves the owning-surface identity of its selected-row surface
- **AND** its painted fill is unchanged from before this capability

### Requirement: Non-Wide library lists stripe with the library-panel pair

The non-Wide library list SHALL stripe with the `LibraryPanel` pair — focused `#3c4841`, resting
`#333c43` — while its own list box is filled with the `MainContentBox` pair (`#48584e` focused,
`#2d353b` resting), so the stripe differs from the fill of its own list box in both focus states.
Stripe parity, the selected row keeping its own parity, and `Heading`/`Spacer` rows never striping
SHALL remain as the Wide requirement already defines them.

#### Scenario: A focused non-Wide list alternates two tones

- **WHEN** a non-Wide library list renders with library focus
- **THEN** the alternate visible rows carry `#3c4841`
- **AND** the rows between them carry the `MainContentBox` focused fill `#48584e`

#### Scenario: A resting non-Wide list alternates two tones

- **WHEN** a non-Wide library list renders without library focus
- **THEN** the alternate visible rows carry `#333c43`
- **AND** the rows between them carry the `MainContentBox` resting fill `#2d353b`

#### Scenario: The stripe never equals its own list box

- **WHEN** a non-Wide library list renders in either focus state
- **THEN** its stripe colour differs from the fill its own list box is painted with in that state
