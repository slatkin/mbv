## Purpose

Defines when saved playback progress is substantial enough for mbv to resume rather than restart an item from its beginning.

## ADDED Requirements

### Requirement: Resume requires six percent progress
For media with a known runtime, mbv SHALL treat saved playback position as resumable only when it is at least 6 percent of runtime. Exactly 6 percent SHALL qualify. A positive saved position with unknown runtime SHALL remain resumable.

#### Scenario: Position is below six percent
- **WHEN** an item with known runtime has saved progress below 6 percent
- **THEN** mbv starts the item from its beginning rather than resuming

#### Scenario: Position is exactly six percent
- **WHEN** an item with known runtime has saved progress equal to 6 percent
- **THEN** mbv resumes from the saved position

#### Scenario: Runtime is unknown
- **WHEN** an item has positive saved progress and unknown runtime
- **THEN** mbv treats the position as resumable
