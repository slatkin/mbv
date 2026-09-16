## MODIFIED Requirements

### Requirement: Every library destination renders through one Library panel skeleton

Home, Movies, Emby home videos, TV shows, grouped Music, Audiobookshelf Books, Audiobookshelf Podcasts and Feeds SHALL each render through the same Library panel: the Wide library panel when the shared Wide width and minimum-height conditions are met, and the non-Wide library panel otherwise. The panel SHALL own every row, pane, fill, border, gap, inset, overlay and placeholder position. A destination SHALL supply only typed content for the panel's slots and SHALL NOT paint outside a slot, add a slot, or select an alternative skeleton, pane order, fill, inset, border, or overlay placement.

A slot a destination does not fill SHALL render as absent in the same way on every destination. Two destinations supplying the same kind of content SHALL render it identically at the same geometry. In non-Wide geometry the Browser pane SHALL show ordinary fixed-height canonical rows; selected-row replacement and Inline hero SHALL NOT render.

#### Scenario: Two destinations with the same slot content
- **WHEN** two library destinations supply the same kind of content for the same slots at the same terminal size
- **THEN** their panes, rows, fills, borders and insets occupy the same cells with the same surfaces

#### Scenario: A destination has no content for an optional slot
- **WHEN** a destination supplies no List controls row content
- **THEN** no List controls row renders and the rows below move up exactly as on every other destination without that content

#### Scenario: The breakpoint is crossed
- **WHEN** the terminal crosses the shared Wide conditions in either direction
- **THEN** every library destination switches between the Wide two-pane panel and the non-Wide standard-list panel at the same width and height
- **AND** no selected row changes height during that transition

### Requirement: Narrow inline hero has one form

The Narrow inline hero form is removed. In every non-Wide Library panel, the selected item SHALL remain an ordinary fixed-height canonical media row until the user opens the Library Hero overlay. No destination SHALL supply or paint a separate non-Wide Hero form.

#### Scenario: Narrow Movie and Narrow Audiobookshelf podcast

- **WHEN** a Movie and an Audiobookshelf podcast are each selected where the Wide Hero arrangement does not fit
- **THEN** both remain ordinary fixed-height media rows
- **AND** no Hero content replaces either row

#### Scenario: Narrow landscape artwork

- **WHEN** an item with landscape artwork is selected where the Wide Hero arrangement does not fit
- **THEN** its artwork does not render inside or beside the selected browser row
- **AND** opening its Library Hero overlay presents the artwork through the shared Hero content

### Requirement: Narrow workspace lists open only as the selection modal

The Narrow constituent-list selection modal is removed. In every non-Wide Library panel, constituent items SHALL be reachable through the selected parent's Library Hero overlay, whose Workspace uses the same canonical list and provider-owned content as Wide Hero.

#### Scenario: Narrow Audiobookshelf book

- **WHEN** an Audiobookshelf book is selected where the Wide Hero arrangement does not fit
- **THEN** no chapter rows render inside the browser list
- **AND** Enter opens the Library Hero overlay with its chapter Workspace focused

#### Scenario: Narrow Audiobookshelf podcast

- **WHEN** an Audiobookshelf podcast show is selected where the Wide Hero arrangement does not fit
- **THEN** no filter pills or episode rows render inside the browser list
- **AND** Enter opens the Library Hero overlay with its episode Workspace focused
