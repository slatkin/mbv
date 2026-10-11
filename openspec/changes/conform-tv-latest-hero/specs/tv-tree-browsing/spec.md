# Spec Delta

## MODIFIED Requirements

### Requirement: Episode content modes and search retain their own presentation
`Latest` and `Upcoming` SHALL remain flat lists outside the show tree; they SHALL NOT acquire show/season nesting. Their activation and hero rules are those of `tv-library-content-modes`. Inline Search SHALL remain a separate result list, without changing the show-tree's settled expansion or selection when it is opened and dismissed.

#### Scenario: Latest stays flat
- **WHEN** `Latest` or `Upcoming` is selected
- **THEN** its rows remain flat
- **AND** no show-tree hierarchy is shown for them

#### Scenario: Dismiss search
- **WHEN** the user enters and then dismisses Inline Search from a show mode
- **THEN** the prior show-tree selection and expansion are restored
