# Spec Delta

## MODIFIED Requirements

### Requirement: Shows selects one view

The GUI Shows destination's list header SHALL paint one dropdown with exactly four single-choice entries: All, New, Upcoming, and Favourites, in that order. All SHALL be the default. A press on one entry SHALL replace the active entry and close the dropdown. The choice SHALL be a view of the destination and SHALL NOT be a collection set, a collection target, a collection filter, or a key binding. The saved launch state SHALL keep All, New, and Upcoming and SHALL restore Favourites as All. The TUI Client's Shows modes SHALL NOT change.

#### Scenario: Shows opens on All

- **WHEN** the user opens a Shows destination with no saved view
- **THEN** the list shows the library's series sorted by name with the letter rail, the Hero and Workspace are present, and the dropdown marks All

#### Scenario: One view replaces another

- **GIVEN** All is active and the view dropdown is open
- **WHEN** the user presses New
- **THEN** New is the only marked entry and the dropdown closes

#### Scenario: Favourites is the fourth entry

- **WHEN** the user opens the Shows view dropdown
- **THEN** it lists All, New, Upcoming, and Favourites, in that order

#### Scenario: Favourites does not survive a restart

- **GIVEN** Favourites is the active Shows view when mbv quits
- **WHEN** mbv starts again and opens the Shows destination
- **THEN** the dropdown marks All

## ADDED Requirements

### Requirement: Every other Emby library destination offers a Favourites view

The GUI destination of every Emby library other than a Shows library, meaning a plain library such
as Movies and a Music library, SHALL paint a view dropdown in its list header with exactly two
single-choice entries: All and Favourites, in that order. All SHALL be the default and the choice
SHALL NOT be saved across a restart. Audiobookshelf and Feeds destinations SHALL paint no such
dropdown.

#### Scenario: Movies library

- **WHEN** the user opens a Movies destination
- **THEN** its list header shows a view dropdown that marks All and offers Favourites

#### Scenario: Music library

- **WHEN** the user opens a Music destination and presses Favourites
- **THEN** the list shows that library's favourite artists, albums, and songs and the dropdown marks Favourites

#### Scenario: Audiobookshelf library

- **WHEN** the user opens an Audiobookshelf destination
- **THEN** its list header shows no view dropdown

### Requirement: A Favourites view replaces the destination's list column

While a library's view is Favourites, the destination SHALL paint the Favourites list in the list
column in place of that destination's All list, and the Hero SHALL show the selected favourite.
Pressing All SHALL restore the All list with the selection and scroll it had before.

#### Scenario: Return to All

- **GIVEN** the user scrolled the Movies All list, then pressed Favourites
- **WHEN** the user presses All
- **THEN** the Movies list shows the same row selected at the same position as before

#### Scenario: Hero follows a favourite

- **WHEN** the user selects a favourite row
- **THEN** the Hero shows that item
