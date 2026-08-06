# search-modal Specification

## Purpose
Provide a single, unified search experience — fuzzy over the current library and global across the Emby server — presented as a modal overlay, replacing the old inline library search box and the unreachable global search machinery.

## Requirements

### Requirement: Search renders as a modal, not as a filtered library list

Search SHALL present its input and results in a modal overlay drawn above a dimmed backdrop. The library list SHALL NOT render a filtered or reordered view of itself for search purposes under any circumstances.

The modal SHALL occupy 60% of the terminal width and 80% of the terminal height, centered, clamped to the terminal bounds, and floored at a minimum size below which it would be unusable.

#### Scenario: Opening search over a library

- **WHEN** the user opens search from a library tab
- **THEN** a centered modal SHALL appear over a dimmed backdrop
- **AND** the library list beneath SHALL continue to show its unfiltered items in their normal order

#### Scenario: Terminal resized while the modal is open

- **WHEN** the terminal is resized while the search modal is open
- **THEN** the modal SHALL recompute its size from the new terminal dimensions on the next frame
- **AND** the selected result SHALL remain selected

#### Scenario: Terminal too small for the proportional size

- **WHEN** 60% by 80% of the terminal is smaller than the modal's minimum usable size
- **THEN** the modal SHALL render at its minimum size, clamped to the terminal bounds

### Requirement: Modal contains no images

The search modal SHALL NOT render any image, in any image protocol, in its result rows or in its hero block. It SHALL NOT reserve layout space for an image, and SHALL NOT trigger image fetching for the items it displays.

#### Scenario: Result selected

- **WHEN** a result is selected and its hero block is drawn
- **THEN** the hero SHALL render text at the full width of the modal's content area
- **AND** no poster or artwork SHALL be drawn and no image fetch SHALL be initiated

### Requirement: Two search modes over one state

Search SHALL have exactly two modes: fuzzy search over the current library, and global search across the Emby server. Both modes SHALL share one state representation and one renderer, differing only in how results are produced.

#### Scenario: Fuzzy mode results

- **WHEN** the user types a query in fuzzy mode
- **THEN** results SHALL be produced by fuzzy-matching the query against the corpus locally
- **AND** SHALL be ordered by descending match score

#### Scenario: Global mode results

- **WHEN** the user types a query in global mode
- **THEN** the query SHALL be dispatched to the Emby search endpoint without blocking the UI
- **AND** the modal SHALL show a loading state until results arrive

#### Scenario: Search request fails

- **WHEN** a global search request returns an error
- **THEN** the loading state SHALL clear
- **AND** the error SHALL be surfaced to the user rather than rendered as an empty result set

### Requirement: Fuzzy search covers the whole library at any depth

Fuzzy search SHALL match against the entire current library regardless of how deep the user has navigated into it. The current navigation level SHALL NOT narrow the corpus.

#### Scenario: Searching from a nested level

- **WHEN** the user opens fuzzy search while navigated into a nested level of a library
- **THEN** the corpus SHALL be the whole library's items, not the current level's items

#### Scenario: Letter filter active

- **WHEN** a letter-range filter is active on the library
- **THEN** the corpus SHALL still span the entire library and SHALL NOT be truncated to the filtered range

#### Scenario: Corpus not yet loaded

- **WHEN** fuzzy search is opened before the library's full item set is available
- **THEN** the modal SHALL show a loading state rather than an empty or partial result list

### Requirement: Fuzzy search works on every library type

Fuzzy search SHALL return correct results on every library type, including music. Results SHALL NOT be reordered, mismatched, or omitted as a consequence of any grouping applied to the underlying library view.

#### Scenario: Music library search

- **WHEN** the user fuzzy-searches a music library whose browse view groups albums by artist
- **THEN** every album matching the query SHALL appear, ordered by match score
- **AND** each result row SHALL display the album it matched, not a different album

#### Scenario: Library with letter grouping

- **WHEN** the user fuzzy-searches a library whose browse view groups items under letter headers
- **THEN** results SHALL render as a flat list with no letter headers

