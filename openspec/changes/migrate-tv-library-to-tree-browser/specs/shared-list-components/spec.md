# Spec Delta

## ADDED Requirements

### Requirement: TreeBrowser supports optional structural group headings
A nested browser SHALL support optional group headings among its root rows. Each heading SHALL occupy a painted row and participate in scrolling and grouping, but SHALL carry no selectable media target and SHALL never receive cursor focus, expansion, marks, activation, context intent, or pointer selection. Movement and paging SHALL skip headings; their presence SHALL not make a destination without headings display any. A heading SHALL remain associated with its following group when the browser's content is refreshed or its geometry changes.

#### Scenario: Grouping is visible but not interactive
- **WHEN** a destination supplies a heading followed by show nodes
- **THEN** the heading paints before those shows and occupies one row in the scrollable flow
- **AND** keyboard movement and pointer gestures can select the shows but cannot select, expand, mark, activate, or open a menu on the heading

#### Scenario: Ungrouped tree is unchanged
- **WHEN** a destination supplies no headings
- **THEN** its tree has no additional rows and retains its existing navigation, marks, painting, and hit behavior

#### Scenario: Refresh preserves stable selection across headings
- **WHEN** grouped content is refreshed or reordered while the selected media target remains present
- **THEN** that target remains selected and its viewport remains valid even if a heading appears or disappears
