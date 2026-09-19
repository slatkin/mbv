## ADDED Requirements

### Requirement: Grouped music levels omit folders without contents

When a grouped music view lists a level's children, a folder item with no
child items SHALL NOT be enumerated as a row of that level. Items whose
child count is unknown SHALL be enumerated, and non-folder items SHALL
never be treated as empty. Background artist warm-up SHALL NOT spend
resolution work on folders that are not enumerated.

#### Scenario: Empty folder is not enumerated

- **WHEN** a grouped music level is listed and its children include a
  folder with no child items (for example a `Downloads` folder at the
  music library root)
- **THEN** that folder does not appear as a row of the level

#### Scenario: Unknown child count still enumerates

- **WHEN** a grouped music level is listed and a folder item's payload
  does not carry a child count
- **THEN** that folder is enumerated like any other folder

#### Scenario: Non-folder items are never treated as empty

- **WHEN** a grouped music level is listed and a non-folder item's payload
  carries a child count of zero
- **THEN** that item is still enumerated

#### Scenario: Warm-up skips empty folders

- **WHEN** background grouping-artist warm-up lists a music library's
  group-level children
- **THEN** folders with no child items receive no grouping-artist
  resolution work
