# Spec Delta

## MODIFIED Requirements

### Requirement: Queue composes canonical fixed-row mechanics

The Queue Interactive Component SHALL embed `WideMediaList<QueueSlotId>` directly for Queue's fixed-height rows. Queue SHALL NOT use `InlineMediaBrowser`, Wide hero, Inline hero, or responsive Wide/Inline handoff. Queue SHALL NOT duplicate selectable indexing, fixed-row placement, cursor movement, scrolling, or scrollbar geometry in the parent or shell. Every slot-targeted Queue effect request SHALL identify its stable `QueueSlotId`; only reorder MAY carry a destination position, and that position SHALL be resolved against the same canonical queue.

#### Scenario: Queue renders canonical rows

- **WHEN** Queue is visible in its supported panel mode
- **THEN** the canonical fixed-row child paints the Queue rows
- **AND** the Queue parent supplies prepared content and translates typed intents
- **AND** no legacy Queue body painter also paints that rect

#### Scenario: Queue state remains parent-owned where required

- **WHEN** the user reorders, activates, removes, or plays a slot
- **THEN** the Queue parent emits the corresponding typed request
- **AND** the shell retains the choice of shown Player owner, Player/queue authority, persistence, title, and playback effects
- **AND** the child does not receive a Service client, Player, persistence handle, credentials, or callbacks

### Requirement: Queue projection is bounded presentation data

Queue SHALL project selectable rows with stable opaque `QueueSlotId` targets and presentation metadata, semantic state distinguishing the now-playing row from resume-progress rows, and optional integer `progress_percent` clamped to `0..=100`. The projection SHALL NOT carry ticks, runtime, source preparation, credentials, callbacks, or provider effects.

The now-playing row SHALL show its total duration like every other row. It SHALL project the `NowPlaying` semantic state with live progress following the playback position; the live percentage renders inline as trailing metadata, exactly as resume progress does. When runtime is unknown the row SHALL project no `progress_percent` and show no duration. A non-active row SHALL project its stored resume position as inline trailing progress metadata, whatever the item's provider kind — feed, Audiobookshelf, or Emby — with the duration slot unchanged. The playing row's live ticks SHALL win over that stored position.

The projection's semantic active state — whether the shown Player owner is playing, which slot in its queue, and whether that slot is observed by the Player owner or is the owner's pending transition awaiting observation — SHALL be one owned value derived from the adopted owner snapshot outside the render path. The Client SHALL NOT originate a prediction of its own: a pending slot SHALL come only from the transition the owner published.

While the owner's pending transition names a different slot from the observed one, the projection SHALL report the pending slot as now-playing with no `progress_percent` until the owner observes it. Whenever the active slot is observed, `progress_percent` SHALL follow the live playback position.

`QueueRevision` SHALL be authoritative for every mutation that changes a projected row (slot add/remove/move/replace, item replacement, progress application, refresh merge); queue replacement SHALL invalidate previously issued revisions. Queue rows SHALL rebuild only when the `(shown Player owner, revision, playback.active, projected active target, progress-percentage bucket)` fingerprint changes. The no-change tick path SHALL perform no slot clone, no row projection, and no row push. A change to the progress bucket alone SHALL patch the now-playing row in place rather than rebuild every row. Cursor pushes and owner/chrome/title delivery SHALL NOT be gated by the fingerprint: they are delivered on every tick regardless of whether rows changed.

A push that forces the child to adopt a specific active index SHALL be scoped to the queue of one Player owner and SHALL be consumed only while that queue is the one on screen; a push armed for a queue that is not shown SHALL be dropped rather than move the shown queue's independent selection.

#### Scenario: Active progress is safe to paint

- **WHEN** an active Queue slot has progress outside the presentation range
- **THEN** Queue clamps the projected percentage to `0..=100`
- **AND** the child paints only the bounded presentation value

#### Scenario: Now-playing row keeps total duration

- **WHEN** a Queue slot is the now-playing row
- **THEN** the row shows its total duration in the duration slot
- **AND** live progress renders inline as trailing metadata, as on other rows
- **AND** no throbber glyph appears

#### Scenario: Unknown runtime shows neither progress nor duration

- **WHEN** the now-playing item has unknown runtime
- **THEN** the row projects no `progress_percent`
- **AND** the duration slot stays empty

#### Scenario: Refresh preserves target identity

- **WHEN** Queue content is refreshed without a navigation event
- **THEN** the child preserves its local cursor and scroll where the selected `QueueSlotId` remains present
- **AND** it clamps or resets only when the target is absent or content no longer permits the position
- **AND** the shell does not mirror the child cursor or scroll per frame

#### Scenario: Unchanged queue skips projection

- **WHEN** a shell tick finds the shown Player owner, queue revision, playback-active flag, projected active target, and progress bucket all unchanged
- **THEN** no slot list is cloned, no rows are projected, and no rows are pushed to the child
- **AND** projected row data is unchanged (the painted frame may still differ via title marquee or other non-row state)
- **AND** any pending cursor push and the current owner/chrome/title are still delivered

#### Scenario: Progress-bucket change patches one row

- **WHEN** only the now-playing progress bucket changes between ticks
- **THEN** only the now-playing row is updated
- **AND** all other rows are untouched

#### Scenario: Selecting a different item to play

- **WHEN** the user starts a different queue item and the owner has published it as its pending transition but not yet observed it
- **THEN** the projection reports the pending slot as now-playing
- **AND** it reports no progress for that slot until the owner observes it
- **AND** it never carries the previously playing item's position or runtime onto the new slot

#### Scenario: A queue edit relocates the playing item

- **WHEN** a queue edit moves or removes rows such that the still-playing item's index changes
- **THEN** the projection reports the item's new index as now-playing from the owner's answering snapshot
- **AND** it keeps that item's existing progress unchanged

#### Scenario: Paused playback keeps state without animation

- **WHEN** playback is paused while a slot is now-playing
- **THEN** the row keeps its `NowPlaying` state and progress
- **AND** projected rows carry no animation state

#### Scenario: Playback stopping clears now-playing

- **WHEN** playback becomes inactive (stop, queue exhausted, or a connect or disconnect that shows the queue of a Player owner that does not play)
- **THEN** no row carries the `NowPlaying` state
- **AND** the former row loses all now-playing decoration

#### Scenario: Empty queue or invalid active index never patches

- **WHEN** the queue is empty or the active index resolves to no slot
- **THEN** no row is patched or projected as now-playing
- **AND** the fingerprint treats the projected active target as absent

#### Scenario: Predicted selection shows no progress

- **WHEN** the owner's pending transition names a different item and the owner has not yet observed it
- **THEN** the pending slot renders as now-playing with no percentage
- **AND** its duration slot shows that item's own total duration, like every other row
- **AND** it never carries the previously playing item's position or runtime

#### Scenario: Collection rows never show now-playing

- **WHEN** a row is a navigable container
- **THEN** it never carries the `NowPlaying` state
- **AND** the painter suppresses the duration slot for containers even if one is projected

#### Scenario: A push targets a scope the user is not viewing

- **WHEN** an authoritative active-index push is armed for one Player owner's queue while the queue of a different Player owner is shown
- **THEN** the shown queue's selection is unchanged
- **AND** the push does not take effect when that queue is shown later unless it is re-armed

#### Scenario: Reconciliation does not run during paint

- **WHEN** the queue projection or playback indicator is read to render a frame
- **THEN** reading it does not change the derived active state
- **AND** the active state changes only when an owner snapshot is adopted

