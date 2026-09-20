# inline-library-search Specification

## Purpose
Lets the user narrow the library list they are already looking at by typing a fuzzy query into a small input box above that list, so filtering a library never hides the library.
## Requirements
### Requirement: The search key opens an inline input box above the library list

Pressing the search key while a library tab is focused SHALL replace that library panel's browser pill row with a one-row Inline Search bar. The bar SHALL use the pill row's exact rectangle, background, and height.

The existing parent-background spacer below the pill row SHALL remain unchanged, and search results SHALL begin in the same content rectangle used by the normal library presentation. The search bar SHALL NOT overlay or dim the results or any other part of the view.

While Inline Search is active, the replaced pill controls SHALL NOT be painted or remain mouse-active. The active destination SHALL paint exactly one search bar and one result list; it SHALL NOT also paint a bordered search input or a second search presentation.

The search key SHALL have no effect on the home tab, which has no library list to filter.

The search key SHALL have no effect while the library panel is not the focused panel.

#### Scenario: Opening search on a library tab

- **WHEN** the user presses the search key with a library tab focused
- **THEN** a one-row Inline Search bar SHALL replace the browser pill row in its existing rectangle
- **AND** the parent-background spacer SHALL remain below the bar
- **AND** search results SHALL begin where the normal library content begins

#### Scenario: Pill controls while search is active

- **WHEN** Inline Search is active on a library panel
- **THEN** the panel's pill controls SHALL NOT be painted
- **AND** the pill controls SHALL NOT respond to mouse input

#### Scenario: One search presentation

- **WHEN** Inline Search is active in either Normal or Wide presentation
- **THEN** exactly one one-row search bar and one result list SHALL be painted by the active destination
- **AND** no bordered or duplicate search input SHALL be painted above the results

#### Scenario: Library list too short for the box

- **WHEN** the normal library-content rectangle has no rows available for results
- **THEN** the one-row search bar SHALL remain in the existing pill rectangle

#### Scenario: Search key on the home tab

- **WHEN** the user presses the search key on the home tab
- **THEN** nothing SHALL happen and no search bar SHALL appear

#### Scenario: Library panel is not focused

- **WHEN** the user presses the search key while the library panel is not focused
- **THEN** nothing SHALL happen and the pill row SHALL remain unchanged

### Requirement: Typing edits the query and re-filters the list in place

While the input box is open, printable characters SHALL be appended to the query and SHALL NOT be interpreted as library list shortcuts. Each change to the query SHALL re-score the active destination's corpus after a 300 ms debounce; typed characters SHALL appear immediately.

For destinations other than Grouped Music, matching SHALL apply the shared word-local fuzzy rule: every word of the query SHALL match inside a single word of the item's display name, in order, with no letter taken from a different word; the fuzzy matcher's scoring over those accepted word matches SHALL be retained. Results SHALL remain ordered by descending match score, an empty query SHALL show no results, and scored results SHALL reset selection to the first result.

For Grouped Music, matching SHALL operate on the current settled tree as specified by `grouped-music-tree-browser`: every node SHALL match under the same word-local rule against only its own searchable text — artist roots their name, album leaves their album title and year, track items their track title; score SHALL be only a visibility predicate; settled order SHALL be preserved; a match SHALL keep exactly the ancestors needed to reach it; and an empty query SHALL show the complete tree. A Grouped Music query change SHALL preserve the selected tree node when it remains visible and otherwise select a valid visible node.

#### Scenario: Typing a query
- **WHEN** the user types characters into the open search box on another searchable destination
- **THEN** the characters appear immediately
- **AND** after the debounce the list shows fuzzy matches ordered by descending score

#### Scenario: Typing a Grouped Music query
- **WHEN** the user types characters into the open search box on Grouped Music
- **THEN** the characters appear immediately
- **AND** after the debounce the tree hides nonmatching album leaves while preserving settled order and matching artist ancestors

#### Scenario: A list shortcut letter is typed
- **WHEN** the user types a character that is otherwise a library list shortcut
- **THEN** it is inserted into the query
- **AND** the library list action bound to that character does not run

#### Scenario: Opening search with an empty query
- **WHEN** the user opens Inline Search outside Grouped Music and has not typed anything
- **THEN** the result list is empty and no corpus load starts

#### Scenario: Opening Grouped Music filtering with an empty query
- **WHEN** the user opens Inline Search on Grouped Music and has not typed anything
- **THEN** the complete settled tree remains visible with no loading indicator

#### Scenario: Query emptied by deletion
- **WHEN** the user deletes back to an empty query without dismissing search
- **THEN** a non-Music flat result list is empty, while Grouped Music shows its complete settled tree
- **AND** no loading indicator is shown


### Requirement: The corpus spans the whole library, not the visible page

