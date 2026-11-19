# Spec Delta

## MODIFIED Requirements

### Requirement: The first GUI slice presents the player panel of the design canvas

The GUI Client SHALL present one player panel whose content, arrangement, and states follow the design canvas. The TUI layout SHALL NOT be the reference. The window SHALL show the top bar as one global header across the full window width, above the Library panel and the Queue column, in every arrangement; the Settings screen SHALL show no global header. The panel SHALL show no header row above its artwork: the artwork SHALL be the panel's topmost content, directly under the global header. The footer line SHALL show the playback-target chooser and the collapse control at its left edge. The GUI Client SHALL paint no now-playing status word on any surface. The footer line SHALL show the prefix-armed state while the Library panel does not show; while the Library panel shows, the Library Status bar shows it. No status line SHALL show toasts: they show as floating cards (`gui-toasts`). The footer line SHALL show the total time of the shown queue and, while rows are marked, the total time of the marked rows. A surface, or a Library destination, that the GUI Client does not yet present SHALL be absent, with no stub and no placeholder: the top bar carries only the destination tabs that the GUI Client presents, and the Library Status bar carries the Search field only once Search is presented.

#### Scenario: The queue has slots

- **GIVEN** the Owner process holds a Bound queue with slots
- **WHEN** the GUI Client window is open
- **THEN** the expanded panel shows, from top to bottom, the artwork with its overlays, the title, the queue header, the queue rows, and a footer line
- **AND** the footer line's left shows the playback target with a chooser and the collapse control
- **AND** the artwork shows the active item, with the track chips at its top left, the season and episode badge at its top right, and the player controls at its bottom
- **AND** the player controls have two rows: the scrubber alone across the full width, and under it play/pause, previous, back 10 s, forward 30 s, and next at the left and the elapsed and remaining clocks at the right
- **AND** the queue lists the slots in canonical order, marks the start slot, and numbers every row with its 1-based position in the queue

#### Scenario: The Status bar projection holds a toast

- **WHEN** a toast is presented
- **THEN** the GUI Client shows it as a floating card, and neither the footer line nor the Library Status bar shows it

#### Scenario: The prefix is armed while a toast shows

- **GIVEN** a toast shows
- **WHEN** the user arms prefix mode while the Library panel does not show
- **THEN** the footer line shows the armed state

#### Scenario: The prefix is armed

- **WHEN** the user arms prefix mode
- **THEN** the Library Status bar shows the armed state while the Library panel shows, and otherwise the footer line shows it

#### Scenario: The queue is empty

- **GIVEN** the Owner process holds an empty queue
- **WHEN** the GUI Client window is open
- **THEN** the panel shows an empty-queue state in the type, colour, and spacing of the canvas

#### Scenario: A surface is not yet ported

- **WHEN** the user presses a key bound to a surface that the GUI Client does not present
- **THEN** no surface opens and no placeholder appears

#### Scenario: A destination is not yet presented

- **GIVEN** the GUI Client presents only the Music destination
- **WHEN** the top bar paints
- **THEN** it shows no tab for another destination, and a digit key for one opens nothing

#### Scenario: The artwork has a still slot

- **WHEN** the expanded panel paints the artwork of the active item
- **THEN** a loaded image fills the panel's width and takes its height from the artwork's aspect, as the TUI's visual slot paints at full column width
- **AND** no placeholder image and no source caption paint
- **AND** the season and episode badge sits at the top right of the artwork

#### Scenario: The footer paints the queue's source text

- **WHEN** the expanded panel paints the footer line
- **THEN** the left line shows the playback-target chooser and the collapse control, then the queue's existing source text — the playlist text the frame carries — without a leading icon, the right line shows the shown queue's total time, and the line uses the canvas footer's type and border
- **AND** the footer shows no Player-owner label and no liveness dot
- **AND** the prefix, autosave, and dirty states still paint in the footer line when they are active and the Library panel does not show

#### Scenario: The footer's target chooser opens above the band

- **WHEN** the user presses the playback-target control in the footer line
- **THEN** the chooser's popover opens above the footer band, anchored to the pressed control, and lists the live choice list with the attached output marked
- **AND** Esc and a press outside the popover close it

#### Scenario: No now-playing status word paints

- **WHEN** the window paints at any playback state
- **THEN** no surface paints a now-playing status word

#### Scenario: The footer totals the marked rows

- **GIVEN** three rows of the shown queue are marked
- **WHEN** the footer line paints
- **THEN** it shows the total time of the three marked rows beside the total time of the queue

#### Scenario: No row is marked

- **WHEN** no row of the shown queue is marked
- **THEN** the footer shows only the total time of the queue

#### Scenario: A runtime is unknown

- **GIVEN** the shown queue holds a slot whose runtime is unknown
- **WHEN** the footer line paints
- **THEN** the total shows the sum of the known runtimes and marks it as a lower bound

#### Scenario: The filter narrows the rows

- **GIVEN** the Queue filter narrows the shown queue
- **WHEN** the footer line paints
- **THEN** the total time still covers every slot of the shown queue

#### Scenario: The header spans both columns

- **GIVEN** the Library panel and the Queue column show, or the Queue column is collapsed to its rail
- **WHEN** the window paints
- **THEN** the top bar spans the full window width, and the Library column and the Queue column or its rail start below it

#### Scenario: The Settings screen shows

- **WHEN** the Settings screen shows
- **THEN** no global header paints


### Requirement: The Library Status bar shows hints and the location

While the Library panel shows, a Library Status bar SHALL show under the Library column: the Search field on the left, and the location on the right: the destination, All or the names of the checked collections, and the picked item. The bar SHALL show no key hints. While prefix mode is armed, the armed state SHALL paint after the Search field, and the field and the location SHALL stay. The bar SHALL show no toast.

#### Scenario: Music has an album picked

- **GIVEN** Music is active with Hip-Hop checked and a Beastie Boys album picked
- **WHEN** the Library Status bar paints
- **THEN** it shows the Search field and the location Music, Hip-Hop, Beastie Boys, and no key hints

#### Scenario: Two collections are checked

- **GIVEN** Feeds is active with Tifo Football and Off Menu checked
- **WHEN** the Library Status bar paints
- **THEN** the location names Feeds, then both subscriptions, then the picked entry

#### Scenario: A toast arrives

- **WHEN** a toast is presented while the Library panel shows
- **THEN** the Library Status bar does not show it and does not change

