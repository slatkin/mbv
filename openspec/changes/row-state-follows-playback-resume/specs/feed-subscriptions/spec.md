# Spec Delta

## MODIFIED Requirements

### Requirement: Feed rows distinguish played entries

The Feeds tab SHALL render a compact played-state indication for entries whose loaded played state is true and that Play would not resume. An entry that Play would resume from its locally recorded position SHALL show its resume state instead, as `playback-resume` defines for rows. The indication SHALL derive only from loaded entry state and SHALL NOT imply that unavailable state was checked successfully.

#### Scenario: Played entry appears in All

- **WHEN** an entry with loaded played state set to true and no resumable position is visible under the All filter
- **THEN** its row SHALL include the played-state indication

#### Scenario: Played entry with a resumable position

- **WHEN** an entry with loaded played state set to true keeps a position above 1% of its runtime
- **THEN** its row SHALL show resume state and SHALL NOT include the played-state indication

#### Scenario: Stateless entry appears

- **WHEN** no state was loaded for an entry
- **THEN** its row SHALL appear unplayed and SHALL NOT show a state-unavailable warning
