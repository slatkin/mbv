# queue-canonical-list Specification

## Purpose
Defines Queue's canonical fixed-row presentation, bounded progress projection, and parent-owned playback and scope authority.

## Requirements

### Requirement: Queue composes canonical fixed-row mechanics

The Queue Interactive Component SHALL embed `WideMediaList<QueueSlotId>` directly for Queue's fixed-height rows. Queue SHALL NOT use `InlineMediaBrowser`, Wide hero, Inline hero, or responsive Wide/Inline handoff. Queue SHALL NOT duplicate selectable indexing, fixed-row placement, cursor movement, scrolling, or scrollbar geometry in the parent or shell. Every slot-targeted Queue effect request SHALL identify its stable `QueueSlotId`; only reorder MAY carry a destination position, and that position SHALL be resolved against the same canonical queue.

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

Queue SHALL project selectable rows with stable opaque `QueueSlotId` targets and presentation metadata, semantic state distinguishing the now-playing row from resume-progress rows, and optional integer `progress_percent` clamped to `0..=100`. The projection SHALL NOT carry ticks, runtime, source preparation, credentials, callbacks, or provider effects.

The now-playing row SHALL show no elapsed time: its duration slot is empty. It SHALL project the `NowPlaying` semantic state with live progress following the playback position; the live percentage renders in the right-aligned slot, replacing the duration entirely, preceded by the current block-ramp throbber glyph. When runtime is unknown the row SHALL project no `progress_percent` and show the throbber alone. Non-active rows SHALL keep progress where it is today (inline trailing badge) with the duration slot unchanged.

The projection's semantic active state — which queue scope is playing, which slot within it, and whether that slot is confirmed by the playback owner or is an optimistic prediction awaiting confirmation — SHALL be one owned value, reconciled against playback-owner status in one place outside the render path. An optimistic prediction SHALL record why it is optimistic: a queue edit relocated the still-playing item, or a different item was selected to play. Reconciliation SHALL clear a prediction once the owner's reported slot and queue length match it.

While a prediction says a different item was selected, the projection SHALL report that item's slot as now-playing with no `progress_percent` until reconciliation confirms it. While a prediction says the still-playing item was relocated, and whenever the active slot is confirmed, `progress_percent` SHALL follow the live playback position.

`QueueRevision` SHALL be authoritative for every mutation that changes a projected row (slot add/remove/move/replace, item replacement, progress application, refresh merge); queue replacement SHALL invalidate previously issued revisions. Queue rows SHALL rebuild only when the `(viewed_scope, revision, playback.active, projected active target, progress-percentage bucket)` fingerprint changes. The no-change tick path SHALL perform no slot clone, no row projection, and no row push. A change to the progress bucket alone SHALL patch the now-playing row in place rather than rebuild every row. Cursor pushes and scope/chrome/title delivery SHALL NOT be gated by the fingerprint: they are delivered on every tick regardless of whether rows changed.

A push that forces the child to adopt a specific active index SHALL be scoped to one queue scope and SHALL be consumed only while that scope is the one on screen; a push armed for a scope the user is not viewing SHALL be dropped rather than move the viewed scope's independent selection.

#### Scenario: Active progress is safe to paint

- **WHEN** an active Queue slot has progress outside the presentation range
- **THEN** Queue clamps the projected percentage to `0..=100`
- **AND** the child paints only the bounded presentation value

#### Scenario: Now-playing row drops elapsed

- **WHEN** a Queue slot is the now-playing row
- **THEN** the row shows no elapsed time and an empty duration slot
- **AND** live progress renders right-aligned with the throbber glyph, replacing the duration
- **AND** every other row keeps its existing progress placement and duration

#### Scenario: Unknown runtime shows throbber only

- **WHEN** the now-playing item has unknown runtime
- **THEN** the row projects no `progress_percent`
- **AND** the right-aligned slot shows the throbber glyph alone

#### Scenario: Refresh preserves target identity

