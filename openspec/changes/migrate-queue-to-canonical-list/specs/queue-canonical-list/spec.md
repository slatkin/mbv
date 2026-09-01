# Queue canonical list composition

## ADDED Requirements

### Requirement: Queue composes canonical fixed-row mechanics

The Queue Interactive Component SHALL embed `WideMediaList` or an equivalent canonical fixed-row child for Queue's fixed-height rows. Queue SHALL NOT duplicate selectable indexing, fixed-row placement, cursor movement, scrolling, scrollbar geometry, or row hit testing in the parent or shell.

#### Scenario: Queue renders canonical rows

- **WHEN** Queue is visible in its supported panel mode
- **THEN** the canonical fixed-row child paints the Queue rows
- **AND** the Queue parent supplies prepared content and translates typed intents
- **AND** no legacy Queue body painter also paints that rect

#### Scenario: Queue state remains parent-owned where required

- **WHEN** the user changes Queue scope, reorders, activates, removes, or plays a slot
- **THEN** the Queue parent emits the corresponding typed request
- **AND** the shell retains Local/Remote scope, Player/queue authority, persistence, title, and playback effects
- **AND** the child does not receive a Service client, Player, persistence handle, credentials, or callbacks

### Requirement: Queue projection is bounded presentation data

Queue SHALL project selectable rows with stable opaque `QueueSlotId` targets and presentation metadata, semantic active state, and optional integer `progress_percent` clamped to `0..=100`. The projection SHALL NOT carry ticks, runtime, source preparation, credentials, callbacks, or provider effects.

#### Scenario: Active progress is safe to paint

- **WHEN** an active Queue slot has progress outside the presentation range
- **THEN** Queue clamps the projected percentage to `0..=100`
- **AND** the child paints only the bounded presentation value

#### Scenario: Refresh preserves target identity

- **WHEN** Queue content is refreshed without a navigation event
- **THEN** the child preserves its local cursor and scroll where the selected `QueueSlotId` remains present
- **AND** it clamps or resets only when the target is absent or content no longer permits the position
- **AND** the shell does not mirror the child cursor or scroll per frame

### Requirement: Queue owns the mouse seam without duplicate geometry

The mounted Queue parent SHALL own its mouse subscription and `MouseGestureState`. The embedded child SHALL populate and resolve `HitRegions<QueueSlotId>` from the geometry it paints. The parent SHALL delegate list-point resolution to the child and translate the result to a semantic Queue request; no restore-mouse-support global hit map or second coordinate path SHALL be introduced.

#### Scenario: A Queue row click resolves once

- **WHEN** the user clicks a painted Queue row
- **THEN** the child resolves the point to one `QueueSlotId`
- **AND** the parent handles gesture timing/focus and emits a typed semantic request
- **AND** the shell does not re-resolve screen coordinates

#### Scenario: Queue scope controls remain parent-owned

- **WHEN** the user clicks Local or Remote scope controls
- **THEN** the Queue parent resolves those parent-owned targets and applies existing scope rules
- **AND** the canonical child is used only for Queue row geometry

### Requirement: Queue preserves the visual contract before test changes

Implementation SHALL characterize current Queue output and behavior before visual correction. Visual correction SHALL be performed at supported Wide/Normal and narrow/mini widths, with explicit user live confirmation before adding or updating UI tests. Tests SHALL then cover metadata, active progress, focus, Local/Remote scope, reorder state, remote state, and stable target/geometry behavior.

#### Scenario: Verification proves one painter

- **WHEN** Queue is rendered at each reachable supported breakpoint
- **THEN** execution evidence shows exactly one Queue body painter
- **AND** changed source files are at most 800 lines
- **AND** the verification record identifies the canonical child and excludes an underpainting legacy path
