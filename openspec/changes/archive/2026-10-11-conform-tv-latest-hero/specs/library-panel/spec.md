# Spec Delta

## MODIFIED Requirements

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
- **THEN** every library destination switches between the Wide panel and the non-Wide standard-list panel at the same width and height
- **AND** the Wide panel has two panes, except for a list of non-media rows, which has no Hero pane
- **AND** no selected row changes height during that transition

## ADDED Requirements

### Requirement: A list of non-media rows has no Hero pane
In Wide geometry, when a destination's current list rows are not media items, the Library panel SHALL paint no Hero pane and the list SHALL span the whole content area. Every other list SHALL keep the Hero pane, empty when its selection has no hero. The panel SHALL decide this from the destination's content, never from which destination it is.

#### Scenario: TV Upcoming spans the content area
- **WHEN** a TV library shows `Upcoming` in Wide geometry
- **THEN** no Hero pane is painted and the list fills the content area below the Selector band

#### Scenario: A media list without a selected hero keeps its pane
- **WHEN** a media list in Wide geometry has a selection that produces no hero
- **THEN** the Hero pane is still painted, empty, at its usual geometry
