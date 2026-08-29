## MODIFIED Requirements

### Requirement: Interactive components own only presentation authority

An Interactive Component SHALL own its cursor, scroll, local focus, selection, filters, form drafts, local loading/error/result presentation state, local event interpretation and update rules, rendering, viewport, and render-derived hit geometry. It SHALL emit a `Msg` only for work that crosses its authority boundary — navigation, playback, Service access, persistence, mounting or dismissing another component, or changing focus.

An Interactive Component SHALL NOT receive or hold `App`, a Service client, credentials, `Config`, `PlayerProxy`, `RemotePlayer`, a protocol object, an `mpsc` channel, an `Arc<Mutex<_>>` integration lock, source URLs or headers, or arbitrary Ratatui `Color`/`Style`. It MAY receive owned presentation models containing text, durations, badges, semantic focus/selection/disabled state, image cache keys, semantic variant/policy values, and opaque action keys (for example a `QueueSlotId` the shell resolves).

A component's own cursor, scroll, or selection value MUST NOT be written into a shell (`App`) field for the sole purpose of being read back by a shell-side handler invoked immediately afterward. When a component-owned value determines which shell-owned effect runs, the component SHALL pass that value as a parameter of the typed `Msg` it emits, and the shell-side handler SHALL accept it as an argument rather than re-reading it off `App`. A shell field that exists only to close this loop is a forbidden mirror, not a sanctioned content push: pushing validated shell-owned content (list rows, setting values, setup drafts, and similar presentation content the shell computed) into a component remains sanctioned and is unaffected by this rule.

When a component-local movement also has to drive shell-owned persisted or
effectful state (pagination, position persistence, navigation-idle timers),
the component SHALL resolve the movement itself and carry the resolved value
in the `Msg`; the shell SHALL apply that value directly rather than
independently recomputing the same movement. The shell SHALL NOT write a
value the component painted (a scroll offset, a resolved cursor) back into
component-local state on every render pass; it MAY push shell-owned
navigation state (a browse level's resting cursor/scroll) into the component
only at the discrete event where the visible level changes.

A movement stride (page size, column count, or equivalent) used to resolve a
component-local movement SHALL have exactly one source. Where the component
resolves the movement, that source is the component's own painted geometry;
the shell SHALL NOT apply a second stride to the same movement.

Where a projection replaces a component's state wholesale, the component's own
interaction values SHALL take precedence over the incoming snapshot's
unconditionally. When the projected content no longer contains the item a
component-owned value referred to, the component SHALL reset that value to its
own default or clamp it against the new content; it SHALL NOT fall through to
the value carried in the shell's snapshot.

A component SHALL NOT hold echo-detection state: a field whose purpose is to
distinguish the component's own writes from values arriving in a shell
projection (for example a stored copy of the last pushed cursor, compared
against the current one to decide whether to adopt an incoming value). Such a
field is evidence of two owners. Where the shell must move a component-owned
cursor, it SHALL do so through an explicit re-anchor at the navigation event
that requires it, not by an equality test evaluated on every content push.

A type projected from the shell into an Interactive Component SHALL NOT carry
a field the component owns. Content the shell computes and interaction state
the component owns SHALL be separate types, so that a projection cannot
overwrite an interaction value and no component needs to save and restore its
own fields around one.

Where a cursor or scroll value is both interacted with and persisted, the live
value and the persisted resting position SHALL be distinct, separately named
state. The component owns the live value; the shell owns the resting position
and writes it only at a navigation event.

#### Scenario: Local interaction does not become a global message

- **WHEN** the user moves a cursor, scrolls, cycles a filter chip, or edits a local form field
- **THEN** the owning component updates its private state directly
- **AND** it emits no `Msg`

#### Scenario: Cross-boundary work is a typed request

- **WHEN** a component needs playback, navigation, Service access, or persistence
- **THEN** it emits a typed `Msg` describing the request
- **AND** the shell Model performs the effect; the component neither calls the Player/Service nor mutates the canonical queue

#### Scenario: A component-owned cursor drives a shell-owned effect without a round trip

- **WHEN** the Settings/Services component's local cursor determines which setting or service entry the user activated
- **THEN** the component emits `SettingsIntent::Activate` or `ServiceRequest::ActivateService` carrying that cursor as a value
- **AND** the shell resolves the target and calls the shell-side handler with that resolved value directly
- **AND** no `App` field stores the cursor for the handler to read back

#### Scenario: A local movement that also persists carries its resolved value once

- **WHEN** the user moves the cursor in a component whose movement also
  drives shell-owned pagination or position persistence (for example the
  Emby generic/Movies/HomeVideos browser)
- **THEN** the component updates its own cursor locally and emits a `Msg`
  carrying the resolved index it landed on
- **AND** the shell applies that index directly to its persisted state and
  runs the associated effects, without independently recomputing the
  movement

#### Scenario: Audiobookshelf show and book movement carries a resolved value

- **WHEN** the user moves the show cursor, the book cursor, the surname-bucket
  pill, or the chapter focus in an Audiobookshelf browser
- **THEN** the component resolves the movement against its own content and
  geometry and emits a `Msg` carrying the resolved index, bucket position, or
  chapter selection
- **AND** the shell applies that value through its existing index-taking entry
  point, running the position-save and detail-fetch effects unchanged
- **AND** no `App` helper recomputes the same movement from a delta

#### Scenario: Paging uses one stride

- **WHEN** the user pages a component-owned list whose movement also drives a
  shell-owned effect
- **THEN** the page stride comes from the component's painted geometry alone
- **AND** the shell does not re-page the same movement with a stride of its own

#### Scenario: A projection never reinstates a component value the shell happens to hold

- **WHEN** the shell pushes content in which the item a component-owned
  selection referred to is no longer present
- **THEN** the component resets that selection, and any scroll, filter, or
  sub-selection derived from it, to its own defaults
- **AND** the values carried in the shell's snapshot for those fields are
  discarded rather than adopted

#### Scenario: A shell re-anchor lands regardless of prior local movement

- **WHEN** the shell re-anchors a component-owned cursor at a navigation event
  (a group switch, a recursive activation, or a saved-position restore)
- **THEN** the component adopts the re-anchored value
- **AND** the outcome does not depend on whether the user moved that cursor
  since the previous projection

#### Scenario: Ordinary content pushes leave a component cursor alone

- **WHEN** the shell pushes refreshed content without a navigation event
- **THEN** the component's cursor, scroll, and local focus are unchanged
- **AND** the component holds no stored copy of a previously pushed value in
  order to reach that outcome

#### Scenario: A projected content type carries no component-owned field

- **WHEN** the shell projects browse content into a component
- **THEN** the projected type contains only content the shell computed
- **AND** the component's cursor, scroll, selection, and local filters are
  absent from it, so the component neither saves nor restores its own state
  around the projection

#### Scenario: Live cursor and resting position are distinct state

- **WHEN** the user moves the cursor on a visible browse level, and the shell
  later persists that level's position or restores it on re-entry
- **THEN** the live cursor is read from the component that owns it
- **AND** the persisted resting position is separate state the shell writes at
  the navigation event, not the same field serving both purposes

#### Scenario: Painted output is not written back into shell state every frame

- **WHEN** a component renders and produces a final scroll offset or other
  paint-derived value
- **THEN** the shell does not copy that value into its own persisted state
  as part of the render pass
- **AND** any persistence of that value happens only at the navigation event
  that actually changes which content is visible, not on every paint