### Requirement: A second search key press promotes fuzzy to global

Pressing the search key a second time within the same interval used for double-click detection SHALL promote an open fuzzy search to global search, preserving the typed query. The first press SHALL take effect immediately and SHALL NOT be delayed in anticipation of a second.

Promotion SHALL occur only while the query is empty. Once any other character has been typed, the search key SHALL be treated as a literal query character.

#### Scenario: Double press promotes

- **WHEN** the user presses the search key twice in quick succession from a library tab
- **THEN** the modal SHALL open on the first press in fuzzy mode
- **AND** SHALL switch to global mode on the second press

#### Scenario: Single press stays fuzzy

- **WHEN** the user presses the search key once and the interval elapses
- **THEN** the modal SHALL remain in fuzzy mode with no visible change

#### Scenario: Search key typed as a query character

- **WHEN** the user has typed at least one character and then types the search key
- **THEN** it SHALL be inserted into the query
- **AND** the mode SHALL NOT change

#### Scenario: Query preserved across promotion

- **WHEN** promotion occurs
- **THEN** the query text SHALL be preserved and dispatched to the global search

### Requirement: Search from the home tab opens global search

The home tab has no current library, so the search key SHALL open global search directly there.

#### Scenario: Search from home

- **WHEN** the user presses the search key on the home tab
- **THEN** the modal SHALL open in global mode

### Requirement: Results render as a flat list with an inline hero

Results SHALL render as a single-column flat list. The modal SHALL NOT apply groupings, letter headers, or multi-column layout of any kind.

The selected result SHALL have a hero block rendered inline directly below its row, showing the item's title, a meta line, and its overview.

#### Scenario: Hero placement

- **WHEN** a result is selected
- **THEN** the hero block SHALL appear directly below the selected row
- **AND** rows below SHALL be displaced downward rather than overdrawn

#### Scenario: Selection moved

- **WHEN** the user moves the selection to another result
- **THEN** the hero SHALL move to sit below the newly selected row and show that item's detail

#### Scenario: Selection moved out of view

- **WHEN** selection movement places the selected row or its hero outside the visible area
- **THEN** the modal SHALL scroll by the minimum amount needed to bring the whole selected block into view

#### Scenario: No results

- **WHEN** the query matches nothing
- **THEN** the modal SHALL show an empty state and SHALL NOT render a hero block

### Requirement: Results are differentiated by item type

Each result row SHALL display a type badge identifying its item type. The meta line SHALL be composed according to the item type, so that the fields shown are meaningful for that type.

Results whose meaning depends on a parent item SHALL display that parent in the row itself, not only in the hero.

#### Scenario: Mixed-type global results

- **WHEN** global search returns results of several item types
- **THEN** each row SHALL carry a badge naming its type
- **AND** each row's meta SHALL use the fields appropriate to that type

#### Scenario: Episode result

- **WHEN** a result is an episode
- **THEN** its row SHALL show the series it belongs to alongside its season and episode position

#### Scenario: Track result

- **WHEN** a result is an audio track
- **THEN** its row SHALL show its artist

#### Scenario: Badge column stable across promotion

- **WHEN** the modal is promoted from fuzzy to global
- **THEN** the badge column SHALL remain present and the row layout SHALL NOT shift

#### Scenario: Badge text is not matched

- **WHEN** the user types text matching a type name
- **THEN** matching SHALL be performed against item names only and the badge text SHALL NOT contribute to the match

### Requirement: Type filtering is available in global mode only

In global mode the user SHALL be able to narrow results to a single item type, chosen from the types actually present in the current results. In fuzzy mode no type filter SHALL be shown.

#### Scenario: Filtering global results

- **WHEN** the user narrows global results to one type
- **THEN** only results of that type SHALL be displayed
- **AND** the available types SHALL be drawn from the current result set

#### Scenario: New query resets the filter

- **WHEN** a new set of results arrives
- **THEN** any active type filter SHALL be cleared and the selection SHALL return to the first result

