# queue-playback-panel

## MODIFIED Requirements

### Requirement: Playback controls respond to pointer input wherever the panel is painted

The playback panel's transport glyphs and seekbar SHALL resolve pointer input in every layout where
the panel renders, using the geometry of the panel that was actually painted in the current frame:
the queue column panel in queue-visible layouts and the right-column strip otherwise. Every
transport glyph it paints, the previous control included, SHALL retain the geometry of its own glyph,
so a click resolves that control's intent and no two controls share one region. A transport control
whose action is unavailable SHALL paint in its unavailable role and SHALL NOT resolve an intent. A
panel that is not painted SHALL resolve nothing.

#### Scenario: Sidebar panel accepts clicks in queue-only

- **WHEN** the layout is queue-only and the playback panel is painted
- **THEN** a click on its play/pause glyph SHALL toggle playback, and a click on its seekbar SHALL
  seek to the corresponding fraction

#### Scenario: Sidebar panel accepts clicks in the two-panel layout

- **WHEN** the layout shows both columns and the playback panel is painted in the queue column
- **THEN** a click on its transport glyphs SHALL emit the corresponding transport intent

#### Scenario: Strip accepts clicks in library-only

- **WHEN** the layout is library-only
- **THEN** clicks on the strip's glyphs and seekbar SHALL resolve against the strip's painted geometry

#### Scenario: Previous control accepts clicks

- **WHEN** the panel is painted with the previous control available
- **THEN** a click on the previous glyph SHALL emit the previous transport intent
- **AND** its retained region SHALL NOT overlap the next control's

#### Scenario: Collapsed panel resolves nothing

- **WHEN** the panel is not painted because playback is idle
- **THEN** a click in the rows it would have occupied SHALL NOT emit a playback intent
