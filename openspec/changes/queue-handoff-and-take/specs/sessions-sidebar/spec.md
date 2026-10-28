# Spec Delta

## ADDED Requirements

### Requirement: F3 offers Hand off and Take on a peer target

F3 SHALL offer Hand off and Take as separate actions on the selected target when that target has a direct Player-owner endpoint other than the local Owner. Each action SHALL name the selected target by its stable identity, as activation does. F3 SHALL NOT offer them on a target without a direct endpoint; activation of such a target SHALL stay unchanged.

#### Scenario: Peer target selected

- **WHEN** the selected F3 target is an mbv or mbvd peer with a direct endpoint
- **THEN** Hand off and Take SHALL be available for it
- **AND** the footer SHALL list them

#### Scenario: Emby Session without a direct endpoint selected

- **WHEN** the selected F3 target has no direct endpoint
- **THEN** Hand off and Take SHALL NOT be available
- **AND** activation SHALL keep its current behavior

#### Scenario: Selected peer disappears before the request resolves

- **WHEN** the selected peer disappears before the shell resolves a Hand off or Take request
- **THEN** the request SHALL act on no target
