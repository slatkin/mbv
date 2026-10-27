# Spec Delta

## MODIFIED Requirements

### Requirement: Content the owner did not load is presented, not advanced

When an Emby Session presents an item that the owner did not load (neither the loaded item nor the queued next item) and the device did not begin it as the queue's end continuation of the tracked item, the owner SHALL present that item, its position, and its pause state to every Client and SHALL NOT advance, record completion, or change the active slot. Play, pause, seek, volume, mute, and track commands SHALL act on that item. Next and previous SHALL be sent to the device as its own next and previous commands. The queue's end continuation of the tracked item is adopted as the next queue slot instead.

#### Scenario: Another app starts an item on the device

- **WHEN** someone starts a different item on the device while an Emby Session is the output
- **THEN** every Client presents that item with its title and position
- **AND** the owner's active slot is unchanged

#### Scenario: Next on foreign content

- **WHEN** the user presses next while the Session plays an item the owner did not load
- **THEN** the device receives its own next command
- **AND** the owner loads nothing

## ADDED Requirements

### Requirement: The device's own continuation at the queue's end becomes the next queue slot

When the tracked item on an Emby Session ends within 10 seconds of its runtime and the device holds no queued next item, the owner SHALL adopt the next item the device starts and the owner did not load: it SHALL append that item at the end of its queue, make it the active slot, and track it. The owner SHALL adopt an item only when the Session first presents it within 30 seconds of that end, and SHALL record the ended slot's completion once.

#### Scenario: The device autoplays its next episode at the queue's end

- **WHEN** the queue's last slot plays to its end and the device starts the next episode of its own
- **THEN** the owner appends that episode as a new slot at the end of the queue
- **AND** the new slot becomes the active slot
- **AND** the ended slot records completion under the same rule as a device move to the queued next item

#### Scenario: The device autoplays after an idle gap

- **WHEN** the queue's last slot plays to its end, the Session is idle for one observation, and within 30 seconds the device starts an item of its own
- **THEN** the owner appends that item as the active slot as in the direct case
- **AND** the ended slot has recorded completion once

#### Scenario: A late foreign item is not adopted

- **WHEN** the Session first presents an item the owner did not load more than 30 seconds after the tracked item's end
- **THEN** the owner presents that item and appends nothing to the queue

#### Scenario: The item lookup fails

- **WHEN** the device starts an item that meets the adoption rule and the owner's lookup of that item fails
- **THEN** the owner presents that item, appends nothing to the queue, and logs the failure

#### Scenario: Adoption repeats

- **WHEN** an adopted item reaches its end and the device starts another item of its own
- **THEN** that item is appended to the queue and tracked the same way

#### Scenario: The adopted item is tracked like a loaded item

- **WHEN** an adopted item plays on the device
- **THEN** the owner records its position from the Session's own reports and records completion when it ends
- **AND** when the device then goes idle, the owner loads nothing and the output stays idle

#### Scenario: A stale queued item is corrected, not adopted

- **WHEN** the device starts the stale queued item after a queue edit removed it
- **THEN** the owner corrects the device as before and appends nothing to the queue

#### Scenario: Next on an adopted item follows the queue

- **WHEN** the user presses next while the device plays an adopted item that is the queue's last slot
- **THEN** the owner loads nothing and the device receives no next command
