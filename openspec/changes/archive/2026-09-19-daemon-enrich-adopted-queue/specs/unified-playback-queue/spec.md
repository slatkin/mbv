## ADDED Requirements

### Requirement: A Player owner refreshes progress on a cold-adopted persisted queue

When a Player owner (Local daemon or packaged `mbvd`) has no queue of its own
and adopts a client's persisted queue snapshot, it SHALL treat that
snapshot's per-item progress as provisional. It SHALL asynchronously refresh
progress for the adopted items against the owning Service without blocking
playback of the adopted queue, apply the refreshed values to its own
canonical queue (not only to a client-side snapshot), and broadcast the
refreshed queue to attached clients once applied.

#### Scenario: Cold daemon adopts a persisted queue with server-side progress

- **WHEN** a Local daemon with no existing queue receives an adoption request
  carrying a persisted snapshot whose items have resume progress on the
  owning Service
- **THEN** the daemon fetches current progress for those items from the
  Service asynchronously, merges it into its own canonical queue, and
  broadcasts the refreshed queue to attached clients — without requiring any
  item to be played first

#### Scenario: Adopted item is played before the refresh completes

- **WHEN** the user plays an item from the adopted queue before the daemon's
  asynchronous refresh has finished
- **THEN** the play-driven progress-application path is authoritative for
  that item's canonical position, and a refresh result that arrives
  afterward does not overwrite it with older data

#### Scenario: Refresh is scoped to the owning Service

- **WHEN** the adopted queue contains items from more than one Service, or
  Feed entries
- **THEN** the refresh updates or prunes only the slots belonging to the
  Service it queried, and leaves every other slot's progress untouched

#### Scenario: Refresh does not regress adopted positions

- **WHEN** the asynchronous adoption-time refresh returns UserData for an
  adopted slot whose stored position is greater than the fetched one
- **THEN** the slot keeps its greater stored position — unless the fetched
  item reports the slot as played, in which case the fetched state is adopted
  verbatim — and a fetched position greater than the stored one still updates
  the slot