- **WHEN** Queue content is refreshed without a navigation event
- **THEN** the child preserves its local cursor and scroll where the selected `QueueSlotId` remains present
- **AND** it clamps or resets only when the target is absent or content no longer permits the position
- **AND** the shell does not mirror the child cursor or scroll per frame

#### Scenario: Unchanged queue skips projection

- **WHEN** a shell tick finds the viewed scope, queue revision, playback-active flag, projected active target, and progress bucket all unchanged
- **THEN** no slot list is cloned, no rows are projected, and no rows are pushed to the child
- **AND** projected row data is unchanged (the painted frame may still differ via paint-time throbber animation or other non-row state)
- **AND** any pending cursor push and the current scope/chrome/title are still delivered

#### Scenario: Progress-bucket change patches one row

- **WHEN** only the now-playing progress bucket changes between ticks
- **THEN** only the now-playing row is updated
- **AND** all other rows are untouched

#### Scenario: Selecting a different item to play

- **WHEN** the user starts a different queue item and the playback owner has not yet reported the change
- **THEN** the projection reports the newly selected slot as now-playing
- **AND** it reports no progress for that slot until the owner confirms the change
- **AND** it never carries the previously playing item's position or runtime onto the new slot

#### Scenario: A queue edit relocates the playing item

- **WHEN** a queue edit moves or removes rows such that the still-playing item's index changes, before the playback owner reports the new index
- **THEN** the projection reports the item's new index as now-playing
- **AND** it keeps that item's existing progress unchanged

#### Scenario: Paused playback freezes the throbber

- **WHEN** playback is paused while a slot is now-playing
- **THEN** the row keeps its `NowPlaying` state and progress
- **AND** the paint-time glyph holds a static frame (no animation) without touching projected rows

#### Scenario: Playback stopping clears now-playing

- **WHEN** playback becomes inactive (stop, queue exhausted, or scope switch away from the playing scope)
- **THEN** no row carries the `NowPlaying` state
- **AND** the throbber glyph is not painted

#### Scenario: Empty queue or invalid active index never patches

- **WHEN** the queue is empty or the active index resolves to no slot
- **THEN** no row is patched or projected as now-playing
- **AND** the fingerprint treats the projected active target as absent

#### Scenario: Predicted selection shows throbber without progress

- **WHEN** a prediction says a different item was selected and the owner has not yet confirmed it
- **THEN** the newly selected slot renders as now-playing with the throbber glyph and no percentage
- **AND** it never carries the previously playing item's position or runtime

#### Scenario: Collection rows never show now-playing

- **WHEN** a row is a navigable container
- **THEN** it never carries the `NowPlaying` state
- **AND** the painter suppresses the throbber/progress slot even if one were projected

#### Scenario: A push targets a scope the user is not viewing

- **WHEN** an authoritative active-index push is armed for one queue scope while the user is viewing a different scope
- **THEN** the viewed scope's selection is unchanged
- **AND** the push does not take effect when the user later switches to its scope unless it is re-armed

#### Scenario: Reconciliation does not run during paint

- **WHEN** the queue projection or playback indicator is read to render a frame
- **THEN** reading it does not consume or clear any pending prediction
- **AND** predictions are cleared only by the single reconciliation step that runs against playback-owner status

### Requirement: Queue preserves the visual contract through continuous verification

Implementation, focused tests and automated gates, review, and acceptance SHALL form one uninterrupted slice without a pre-test visual-approval checkpoint. Tests SHALL cover metadata, active progress, focus, Local/Remote scope, reorder state, remote state, and stable target/geometry behavior. Live review SHALL cover supported Wide/Normal and narrow/mini widths; any defect found there SHALL be fixed as a bug and followed by rerunning the affected tests and gates.

#### Scenario: Verification proves one painter

- **WHEN** Queue is rendered at each reachable supported breakpoint
- **THEN** execution evidence shows exactly one Queue body painter
- **AND** changed source files are at most 800 lines
- **AND** the verification record identifies the canonical child and excludes an underpainting legacy path
