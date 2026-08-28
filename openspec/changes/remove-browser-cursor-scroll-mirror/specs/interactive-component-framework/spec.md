## MODIFIED Requirements

### Requirement: Interactive components own only presentation authority

An Interactive Component SHALL own its cursor, scroll, local focus, selection,
filters, form drafts, local loading/error/result presentation state, local event
interpretation and update rules, rendering, viewport, and render-derived hit
geometry. It SHALL emit a `Msg` only for work that crosses its authority
boundary — navigation, playback, Service access, persistence, mounting or
dismissing another component, or changing focus.

An Interactive Component SHALL NOT receive or hold `App`, a Service client,
credentials, `Config`, `PlayerProxy`, `RemotePlayer`, a protocol object, an
`mpsc` channel, an `Arc<Mutex<_>>` integration lock, source URLs or headers, or
arbitrary Ratatui `Color`/`Style`. It MAY receive owned presentation models
containing text, durations, badges, semantic focus/selection/disabled state,
image cache keys, semantic variant/policy values, and opaque action keys (for
example a `QueueSlotId` the shell resolves).

When a component-local movement also has to drive shell-owned persisted or
effectful state (pagination, position persistence, navigation-idle timers),
the component SHALL resolve the movement itself and carry the resolved value
in the `Msg`; the shell SHALL apply that value directly rather than
independently recomputing the same movement. The shell SHALL NOT write a
value the component painted (a scroll offset, a resolved cursor) back into
component-local state on every render pass; it MAY push shell-owned
navigation state (a browse level's resting cursor/scroll) into the component
only at the discrete event where the visible level changes.

#### Scenario: Local interaction does not become a global message

- **WHEN** the user moves a cursor, scrolls, cycles a filter chip, or edits a
  local form field
- **THEN** the owning component updates its private state directly
- **AND** it emits no `Msg`

#### Scenario: Cross-boundary work is a typed request

- **WHEN** a component needs playback, navigation, Service access, or persistence
- **THEN** it emits a typed `Msg` describing the request
- **AND** the shell Model performs the effect; the component neither calls the
  Player/Service nor mutates the canonical queue

#### Scenario: A local movement that also persists carries its resolved value once

- **WHEN** the user moves the cursor in a component whose movement also
  drives shell-owned pagination or position persistence (for example the
  Emby generic/Movies/HomeVideos browser)
- **THEN** the component updates its own cursor locally and emits a `Msg`
  carrying the resolved index it landed on
- **AND** the shell applies that index directly to its persisted state and
  runs the associated effects, without independently recomputing the
  movement

#### Scenario: Painted output is not written back into shell state every frame

- **WHEN** a component renders and produces a final scroll offset or other
  paint-derived value
- **THEN** the shell does not copy that value into its own persisted state
  as part of the render pass
- **AND** any persistence of that value happens only at the navigation event
  that actually changes which content is visible, not on every paint
