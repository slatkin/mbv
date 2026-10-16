# Spec Delta

## MODIFIED Requirements

### Requirement: Playback controls respond to pointer input wherever the panel is painted

The playback panel's transport glyphs and seekbar SHALL resolve pointer input in every layout where
the panel renders, using the geometry of the panel that was actually painted in the current frame:
the queue column panel in queue-visible layouts and the right-column strip otherwise. In the queue
column's band the transport glyphs resolve on the controls row and the seekbar on the seekbar row's
bar span; the blank title row, the blank gap row, and the time labels resolve nothing. Every
transport glyph it paints, the previous control included, SHALL retain the geometry of its own glyph,
so a click resolves that control's intent and no two controls share one region. A transport control
whose action is unavailable SHALL NOT resolve an intent. A
panel that is not painted SHALL resolve nothing.

A transport glyph SHALL resolve on a single press. The seekbar SHALL resolve a seek only on a
recognized double-click: two presses inside the double-click interval at the same cell. A single
press on the bar SHALL resolve neither a seek nor a transport intent. A left-button drag that
anchored inside the bar span SHALL scrub — each admitted motion resolves the seek fraction of the
pointer's current column — and the panel SHALL admit at most one scrub seek per 150 ms. A drag that
anchored outside the bar span, and a motion whose pointer is outside it, SHALL resolve no seek.

#### Scenario: Sidebar panel accepts clicks in queue-only

- **WHEN** the layout is queue-only and the playback panel is painted
- **THEN** a click on its play/pause glyph SHALL toggle playback, and a double-click on its seekbar
  SHALL seek to the corresponding fraction

#### Scenario: Sidebar panel accepts clicks in the two-panel layout

- **WHEN** the layout shows both columns and the playback panel is painted in the queue column
- **THEN** a click on its transport glyphs SHALL emit the corresponding transport intent

#### Scenario: Queue band resolves only its glyphs and bar

- **WHEN** the queue column's band is painted
- **THEN** a click on a glyph in the controls row SHALL emit that control's intent and a
  double-click on the seekbar row's bar SHALL seek
- **AND** a single press on the seekbar row's bar SHALL emit no playback intent
- **AND** a click on the blank title row, the blank gap row, or either time label SHALL NOT emit a
  playback intent

#### Scenario: Strip accepts clicks in library-only

- **WHEN** the layout is library-only
- **THEN** clicks on the strip's glyphs SHALL resolve against the strip's painted geometry, and a
  double-click on its seekbar SHALL seek

#### Scenario: A drag on the bar scrubs at most once per interval

- **WHEN** the panel is painted and the user presses the left button inside the seekbar's bar span
  and moves the pointer along it
- **THEN** each admitted motion SHALL seek to the fraction of the pointer's current column
- **AND** no two admitted scrubs SHALL be less than 150 ms apart

#### Scenario: A drag that anchored outside the bar does not scrub

- **WHEN** the user drags the pointer across the seekbar's bar span from a press that landed outside
  it
- **THEN** the panel SHALL emit no seek intent

#### Scenario: Previous control accepts clicks

- **WHEN** the panel is painted with the previous control available
- **THEN** a click on the previous glyph SHALL emit the previous transport intent
- **AND** its retained region SHALL NOT overlap the next control's

#### Scenario: Collapsed panel resolves nothing

- **WHEN** the panel is not painted because playback is idle
- **THEN** a click in the rows it would have occupied SHALL NOT emit a playback intent

### Requirement: Fractional Queue fill preserves the seek row and interaction boundaries

The bar span alone SHALL remain the pointer target. A partial cell SHALL be part of that
same target and SHALL resolve the same terminal-column-based seek fraction as before;
fractional painting SHALL NOT imply sub-cell pointer precision. The time labels, padding,
and blank transport rows SHALL continue to resolve no seek intent. When no bar width
remains, the existing clipped time-only fallback SHALL remain and SHALL expose no seek
region. No border, label, or extra row SHALL be added to the bar.

#### Scenario: A partially filled cell is still an ordinary seek target

- **WHEN** the user double-clicks a partially filled cell of the Queue seekbar
- **THEN** the seek SHALL resolve the same fraction that column resolved before
- **AND** a single press on the same cell SHALL NOT seek
- **AND** clicks on either flanking time label SHALL NOT seek

#### Scenario: A narrow seek row has no room for a bar

- **WHEN** the available Queue seek row leaves zero cells for the bar after its labels and
  padding are accounted for
- **THEN** the row SHALL retain its existing clipped time-only presentation
- **AND** it SHALL expose no seek region
