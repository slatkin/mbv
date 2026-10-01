# Spec Delta

## MODIFIED Requirements

### Requirement: Queue-visible layouts paint a now-playing header row

In every layout that renders the queue column, the left column SHALL paint one header row at the top
of its content, above the visual slot, on the chrome surface. The header SHALL sit in the queue
column's recessed inset: one row of column surface above it and two columns of column surface on
each side, with no padding below it, so it is not flush with the column's top, left, or right edge
and its text aligns with the slot and transport below. The header SHALL always be painted, including
while playback is idle.

While playback is idle the header SHALL state `IDLE` on the left and the playback target as
`on <host>` on the right, with the `on ` prefix muted and the hostname green when local and aqua
when remote. The target SHALL be the same value the queue title row already resolves: this machine's
device name when playback is local, and the connected session's device name (then its host) or the
direct-remote label when it is remote.

While a target plays, the header row SHALL begin with the aqua play icon (the pause icon while
paused), one space, and then either the now-playing title or the label `Now Playing`. It SHALL
carry the title — a two-part title as the context part left-aligned and the title part with the
overflow marquee on the remaining width, a one-part title as a single yellow title — unless the
queue artwork carries the title under the `queue-artwork-title-overlay` capability, in which case
it SHALL read `Now Playing`. No throbber, progress percent, or time SHALL paint in the header.

The header SHALL follow the effective playback target — cast, then connected session, then local —
and SHALL NOT follow the queue scope being viewed.

#### Scenario: Header is recessed in the queue column

- **WHEN** a queue-visible layout is painted
- **THEN** the header SHALL sit one row below the column's top edge, inset two columns from each side
  edge, with no blank row between it and the slot/transport band below

#### Scenario: Active local playback

- **WHEN** a queue-visible layout is painted while a target plays and the artwork does not carry
  the title
- **THEN** the header row SHALL read the aqua state icon followed by the now-playing title
- **AND** no throbber, progress percent, or time SHALL paint in the header row

#### Scenario: Now Playing label while the artwork carries the title

- **WHEN** a queue-visible layout is painted while the artwork carries the title
- **THEN** the header row SHALL read the aqua state icon followed by `Now Playing`

#### Scenario: Paused playback

- **WHEN** a queue-visible layout is painted while playback is paused
- **THEN** the header row SHALL show the pause icon in place of the play icon

#### Scenario: Idle

- **WHEN** a queue-visible layout is painted while no transport is active
- **THEN** the header row SHALL read `IDLE` on the left and the resolved target on the right

#### Scenario: Active watched remote playback

- **WHEN** a queue-visible layout is painted while attached to a Session that reports a
  now-playing item the viewed queue does not hold (another device's own selection)
- **THEN** the header row SHALL describe that Session's item as above
- **AND** no queue row SHALL be painted as the playhead

#### Scenario: Header follows the playback target, not the viewed queue

- **WHEN** the layout is queue-visible while attached to a remote session and the local queue scope is
  selected for viewing
- **THEN** the header SHALL describe the remote target's playback, not the local queue's

#### Scenario: No header when the queue column is hidden

- **WHEN** the layout is library-only
- **THEN** no header row SHALL be painted
