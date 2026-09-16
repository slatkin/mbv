## ADDED Requirements

### Requirement: The non-Wide library panel body has one fill authority

In non-Wide geometry the library column's own body fill SHALL resolve one named surface identity at
every site that paints it — the panel placement, the Selector row's spacer row, and the status band's
padding rows — rather than each site resolving an identity of its own. The identity SHALL follow the
library panel's focus bit: while the panel holds focus it SHALL paint the focused content-body fill
(`#3c4841`), and while it does not it SHALL paint the app backdrop (`#2d353b`). The status row inside
the status band SHALL remain the status bar's own surface, and the identity SHALL NOT be reachable
from Wide geometry.

#### Scenario: A focused non-Wide library paints one body tone

- **WHEN** the library panel renders in non-Wide geometry with the library panel focused
- **THEN** the panel placement, the Selector row's spacer row and the status band's padding rows all
  carry `#3c4841`
- **AND** the status row keeps the status bar's own surface

#### Scenario: A resting non-Wide library keeps its previous fill

- **WHEN** the library panel renders in non-Wide geometry without library focus
- **THEN** the panel placement, the Selector row's spacer row and the status band's padding rows all
  carry `#2d353b`
- **AND** they are unchanged from the fill the library column painted before this capability

#### Scenario: Wide keeps its own column fill

- **WHEN** the library panel renders in Wide geometry, focused or not
- **THEN** the library column paints its fixed backdrop in both focus states
- **AND** no part of the Wide skeleton resolves the non-Wide body identity

#### Scenario: Mini follows the non-Wide presentation

- **WHEN** the terminal is narrow enough for the mini view
- **THEN** the library paints the non-Wide body identity with the focus bit, exactly as Narrow does
- **AND** there is no Mini-specific paint bit or resting-only override

### Requirement: The non-Wide list panel is inset inside its own surface

The non-Wide list panel SHALL keep the whole list slot as its claim and take its rows from a row flow
inset by one spacer row above and below (`PANE_PAD_Y`). Those spacer rows SHALL belong to the inset's
own surface, not to the surrounding panel body. The geometry the panel reads back for row
arithmetic, hit resolution and menu placement SHALL be the inset the rows occupy. A list slot with no
rows to spare SHALL keep a single row rather than collapsing.

#### Scenario: The inset's spacer rows carry the inset's fill

- **WHEN** the non-Wide list panel renders focused
- **THEN** the spacer row above and below the rows carry the inset's own focused fill (`#48584e`)
- **AND** the surrounding panel body keeps its own fill

#### Scenario: Retained geometry is the row flow

- **WHEN** the non-Wide list panel has painted
- **THEN** the retained list geometry, its selected-row rect and the resolved hit region agree with
  the inset the rows were painted in
