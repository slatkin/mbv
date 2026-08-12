# enqueue-feedback Specification

## Purpose
Defines user-visible feedback for queue enqueue actions so successful additions stay quiet while failures remain actionable.
## Requirements
### Requirement: Successful enqueue is silent
The system SHALL NOT display an in-app success toast or emit a desktop success notification when an enqueue action successfully adds one or more items to the visible queue. The updated queue SHALL provide the visible confirmation of the action.

#### Scenario: Library item is enqueued
- **WHEN** a user successfully enqueues a playable library item
- **THEN** the item appears in the visible queue without an enqueue success toast or desktop success notification

#### Scenario: Feed entry is enqueued
- **WHEN** a user successfully enqueues a playable FeedEntry
- **THEN** the FeedEntry appears in the visible queue without an enqueue success toast or desktop success notification

### Requirement: Enqueue failure remains visible
The system SHALL display an error toast when an enqueue action fails in a condition for which enqueue failure feedback is currently provided, and SHALL NOT leave a locally appended item in the queue when the existing rollback path applies.

#### Scenario: Player owner cannot accept an append
- **WHEN** an enqueue append cannot be sent to the applicable Player owner
- **THEN** the system displays the applicable error toast and restores the queue to its pre-enqueue contents

#### Scenario: Selected item cannot be enqueued
- **WHEN** the selected item has no playable source or no playable items can be resolved
- **THEN** the system displays the applicable error toast and does not add an item to the queue

