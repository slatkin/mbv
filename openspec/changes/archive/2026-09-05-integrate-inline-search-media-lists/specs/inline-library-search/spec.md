## ADDED Requirements

### Requirement: Open search survives responsive presentation transitions

An open Inline Search session SHALL remain open when the selected Emby destination changes between Normal and Wide presentation without changing destinations. The query and selected result SHALL be preserved, the same full-library corpus SHALL remain in effect, and the selected result SHALL remain visible after the results are laid out for the new presentation.

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
