## MODIFIED Requirements

### Requirement: A feed entry carries only identity and playback fields

A feed entry in the queue SHALL carry the fields needed to identify, address,
and play it: a stable entry identifier, stable feed identity, title, enclosure
URL when present, link URL when present, MIME type when present, duration in
ticks when known, playback position, and played state. The feed identity and
playback fields SHALL survive queue persistence and supported ctrl transport.
Legacy serialized feed entries that lack the added identity or playback fields
SHALL continue to load with unavailable feed identity, zero position, and
unplayed state.

#### Scenario: Entry with an enclosure

- **WHEN** a feed entry has an enclosure URL
- **THEN** that URL SHALL be available as the entry's primary playable source

#### Scenario: Entry without an enclosure

- **WHEN** a feed entry has no enclosure URL but has a link
- **THEN** the link SHALL be available as the fallback playable source

#### Scenario: Entry carries stored progress

- **WHEN** stored state is available for a feed entry
- **THEN** the queued entry SHALL carry that position and played state with its
  stable feed and entry identities

#### Scenario: Legacy entry lacks progress fields

- **WHEN** a persisted or transported feed entry predates feed playback state
- **THEN** it SHALL load as unplayed at position zero and SHALL remain playable

## ADDED Requirements

### Requirement: Feed playback reads stored state before starting

Before starting an addressable feed entry, mbv SHALL read its state from the
feed-entry store and apply the shared resume rule. A qualifying position SHALL
start at the saved position; a non-qualifying position or played entry with a
zero position SHALL start from the beginning.

#### Scenario: Feed entry has qualifying saved progress

- **WHEN** a feed entry has stored progress that qualifies under the shared
  resume rule
- **THEN** playback SHALL begin from the stored position

#### Scenario: Feed entry has only trivial progress

- **WHEN** a known-runtime feed entry has stored progress below 6%
- **THEN** playback SHALL begin from the start

#### Scenario: Stored feed entry is played

- **WHEN** a feed entry's stored state is played with position zero
- **THEN** replaying it SHALL begin from the start

### Requirement: Feed progress is persisted only on playback lifecycle events

mbv SHALL write a feed entry's current position and played state on stop,
pause, confirmed seek completion, and EOF. It SHALL NOT perform periodic or
time-tick feed-state writes.

#### Scenario: Feed playback stops before completion

- **WHEN** a feed entry stops below 95% of a known runtime
- **THEN** mbv SHALL store its current position with played set to false

#### Scenario: Feed playback pauses

- **WHEN** feed playback enters the paused state
- **THEN** mbv SHALL store the current position and current played state once
  for that pause event

#### Scenario: Feed seek completes

- **WHEN** a seek during feed playback reaches its confirmed destination
- **THEN** mbv SHALL store the resulting position and current played state

#### Scenario: No lifecycle event occurs

- **WHEN** feed playback advances normally without stop, pause, seek completion,
  or EOF
- **THEN** mbv SHALL NOT write feed state merely because time advanced

### Requirement: Feed completion uses known runtime and a 95 percent boundary

A feed entry with known runtime SHALL be marked played when it reaches EOF or
when it stops at or beyond 95% of runtime. Marking an entry played SHALL store
position zero. An entry with unknown runtime SHALL NOT be marked played solely
from EOF or a percentage calculation that cannot be made.

#### Scenario: Known-runtime entry reaches EOF

- **WHEN** a feed entry with known runtime reaches EOF
- **THEN** mbv SHALL store played as true and position as zero

#### Scenario: Entry stops exactly at 95 percent

- **WHEN** a feed entry with known runtime stops at exactly 95% of runtime
- **THEN** mbv SHALL store played as true and position as zero

#### Scenario: Entry stops below 95 percent

- **WHEN** a feed entry with known runtime stops below 95% of runtime
- **THEN** mbv SHALL store played as false with its current position

#### Scenario: Unknown-runtime entry reaches EOF

- **WHEN** a feed entry without known runtime reaches EOF
- **THEN** mbv SHALL NOT infer played state from completion alone

### Requirement: Feed state is optional and never reported to Emby

When the feed-entry store or stable feed identity is unavailable, state reads
and writes SHALL degrade to no-ops and feed playback SHALL remain available from
the beginning. Feed progress SHALL NOT be sent to the Emby playback-reporting
API and SHALL NOT create an Emby Session.

#### Scenario: Store is unavailable

- **WHEN** a feed entry is played without an available feed-entry store
- **THEN** playback SHALL proceed from the beginning without a state read or
  write failure

#### Scenario: Legacy entry has no feed identity

- **WHEN** a legacy queued feed entry cannot address the keyed store
- **THEN** playback SHALL proceed statelessly

#### Scenario: Feed lifecycle event occurs

- **WHEN** mbv records a feed stop, pause, seek completion, or EOF
- **THEN** it SHALL use only the feed-entry state path and SHALL NOT report that
  progress to Emby
