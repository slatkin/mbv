## ADDED Requirements

### Requirement: Inline Search results compose the shared canonical media-list owner

An open Inline Search session SHALL present its scored result rows through the shared canonical media-list owner, embedded inside the Inline Search control, rendered by the one canonical fixed-row presentation in the Library panel's list box at every breakpoint — never through a bespoke search-row renderer. The search control SHALL retain what is search-specific (query, candidate pool, scoring, debounce, open/dismiss, typed activation intents); cursor, scroll, selected stable target, viewport clamping, and retained row geometry SHALL remain in the shared canonical owner, exactly as for every other list.

Result rows SHALL be selectable `Item` rows with stable opaque targets (the matched items' ids). The search flow SHALL project no `Heading` or `Spacer` rows: its result set is flat by construction. The search bar itself SHALL remain the Library panel's selector-row chrome outside the canonical row flow, replacing the pill row exactly as before.

Selection SHALL reset to the first result whenever the scored results re-fire for a changed query. A corpus or pool refresh with an unchanged query SHALL preserve the selected stable target through the shared owner's ordinary refresh rule. A responsive presentation transition SHALL keep the session open, reuse the same shared owner, and clamp the viewport, per the shared owner's geometry-change rule.

Pointer input against search result rows SHALL flow through the panel's normalized media-list surface path like every other list: a click selects the row, a double-click activates it, a wheel gesture moves the local cursor, and a right-click resolves the row's ordinary item-based context-menu intent. The search results SHALL NOT carry a private raw-event mouse path.

#### Scenario: Search results render through the fixed-row presentation

- **WHEN** Inline Search is open with scored results in either Wide or Narrow presentation
- **THEN** the result rows SHALL paint through the same canonical fixed-row presentation as every other library list
- **AND** the selected row SHALL paint the selected-row bar while the list holds focus

#### Scenario: Query change resets the selection

- **WHEN** the scored results re-fire for a changed query
- **THEN** the canonical owner's selection SHALL reset to the first result row
- **AND** the viewport SHALL rest at the top

#### Scenario: Corpus arrival preserves the selected result

- **WHEN** the corpus fetch completes or the pool is re-projected while the query is unchanged
- **THEN** the selected stable target SHALL remain selected when it is still present in the new rows
- **AND** the selection SHALL clamp to the first valid result otherwise

#### Scenario: Presentation transition reuses the shared owner

- **WHEN** an open Inline Search session crosses a Wide/Narrow presentation transition
- **THEN** the same shared canonical owner SHALL remain active with its selection preserved and its viewport clamped
- **AND** no row-local state SHALL be copied into a second control

#### Scenario: Right-click on a search result row

- **WHEN** the user right-clicks a painted search result row
- **THEN** the row's ordinary item-based context-menu intent SHALL resolve through the standard media-list row-intent path
