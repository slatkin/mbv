# Spec Delta

## MODIFIED Requirements

### Requirement: Audiobookshelf books are not castable

mbv SHALL report a multi-file Audiobookshelf book as uncastable and SHALL NOT load it onto a cast target. A book's position is defined across its whole timeline, and a receiver reports position only within the file it is playing, which cannot be written back as a book position without corrupting the stored resume point.

#### Scenario: Book is played to a cast target

- **WHEN** the user plays an Audiobookshelf book while a cast target is the active output
- **THEN** mbv reports the item as uncastable and does not load it
- **AND** SHALL NOT modify the book's stored position

#### Scenario: Selection mixes books and castable items

- **WHEN** the queue that a receiver plays contains both an Audiobookshelf book and castable items
- **THEN** the castable items play on the receiver and the book is reported as uncastable when the owner reaches it

### Requirement: Uncastable items are surfaced, not silently dropped

When the owner finds that the item of a slot cannot be cast, it SHALL tell the user which item and why, and SHALL NOT substitute a different item for an explicit play or fail silently. On an explicit play or jump, the owner SHALL load nothing. On an advance after a natural end, the owner SHALL report the item and step to the next slot of its play order.

#### Scenario: Item is uncastable

- **WHEN** the owner finds that an item cannot be loaded onto the active receiver
- **THEN** mbv presents a message naming the item and the reason

#### Scenario: User plays an uncastable slot

- **WHEN** the user plays or jumps to an uncastable slot while a receiver is the output
- **THEN** mbv names the item and the reason
- **AND** the receiver is given nothing new

#### Scenario: Advance reaches an uncastable slot

- **WHEN** the active item finishes on the receiver and the next slot is uncastable
- **THEN** mbv names that item and the reason
- **AND** the owner loads the next castable slot after it