#### Scenario: Fuzzy mode

- **WHEN** the modal is in fuzzy mode
- **THEN** no type filter control SHALL be rendered

### Requirement: Activating a result navigates to it

Activating a result SHALL navigate to that item in the library that contains it, switching to that library's tab and placing the selection on the item, using the same navigation path already used to reveal an item from elsewhere in the application.

Results whose item type cannot be resolved to a library SHALL be excluded from the result list, so that every displayed result can be activated.

The modal SHALL close when activation navigates. When there is no selected result to activate, the activation key SHALL have no effect and the modal SHALL remain open.

#### Scenario: Activating a result from another library

- **WHEN** the user activates a global search result belonging to a library other than the current tab
- **THEN** the application SHALL switch to that item's library tab
- **AND** SHALL place the selection on that item
- **AND** the modal SHALL close

#### Scenario: Activation with no result selected

- **WHEN** the user presses the activation key while no result is selected, including while results are empty or still loading
- **THEN** nothing SHALL happen
- **AND** the modal SHALL remain open with the query intact

#### Scenario: Unnavigable types excluded

- **WHEN** the server returns results whose item type cannot be resolved to a library
- **THEN** those results SHALL NOT be displayed

#### Scenario: All results excluded

- **WHEN** every result returned is of an unnavigable type
- **THEN** the modal SHALL show its empty state

### Requirement: Modal styling matches the application palette

The modal body SHALL use the library-side background colour, the search input row SHALL use the playback-panel background colour, the search input border and the hero block's rules SHALL use the unplayed seek-track colour, and text SHALL use the soft-white foreground colour.

#### Scenario: Modal drawn

- **WHEN** the search modal is rendered
- **THEN** its body, input row, borders, and text SHALL use those palette colours
- **AND** the backdrop behind it SHALL be dimmed

### Requirement: Dimmed backdrops render images in halfblocks

While a dimmed backdrop is displayed, all images on that backdrop SHALL render using the halfblock image protocol, so that the dimming applies to artwork as well as to text. This SHALL apply to every overlay that dims its backdrop, not only to the search modal.

When no dimmed backdrop is displayed, images SHALL render using the user's configured image protocol.

Switching between protocols SHALL NOT discard already-rendered images of the other protocol, and SHALL NOT require refetching image data over the network.

#### Scenario: Modal opened over artwork

- **WHEN** an overlay that dims its backdrop is opened over a view containing images
- **THEN** those images SHALL render in halfblocks
- **AND** SHALL be dimmed to the same degree as the surrounding text

#### Scenario: Modal dismissed

- **WHEN** the dimming overlay is dismissed
- **THEN** images SHALL return to the user's configured image protocol

#### Scenario: Repeated open and close

- **WHEN** a dimming overlay is opened and dismissed repeatedly
- **THEN** images already rendered in each protocol SHALL be reused rather than refetched from the server

#### Scenario: Configured protocol is already halfblocks

- **WHEN** the user's configured protocol is halfblocks and a dimming overlay is opened
- **THEN** image rendering SHALL be unchanged

#### Scenario: Images disabled

- **WHEN** image rendering is disabled entirely and a dimming overlay is opened
- **THEN** no images SHALL be rendered in either state

### Requirement: Dismissing search restores the previous view

Dismissing the modal SHALL close it and return focus to whatever was focused before it opened, leaving the underlying view's navigation position unchanged.

Dismissal SHALL close the modal outright from either mode. It SHALL NOT change mode, and SHALL NOT require a second press to leave.

#### Scenario: Dismissing without activating

- **WHEN** the user dismisses the search modal without activating a result
- **THEN** the modal SHALL close
- **AND** the underlying library's navigation position and selection SHALL be unchanged from before the modal opened

#### Scenario: Dismissing from global mode

- **WHEN** the user dismisses the modal while it is in global mode
- **THEN** the modal SHALL close on that single press
- **AND** SHALL NOT revert to fuzzy mode first
