# Spec Delta

## ADDED Requirements

### Requirement: Tree rows can carry a split title
A tree node SHALL be able to carry an optional secondary title part. A node with one SHALL paint as a flat-list split row: the primary part in the playback-context role, one space, then the secondary part in the playback-title role, replacing the depth-colour fallback. A played row SHALL mute only the secondary part. Selected-row and semantic-state roles SHALL take precedence as for flat split rows.

#### Scenario: Split node paints context and title
- **WHEN** an unselected, unplayed depth-two node has primary `Show` and secondary `Episode`
- **THEN** `Show` paints in the playback-context role and `Episode` in the playback-title role, not in the depth-two aqua fallback

#### Scenario: Played split node mutes only the title
- **WHEN** a split node is played
- **THEN** its secondary part paints in the muted/played role and its primary part keeps the playback-context role

#### Scenario: Existing destinations unchanged
- **WHEN** Grouped Music or TV nodes carry no secondary part
- **THEN** they paint exactly as before
