# Spec Delta

## ADDED Requirements

### Requirement: The Queue seekbar renders playback progress in eighth-cell increments

When the Queue playback panel paints an active seekbar, it SHALL represent playback position
with solid full-height accent blocks and a left-filled fractional leading block, over a solid
`#272e33` track. Each terminal cell SHALL support eight horizontal fill increments. The rendered
fill SHALL be the clamped position/runtime fraction of the available bar width, rounded to
the nearest eighth of a cell, with half increments rounded upward. It SHALL NOT impose a
minimum visible fill on positive positions below that rounding threshold.

A non-positive runtime SHALL produce an empty fill. With a positive runtime, a non-positive
position SHALL produce an empty fill and a position at or beyond runtime SHALL produce a
full fill. The bar SHALL recompute from the current projected position and current width on
each paint, including while paused or after a backward seek, without extrapolating playback.

This presentation SHALL apply wherever the Queue playback panel's active seekbar renders,
independent of media kind or playback target. It SHALL NOT change the Library playback
panel's thin upper-line seekbar or its existing whole-cell rounding. It SHALL NOT change
transport visibility or the inactive/non-interactive bar presentation.

#### Scenario: Early episode progress is visible before a whole cell fills

- **WHEN** an active Queue seekbar is 24 cells wide and playback is at 10 seconds of a
  45-minute episode
- **THEN** the first bar cell SHALL show one eighth of accent fill over the muted track
- **AND** no whole bar cell SHALL be filled

#### Scenario: Completed cells precede the fractional leading cell

- **WHEN** the unrounded fill of the active Queue seekbar is 3.5 cells
- **THEN** three cells SHALL be fully accent-filled and the next cell SHALL be half
  accent-filled, with the remaining track muted
- **AND** no percentage label SHALL be introduced inside the bar

#### Scenario: Invalid or out-of-range progress stays bounded

- **WHEN** the Queue seekbar paints a non-positive runtime, or a non-positive position with
  a positive runtime
- **THEN** its entire bar SHALL remain the muted track without accent fill
- **WHEN** its position is at or beyond a positive runtime
- **THEN** the entire bar SHALL be accent-filled without an extra cell beyond its bounds

#### Scenario: Backward seeking and resizing use the current playback snapshot

- **WHEN** the active Queue seekbar repaints after a backward seek or a width change
- **THEN** its fill SHALL reflect the current projected position and current bar width
- **AND** no previously filled cells SHALL remain accent-filled beyond the new fill

#### Scenario: Paused playback does not animate the fractional fill

- **WHEN** the Queue seekbar repaints with the same paused position, runtime, and width
- **THEN** its fill SHALL remain unchanged

#### Scenario: Library playback preserves its existing thin bar

- **WHEN** the Library playback panel renders instead of Queue playback
- **THEN** its seekbar SHALL retain its thin upper-line characters and whole-cell fill
- **AND** it SHALL NOT adopt Queue's solid fractional-block presentation

### Requirement: Fractional Queue fill preserves the seek row and interaction boundaries

The fractional Queue seekbar SHALL keep the existing elapsed time, bar, and total time order,
one space between each time and the bar, and one column of outer indent on each side. Its
height and available width SHALL remain unchanged. Elapsed and total labels SHALL retain
their existing formatting and metadata colour; the fill SHALL retain the accent role and
the unplayed track SHALL use the Queue playback panel's slate backdrop colour (`#272e33`),
without changing the shared theme values or the Library `PROGRESS_TRACK` role.

The bar span alone SHALL remain the pointer target. A partial cell SHALL be part of that
same target and SHALL resolve the same terminal-column-based seek fraction as before;
fractional painting SHALL NOT imply sub-cell pointer precision. The time labels, padding,
and blank transport rows SHALL continue to resolve no seek intent. When no bar width
remains, the existing clipped time-only fallback SHALL remain and SHALL expose no seek
region. No border, label, or extra row SHALL be added to the bar.

#### Scenario: A partially filled cell is still an ordinary seek target

- **WHEN** the user clicks a partially filled cell of the Queue seekbar
- **THEN** the click SHALL seek to the same fraction that column resolved before
- **AND** clicks on either flanking time label SHALL NOT seek

#### Scenario: A narrow seek row has no room for a bar

- **WHEN** the available Queue seek row leaves zero cells for the bar after its labels and
  padding are accounted for
- **THEN** the row SHALL retain its existing clipped time-only presentation
- **AND** it SHALL expose no seek region
