## ADDED Requirements

### Requirement: A repaint copies no section content

The window SHALL read a Library list, Library detail, Queue, or Settings section without copying its rows or entries. A repaint that follows a change to some other section SHALL NOT allocate a new value for a section whose revision did not change. A section update SHALL cost one copy of that section's content, however many components read it.

#### Scenario: A repaint after another section changed keeps the Queue value

- **GIVEN** the window holds a Queue section at one revision
- **WHEN** a batch updates only the Status bar section
- **THEN** every reader of the Queue section sees the same Queue value as before the batch

#### Scenario: A Library section update copies the section once

- **GIVEN** the Library panel and its destination list both read the Library list section
- **WHEN** a batch carries a new revision of that section
- **THEN** the window holds one new value of the section
- **AND** the panel and the destination list read that one value

#### Scenario: A repeated revision writes nothing

- **WHEN** a batch carries a section at the revision that the window already holds
- **THEN** the window keeps the value it holds and notifies no reader of that section