Outside Grouped Music, the corpus SHALL be the library's full item set, independent of lazy pagination and any active letter-range filter. Opening search SHALL NOT start the full-library fetch; the fetch SHALL start with the first query character, and once it is running the input box SHALL show a loading indicator until it completes. A non-Grouped-Music library configured for recursive album search SHALL use its album index as the corpus, matching against each album's indexed search text rather than its bare display name.

Grouped Music SHALL instead use only its current settled tree as the corpus. Opening or editing that filter SHALL NOT start a full-library fetch, bypass the active music-group pill, or include albums outside the settled snapshot.

#### Scenario: Only part of the library has been paged in
- **WHEN** the user types the first query character outside Grouped Music on a library whose items are only partly loaded
- **THEN** the full item set is fetched
- **AND** the input box shows a loading indicator until the fetch completes

#### Scenario: A letter-range filter is active
- **WHEN** the user opens search while a letter-range filter narrows a non-Music library view
- **THEN** the corpus spans the entire library, not the filtered range

#### Scenario: Grouped Music uses its settled snapshot
- **WHEN** the user filters Grouped Music while only one music-group pill's settled tree is active
- **THEN** matching considers only albums in that settled tree
- **AND** no full-library or recursive album-index request starts

#### Scenario: Corpus still loading
- **WHEN** a query changes outside Grouped Music while its full-library corpus fetch is still in flight
- **THEN** the view indicates loading rather than presenting an empty result set as final


### Requirement: Results render as a flat list on every library type

Outside Grouped Music, results SHALL render through the same fixed-row canonical media-list control used by library lists, not a bespoke search renderer, so selection styling, zebra striping, and theme roles match the media-list default. No grouping applied by the underlying browse view SHALL be applied to those results, and no `Heading` or `Spacer` rows SHALL be projected. Results SHALL NOT be reordered, mismatched, or omitted as a consequence of browse-view grouping; every displayed row SHALL show the item that matched the query. Those provider-neutral result rows SHALL use stable opaque item targets, the matched display label, and a trailing release year on playable leaves when known; they SHALL be ordered by match score and SHALL paint the Library panel's loading or empty placeholders as appropriate.

Grouped Music SHALL retain and filter its shallow artist/album tree in the same Library panel browser slot. It SHALL not create a flat search-result owner, duplicate input, or second painter. Tree filtering SHALL use the visual and ownership contracts in `grouped-music-tree-browser`.

#### Scenario: Searching a grouped music library
- **WHEN** the user filters a Grouped Music tree
- **THEN** matching artist roots and album leaves remain in the tree in settled order
- **AND** no flat result list or duplicate browser is painted

#### Scenario: Searching a letter-grouped library
- **WHEN** the user searches another library whose browse view groups items under letter headers
- **THEN** results render as a flat list with no letter headers

#### Scenario: Selected search results use the canonical selected-row bar
- **WHEN** search results or a filtered Grouped Music tree are painted with the Library panel focused
- **THEN** the selected flat result row paints the canonical selected-row bar, while the selected tree node paints the tree's own focused-node treatment
- **AND** an unfocused result control paints no cursor bar

#### Scenario: Empty results while the corpus loads
- **WHEN** a query is typed outside Grouped Music while the corpus fetch is still in flight and no result rows are resolvable
- **THEN** the results area paints a loading placeholder rather than a final empty state

#### Scenario: Query matches nothing
- **WHEN** the current query has no matches
- **THEN** the results area paints the empty placeholder and search stays open

#### Scenario: Search dismissed on a grouped library
- **WHEN** the user dismisses search on any grouped library
- **THEN** its ordinary grouped presentation returns with its pre-filter navigation state restored where possible


### Requirement: Results are navigable and activatable without leaving search

While the input box is open, Up and Down SHALL move the active result selection, page keys SHALL move it by the active control's viewport semantics, and Home and End SHALL jump to its first and last visible selectable target. Cursor movement SHALL NOT alter the query, and typing SHALL NOT alter the cursor except where a changed result projection invalidates the selected target.

Outside Grouped Music, result rows SHALL retain ordinary context-menu and Ctrl+P, Ctrl+S, and Ctrl+A actions. Pressing Enter on an album result SHALL dismiss Inline Search, restore the standard library presentation, focus that album at its ordinary natural position, and enable its track-selection mode. Pressing Enter on a Series result SHALL dismiss Inline Search, navigate to the Series' natural place, and open its Workspace in Wide presentation or Library Hero overlay in Narrow presentation. Other results SHALL retain existing activation behavior.

In Grouped Music, keyboard and pointer actions SHALL remain tree actions: artist roots expand, collapse, and resolve visible descendants; album leaves retain album actions. Filtering SHALL remain open until explicitly dismissed or an existing album activation requires dismissal.

#### Scenario: Moving through results
- **WHEN** the user presses Down with flat results showing
- **THEN** selection moves to the next result and the query is unchanged

