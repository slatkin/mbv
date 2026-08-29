# interactive-component-framework Specification (delta)

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

A component's own cursor, scroll, or selection value MUST NOT be written into
a shell (`App`) field for the sole purpose of being read back by a shell-side
handler invoked immediately afterward. When a component-owned value determines
which shell-owned effect runs, the component SHALL pass that value as a
parameter of the typed `Msg` it emits, and the shell-side handler SHALL accept
it as an argument rather than re-reading it off `App`. A shell field that
exists only to close this loop is a forbidden mirror, not a sanctioned content
push: pushing validated shell-owned content (list rows, setting values, setup
drafts, and similar presentation content the shell computed) into a component
remains sanctioned and is unaffected by this rule.

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

#### Scenario: A component-owned cursor drives a shell-owned effect without a round trip

- **WHEN** the Settings/Services component's local cursor determines which
  setting or service entry the user activated
- **THEN** the component emits `SettingsIntent::Activate` or
  `ServiceRequest::ActivateService` carrying that cursor as a value
- **AND** the shell resolves the target and calls the shell-side handler with
  that resolved value directly
- **AND** no `App` field stores the cursor for the handler to read back
