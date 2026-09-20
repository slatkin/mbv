# Spec Delta

## RENAMED Requirements

- FROM: `### Requirement: The list pane has one Selector row and one optional List controls row`
  TO: `### Requirement: The Selector row is a full-width band above one list box`

## MODIFIED Requirements

### Requirement: The Selector row is a full-width band above one list box

In Wide geometry the Selector row SHALL be a single full-width band across the
top of the whole Library panel — one pill bar followed by the panel's
parent-background spacer row — spanning both the Hero pane and the Browser (list)
pane. The Hero/Browser split SHALL be computed over the content area below that
band, so both panes begin below the Selector row and its spacer.

The full-width band SHALL be independent of the Hero/Browser split drag-resize:
the draggable gap between the two panes SHALL cover only the content area below
the band, never the pill row or the spacer row.

This full-width band is the Selector row (pills), NOT a hero or detail area. The
prohibitions on reserving a full-width area above the browser
(`library-list-hero`, `right-panel-arrangements`) govern the selected item's
hero/detail block, which SHALL continue to render beside the browser in Wide and
as an in-flow row replacement in non-Wide; they do not constrain the Selector
row's placement.

In non-Wide geometry the panel is a single full-width pane, and the Selector row
SHALL span that pane's width, followed by its spacer row.

Below the Selector band the list pane SHALL present exactly the list box. There
SHALL be no List controls row and no secondary controls row of any kind. The
Selector row carries destination browse selectors, including the Feeds All /
Played / Unplayed watched filter followed by its feed-group pills. No destination
SHALL render a second pill bar or insert a row between the Selector band and the
list box.

#### Scenario: Feeds renders its selectors
- **WHEN** the Feeds destination renders with subscriptions available
- **THEN** its All / Played / Unplayed filter and feed-group pills render in the one Selector row
- **AND** no second pill bar and no secondary controls row render

#### Scenario: The Wide Selector band spans both panes
- **WHEN** a library destination renders at Wide geometry
- **THEN** the Selector row and its spacer occupy a full-width band across the top of both the Hero pane and the Browser pane
- **AND** both panes begin below that band

#### Scenario: The Wide Selector band is independent of the split drag
- **WHEN** the user drags the gap between the Hero and Browser panes at Wide geometry
- **THEN** the drag adjusts only the panes below the Selector band
- **AND** the pill row and the spacer row are not part of the draggable gap

#### Scenario: An Emby home-video library renders its count
- **WHEN** an Emby home-video library renders
- **THEN** no item-count label and no List controls row render anywhere in the panel (the former home-video count row is removed)

#### Scenario: Selector pill hit geometry
- **WHEN** the user clicks a pill in any destination's Selector row
- **THEN** the pill painted under the pointer in the latest frame is selected

### Requirement: Every library destination renders through one Library panel skeleton

Home, Movies, Emby home videos, TV shows, grouped Music, Audiobookshelf Books, Audiobookshelf Podcasts and Feeds SHALL each render through the same Library panel: the Wide library panel when the shared Wide width and minimum-height conditions are met, and the non-Wide library panel otherwise. The panel SHALL own every row, pane, fill, border, gap, inset, overlay and placeholder position. A destination SHALL supply only typed content for the panel's slots and SHALL NOT paint outside a slot, add a slot, or select an alternative skeleton, pane order, fill, inset, border, or overlay placement.

A slot a destination does not fill SHALL render as absent in the same way on every destination. Two destinations supplying the same kind of content SHALL render it identically at the same geometry. In non-Wide geometry the Browser pane SHALL show ordinary fixed-height canonical rows; selected-row replacement and Inline hero SHALL NOT render.

#### Scenario: Two destinations with the same slot content
- **WHEN** two library destinations supply the same kind of content for the same slots at the same terminal size
- **THEN** their panes, rows, fills, borders and insets occupy the same cells with the same surfaces

#### Scenario: A destination has no content for an optional slot
- **WHEN** a destination's selected item has no constituent items to fill the Workspace slot
- **THEN** no Workspace box renders and the content below it moves up exactly as on every other destination without that content

#### Scenario: The breakpoint is crossed
- **WHEN** the terminal crosses the shared Wide conditions in either direction
- **THEN** every library destination switches between the Wide two-pane panel and the non-Wide standard-list panel at the same width and height
- **AND** no selected row changes height during that transition

### Requirement: Inline Search replaces the Selector row and list box

While Inline Search is active in a searchable destination, the search box SHALL render in the Selector
row's place and the results SHALL render in the list box's place, with the rest of the panel unchanged.
In Wide geometry the Selector row is the full-width band, so the search box SHALL span the full panel
width, above both panes. The Selector row's place SHALL be reserved for the search box even when the
destination supplies no Selector row.
There SHALL be no other library search presentation inside the Library panel.

#### Scenario: Inline Search opens on a Wide destination
- **WHEN** Inline Search is active on a Wide searchable destination
- **THEN** the search box occupies the full-width Selector band and the results occupy the list box
- **AND** the Hero pane continues to render the destination's selected item

### Requirement: The non-Wide library panel is the Wide browser pane without a Hero

In non-Wide geometry the Library panel SHALL compose the Wide browser pane's own rows — the Selector
row and the list box — through the same browser-pane composition and
the same surface identities the Wide list pane uses, with the Hero pane absent. The non-Wide list box
SHALL be filled with the `LibraryPanel` pair and SHALL stripe with the `MainContentBox` pair, exactly
as the Wide Browser-pane list does; it SHALL NOT carry a body fill or a scrollbar-column fill of its
own. The non-Wide library column body — the panel placement, the Selector row's spacer row, the status
band's padding rows and the list's scrollbar column — SHALL resolve the fixed app backdrop
(`#2d353b`) in every focus state, as the Wide library column does. The status row inside the status
band SHALL remain the status bar's own surface. No non-Wide-specific library surface identity SHALL
exist. The non-Wide browser rail's focus input SHALL be the library panel's own focus bit alone: a
Workspace focused in Wide geometry SHALL NOT subtract the rail's focused surface after a transition
to non-Wide geometry, so a panel-focused non-Wide rail always paints one visible focus indicator. A
non-Wide list slot with no rows to spare SHALL keep its single row rather than collapsing to an empty
rect, so a very short panel still paints one row.

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

#### Scenario: A very short non-Wide panel keeps one row

- **WHEN** a non-Wide library panel is too short to afford the row-flow inset
- **THEN** its list slot keeps a single row instead of collapsing to an empty rect

#### Scenario: A Workspace focused in Wide does not darken the non-Wide rail

- **WHEN** the Hero Workspace holds focus in Wide geometry and the panel transitions to non-Wide
  geometry with the Library Hero overlay closed
- **THEN** a panel-focused non-Wide browser rail still paints its focused list-box fill
- **AND** the rail's focus input does not depend on the Workspace's focus bit

#### Scenario: Wide keeps its own column fill

- **WHEN** the library panel renders in Wide geometry, focused or not
- **THEN** the library column paints its fixed backdrop in both focus states
- **AND** no part of the Wide skeleton resolves a non-Wide body identity

#### Scenario: Mini follows the non-Wide presentation

- **WHEN** the terminal is narrow enough for the mini view
- **THEN** the library pane paints the same fixed backdrop and the same list-box pair as Narrow
- **AND** there is no Mini-specific paint bit or resting-only override
