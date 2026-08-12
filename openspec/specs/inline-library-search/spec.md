# inline-library-search Specification

## Purpose
Lets the user narrow the library list they are already looking at by typing a fuzzy query into a small input box above that list, so filtering a library never hides the library.
## Requirements
### Requirement: The search key opens an inline input box above the library list

Pressing the search key while a library tab is focused SHALL open a three-row bordered input box occupying the top of the library list area. The list area SHALL shrink by exactly the height of the box; the box SHALL NOT overlay or dim any part of the view.

The search key SHALL have no effect on the home tab, which has no library list to filter.

The search key SHALL have no effect while the library panel is not the focused panel.

When the library list area is too short to fit the input box, the box SHALL NOT be drawn and the list SHALL render at its normal height.

#### Scenario: Opening search on a library tab

- **WHEN** the user presses the search key with a library tab focused
- **THEN** a three-row bordered input box SHALL appear at the top of the library list area
- **AND** the list beneath it SHALL be reduced in height by three rows

#### Scenario: Search key on the home tab

- **WHEN** the user presses the search key on the home tab
- **THEN** nothing SHALL happen and no input box SHALL appear

#### Scenario: Library list too short for the box

- **WHEN** the library list area is shorter than the input box
- **THEN** the input box SHALL NOT be rendered
- **AND** the library list SHALL occupy the full list area

### Requirement: Typing edits the query and re-filters the list in place

While the input box is open, printable characters SHALL be appended to the query and SHALL NOT be interpreted as library list shortcuts. Each change to the query SHALL re-score the corpus and replace the rendered list contents with the matches.

Matching SHALL be fuzzy, scored against each item's display name, and results SHALL be ordered by descending match score. An empty query SHALL show the whole corpus in its original order.

The selection SHALL reset to the first result whenever the query changes.

#### Scenario: Typing a query

- **WHEN** the user types characters into the open search box
- **THEN** the characters SHALL appear in the input box
- **AND** the list below SHALL show only items whose names fuzzy-match the query, ordered by descending score

#### Scenario: A list shortcut letter is typed

- **WHEN** the user types a character that is otherwise a library list shortcut
- **THEN** it SHALL be inserted into the query
- **AND** the library list action bound to that character SHALL NOT run

#### Scenario: Query emptied by deletion

- **WHEN** the user deletes back to an empty query without dismissing the search
- **THEN** the list SHALL show the whole corpus in its original order

### Requirement: The corpus spans the whole library, not the visible page

The corpus SHALL be the library's full item set, independent of lazy pagination and independent of any active letter-range filter. When the full set is not yet loaded at the moment search opens, the full-library fetch SHALL be started and the input box SHALL show a loading indicator until it completes.

A library configured for recursive album search SHALL use its album index as the corpus, matching against each album's indexed search text rather than its bare display name.

#### Scenario: Only part of the library has been paged in

- **WHEN** the user opens search on a library whose items are only partly loaded
- **THEN** the full item set SHALL be fetched
- **AND** the input box SHALL show a loading indicator until the fetch completes

#### Scenario: A letter-range filter is active

- **WHEN** the user opens search while a letter-range filter narrows the library view
- **THEN** the corpus SHALL span the entire library, not the filtered range

#### Scenario: Corpus still loading

- **WHEN** the query changes while the corpus fetch is still in flight
- **THEN** the view SHALL indicate that loading is in progress rather than presenting an empty result set as final

### Requirement: Results render as a flat list on every library type

While search is open, results SHALL render as a single flat list through the plain column-aware list renderer. No grouping applied by the underlying browse view — artist-grouped album headers or letter headers — SHALL be applied to search results.

Results SHALL NOT be reordered, mismatched, or omitted as a consequence of any grouping the browse view would otherwise apply. Every displayed row SHALL show the item that actually matched the query.

#### Scenario: Searching a grouped music library

- **WHEN** the user searches a music library whose browse view groups albums under artist headers
- **THEN** every album matching the query SHALL appear, ordered by match score
- **AND** each row SHALL display the album it matched, not a different album
- **AND** no artist headers SHALL be drawn

#### Scenario: Searching a letter-grouped library

- **WHEN** the user searches a library whose browse view groups items under letter headers
- **THEN** results SHALL render as a flat list with no letter headers

#### Scenario: Search dismissed on a grouped library

- **WHEN** the user dismisses search on a library whose browse view groups its items
- **THEN** the grouped presentation SHALL return unchanged

### Requirement: Results are navigable and activatable without leaving search

While the input box is open, Up and Down SHALL move the selection through the result list, the page keys SHALL move it by one viewport, and Home and End SHALL jump to the first and last result. The activation key SHALL activate the selected result exactly as activating that item from the unfiltered list would.

Cursor movement SHALL NOT alter the query, and typing SHALL NOT alter the cursor beyond the reset that a query change causes.

#### Scenario: Moving through results

- **WHEN** the user presses Down with results showing
- **THEN** the selection SHALL move to the next result
- **AND** the query SHALL be unchanged

#### Scenario: Activating a result

- **WHEN** the user presses the activation key on a selected result
- **THEN** the application SHALL act on that item as it would from the unfiltered library list

#### Scenario: Navigating an empty result set

- **WHEN** the query matches nothing and the user presses Up or Down
- **THEN** nothing SHALL happen and the search SHALL stay open

### Requirement: Dismissing search restores the unfiltered list

Pressing the dismiss key SHALL close the input box and restore the library list to its unfiltered contents, presentation, and prior navigation position. Pressing the delete key on an already-empty query SHALL dismiss the search the same way.

Dismissal SHALL discard the query and results; reopening search SHALL start from an empty query.

#### Scenario: Dismissing with the dismiss key

- **WHEN** the user presses the dismiss key while the search box is open
- **THEN** the box SHALL close
- **AND** the library list SHALL show its unfiltered items in their normal order and grouping

#### Scenario: Deleting past the start of the query

- **WHEN** the query is empty and the user presses the delete key
- **THEN** the search SHALL be dismissed

#### Scenario: Reopening after dismissal

- **WHEN** the user dismisses search and immediately reopens it
- **THEN** the query SHALL be empty