#### Scenario: Moving through a filtered tree
- **WHEN** the user presses Down with the Grouped Music filter open
- **THEN** focus moves to the next visible artist root or album leaf and the query is unchanged

#### Scenario: Result row actions after search-bar mouse-down
- **WHEN** the user presses the mouse in the Inline Search bar and then targets a flat result row
- **THEN** that row's context-menu actions and Ctrl+P, Ctrl+S, and Ctrl+A actions remain available

#### Scenario: Activating a result
- **WHEN** the user presses the activation key on a selected non-album flat result
- **THEN** the application acts on that item as it would from the unfiltered library list

#### Scenario: Enter on a filtered artist root
- **WHEN** the user presses Enter on an artist root while Grouped Music filtering is active
- **THEN** that root toggles its filter-session expansion
- **AND** the query remains open and unchanged

#### Scenario: Enter on a filtered Grouped Music album
- **WHEN** the user presses Enter on an album leaf while Grouped Music filtering is active
- **THEN** filtering closes and the unfiltered tree focuses that album
- **AND** album-track selection is enabled

#### Scenario: Enter on an album result
- **WHEN** the user presses Enter on a selected flat album result
- **THEN** Inline Search closes, the standard library presentation focuses that album, and track-selection mode is enabled

#### Scenario: Enter on a Series result
- **WHEN** the user presses Enter on a selected Series result
- **THEN** Inline Search closes and the library list rests on that Series at its natural position
- **AND** its Workspace opens in Wide presentation or the Library Hero overlay opens in Narrow presentation

#### Scenario: Navigating an empty result set
- **WHEN** the query matches nothing and the user presses Up or Down
- **THEN** nothing happens and search stays open


### Requirement: Open search survives responsive presentation transitions

An open Inline Search session SHALL remain open when the selected Emby destination changes between Normal and Wide presentation without changing destinations. The query and selected result SHALL be preserved and remain visible after the active control is laid out for the new presentation.

Outside Grouped Music, the same full-library corpus SHALL remain in effect. In Grouped Music, the same settled-tree corpus, selected visible node, and filter-forced expansion SHALL remain in effect through the transition; the single retained tree owner SHALL clamp its viewport to the new geometry without turning filter-forced expansion into persistent expansion.

The input box and results SHALL move with the destination's library list; they SHALL NOT remain painted over the pane or area used by the previous presentation.

#### Scenario: TV search transitions from Normal to Wide

- **WHEN** Inline Search is open on a TV library and a resize changes the destination from Normal presentation to Wide presentation
- **THEN** Inline Search SHALL remain open in the Wide library-list pane
- **AND** its query and selected result SHALL be unchanged
- **AND** the selected result SHALL remain visible

#### Scenario: TV search transitions from Wide to Normal

- **WHEN** Inline Search is open on a TV library and a resize changes the destination from Wide presentation to Normal presentation
- **THEN** Inline Search SHALL remain open above the Normal library list
- **AND** its query and selected result SHALL be unchanged
- **AND** the selected result SHALL remain visible

#### Scenario: Search input follows its destination list

- **WHEN** an open Inline Search session crosses a responsive presentation transition
- **THEN** exactly one search input and one result list SHALL be painted in the current library-list area
- **AND** no search content SHALL be painted in the prior presentation's area

#### Scenario: Grouped Music filter survives a responsive transition

- **WHEN** an open Grouped Music filter crosses a responsive presentation transition
- **THEN** its query, settled-tree corpus, selected visible node, and filter-forced expansion SHALL be unchanged
- **AND** the same tree owner SHALL clamp its viewport to keep the selected node visible in the new geometry


### Requirement: Dismissing search restores the unfiltered list

Pressing the dismiss key SHALL close the input box and restore the destination's unfiltered presentation and prior navigation position. Pressing the delete key on an already-empty query SHALL dismiss the search the same way. Dismissal SHALL discard the query and transient filter projection; reopening search SHALL start from an empty query.

For Grouped Music, dismissal SHALL restore persistent artist expansion and the pre-filter selected stable node when it still exists. Filter-forced expansion SHALL NOT become persistent expansion.

#### Scenario: Dismissing with the dismiss key
- **WHEN** the user presses the dismiss key while the search box is open
- **THEN** the box closes and the destination shows its unfiltered contents in ordinary order and grouping

#### Scenario: Grouped Music restores its anchor
- **WHEN** the user dismisses a Grouped Music filter after matching paths were force-expanded
- **THEN** persistent artist expansion is restored
- **AND** the pre-filter selected node is restored when still present

#### Scenario: Deleting past the start of the query
- **WHEN** the query is empty and the user presses the delete key
- **THEN** search is dismissed

#### Scenario: Reopening after dismissal
- **WHEN** the user dismisses search and immediately reopens it
- **THEN** the query is empty

