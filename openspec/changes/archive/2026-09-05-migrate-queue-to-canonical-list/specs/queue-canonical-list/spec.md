# Queue canonical list composition

## ADDED Requirements

### Requirement: Queue composes canonical fixed-row mechanics

The Queue Interactive Component SHALL embed `WideMediaList<QueueSlotId>` directly for Queue's fixed-height rows. Queue SHALL NOT use `InlineMediaBrowser`, Hero-on-left, Inline hero, or responsive Wide/Inline handoff. Queue SHALL NOT duplicate selectable indexing, fixed-row placement, cursor movement, scrolling, or scrollbar geometry in the parent or shell. Every slot-targeted Queue effect request SHALL identify its stable `QueueSlotId`; only reorder MAY carry a destination position, and that position SHALL be resolved against the same canonical queue.

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

### Requirement: Queue preserves the visual contract through continuous verification

Implementation, focused tests and automated gates, review, and acceptance SHALL form one uninterrupted slice without a pre-test visual-approval checkpoint. Tests SHALL cover metadata, active progress, focus, Local/Remote scope, reorder state, remote state, and stable target/geometry behavior. Live review SHALL cover supported Wide/Normal and narrow/mini widths; any defect found there SHALL be fixed as a bug and followed by rerunning the affected tests and gates.

#### Scenario: Verification proves one painter

- **WHEN** Queue is rendered at each reachable supported breakpoint
- **THEN** execution evidence shows exactly one Queue body painter
- **AND** changed source files are at most 800 lines
- **AND** the verification record identifies the canonical child and excludes an underpainting legacy path
