# Spec Delta

## ADDED Requirements

### Requirement: Two-panel layout hides an empty queue column

At 80+ columns, while the Panel mode is `both` and the displayed queue has no slots, the window SHALL render exactly as library-only: no queue column and no queue boundary, the Library playback panel shown, and the library at full width. The Panel mode SHALL stay `both`. When the displayed queue gains a slot, the two-panel layout SHALL return without user action. Queue-only and the mini-view queue panel SHALL keep the empty-queue placeholder.

#### Scenario: Empty queue renders library-only

- **WHEN** the Panel mode is `both` at 80+ columns and the displayed queue is empty
- **THEN** the library SHALL span the full window width, the Library playback panel SHALL render, and the queue column SHALL NOT render

#### Scenario: Queue column returns when the queue fills

- **WHEN** the queue column is hidden because the displayed queue is empty and a slot is added to it
- **THEN** the two-panel layout SHALL render with the queue column

#### Scenario: Cycle continues from both

- **WHEN** the queue column is hidden because the displayed queue is empty and the user presses `x`
- **THEN** the Panel mode SHALL advance to queue-only

#### Scenario: Queue-only keeps the placeholder

- **WHEN** the Panel mode is queue-only, or mini view shows the queue panel, and the displayed queue is empty
- **THEN** the queue panel SHALL render with the empty-queue placeholder

#### Scenario: Resize inactive while hidden

- **WHEN** the queue column is hidden because the displayed queue is empty
- **THEN** queue-column resizing and the Alt+Left return-to-queue key SHALL be inactive

### Requirement: Hidden empty queue moves focus to the library

When the queue column is hidden because the displayed queue is empty, panel focus SHALL be on the library. Focus SHALL stay on the library when the queue column returns.

#### Scenario: Queue focus moves when the queue empties

- **WHEN** the queue holds panel focus in the two-panel layout and its last slot is removed
- **THEN** panel focus SHALL move to the library

#### Scenario: Focus stays on the library on refill

- **WHEN** focus moved to the library because the queue emptied, and the user then adds an item to the queue
- **THEN** panel focus SHALL remain on the library
