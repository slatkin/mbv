## MODIFIED Requirements

### Requirement: Results render as a flat list on every library type

While search is open, results SHALL render as a single flat list through the shared canonical media-list presentation (the same fixed-row list control every library list uses), so selection styling, zebra striping, and theme roles match the media-list default rather than a bespoke search renderer. No grouping applied by the underlying browse view — artist-grouped album headers or letter headers — SHALL be applied to search results, and no `Heading` or `Spacer` rows SHALL be projected into the search result flow.

Results SHALL be provider-neutral rows addressed by stable opaque targets (the matched items' ids), with each row's primary label the item's display label (an indexed album's display label, not its bare name) and a trailing release year on playable leaves when known — the same row content the search previously rendered, restyled by the canonical painter.

Results SHALL NOT be reordered, mismatched, or omitted as a consequence of any grouping the browse view would otherwise apply. Every displayed row SHALL show the item that actually matched the query.

When the result set is empty, the results area SHALL paint the same placeholder states the Library panel paints for an empty list box: a loading placeholder while the corpus fetch is outstanding and an empty placeholder once the query is settled with no matches.

#### Scenario: Searching a grouped music library

- **WHEN** the user searches a music library whose browse view groups albums under artist headers
- **THEN** every album matching the query SHALL appear, ordered by match score
- **AND** each row SHALL display the album it matched, not a different album
- **AND** no artist headers SHALL be drawn

#### Scenario: Searching a letter-grouped library

- **WHEN** the user searches a library whose browse view groups items under letter headers
- **THEN** results SHALL render as a flat list with no letter headers

#### Scenario: Selected search results use the canonical selected-row bar

- **WHEN** search results are painted with the Library panel focused
- **THEN** the selected result row SHALL paint the canonical selected-row bar across its full row width, and unfocused search results SHALL paint the bar nowhere
- **AND** result rows SHALL zebra-stripe like every other library list

#### Scenario: Empty results while the corpus loads

- **WHEN** a query is typed while the corpus fetch is still in flight and no result rows are resolvable yet
- **THEN** the results area SHALL paint a loading placeholder rather than an empty result set presented as final

#### Scenario: Query matches nothing

- **WHEN** the scored results for the current query are empty
- **THEN** the results area SHALL paint the empty placeholder
- **AND** the search SHALL stay open

#### Scenario: Search dismissed on a grouped library

- **WHEN** the user dismisses search on a library whose browse view groups its items
- **THEN** the grouped presentation SHALL return unchanged
