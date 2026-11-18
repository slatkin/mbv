# Spec Delta

## MODIFIED Requirements

### Requirement: The first GUI slice presents the player panel of the design canvas

The GUI Client SHALL present one player panel whose content, arrangement, and states follow the design canvas. The TUI layout SHALL NOT be the reference. The footer line SHALL show the prefix-armed state and the toasts of the shared Status bar projection while the Library panel does not show; while the Library panel shows, the Library Status bar shows them. The footer line SHALL show the total time of the shown queue and, while rows are marked, the total time of the marked rows. A surface, or a Library destination, that the GUI Client does not yet present SHALL be absent, with no stub and no placeholder: the top bar carries only the destination tabs that the GUI Client presents, and it carries the Search field only once Search is presented.

#### Scenario: The queue has slots

- **GIVEN** the Owner process holds a Bound queue with slots
- **WHEN** the GUI Client window is open
- **THEN** the expanded panel shows, from top to bottom, a header, the artwork with its overlays, the title, the queue header, the queue rows, and a footer line
- **AND** the header shows the playback target with a chooser, the playback state with a leading dot, and a collapse control
- **AND** the artwork shows the active item, with the track chips at its top left, the season and episode badge at its top right, and the player controls at its bottom
- **AND** the player controls have play/pause and the scrubber on one row, and previous, back 10 s, forward 30 s, next, and the auto-hide control on the row under it
- **AND** the queue lists the slots in canonical order, marks the start slot, and numbers every row with its 1-based position in the queue

#### Scenario: The Status bar projection holds a toast

- **WHEN** the shared Status bar projection holds a toast
- **THEN** the GUI Client shows the toast in the Library Status bar while the Library panel shows, and otherwise in the footer line or in a conventional GUI form

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

#### Scenario: The image is still loading

- **GIVEN** the active item has an image that has not loaded yet
- **WHEN** the expanded panel paints
- **THEN** the panel reserves the artwork box at the panel's width with a 16:9 aspect and paints the overlays on it, as if the image were there
- **AND** the box takes the image's own aspect when the image loads

#### Scenario: The footer paints the queue's source text

- **WHEN** the expanded panel paints the footer line
- **THEN** the left line shows the queue's existing source text — the playlist text the frame carries — without a leading icon, the right line shows the shown queue's total time, and the line uses the canvas footer's type and border
- **AND** the footer shows no Player-owner label and no liveness dot
- **AND** the prefix, toast, autosave, and dirty states still paint in the footer line when they are active and the Library panel does not show

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

## ADDED Requirements

### Requirement: The player controls overlay the artwork

In the expanded panel, the player controls SHALL paint over the bottom of the artwork and SHALL take no more than one third of the artwork's height. The controls SHALL scale down with the artwork's height to stay inside that limit. The track chips and the season and episode badge SHALL paint over the top of the artwork. A press on a control over the artwork SHALL act on that control, not on the artwork.

#### Scenario: A short image scales the controls

- **GIVEN** the artwork's height is less than three times the controls' full height
- **WHEN** the expanded panel paints
- **THEN** the controls scale down so that they take no more than one third of the artwork's height

#### Scenario: A press lands on a control over the artwork

- **WHEN** the user presses play/pause over the artwork
- **THEN** the play/pause request runs and the artwork's own press does not

#### Scenario: A track popover opens over the artwork

- **WHEN** the user opens the audio popover from its chip at the top of the artwork
- **THEN** the popover paints in full, also where it reaches past the bottom of the artwork

### Requirement: The player controls auto-hide when the user turns it on

The player controls over the artwork SHALL show at all times by default. The auto-hide control SHALL turn auto-hide on and off. With auto-hide on, the controls SHALL hide while an item plays and the pointer is not over the artwork, and SHALL show while the pointer is over the artwork and while the playback is paused. Auto-hide SHALL NOT hide the track chips or the badge. The choice SHALL persist across launches.

#### Scenario: Auto-hide is off

- **GIVEN** auto-hide is off
- **WHEN** an item plays and the pointer is away from the artwork
- **THEN** the player controls show

#### Scenario: Auto-hide hides the controls during playback

- **GIVEN** auto-hide is on
- **WHEN** an item plays and the pointer is away from the artwork
- **THEN** the player controls are hidden and the track chips and the badge still show

#### Scenario: The pointer comes over the artwork

- **GIVEN** auto-hide is on and an item plays
- **WHEN** the pointer moves over the artwork
- **THEN** the player controls show

#### Scenario: The playback is paused

- **GIVEN** auto-hide is on
- **WHEN** the playback is paused and the pointer is away from the artwork
- **THEN** the player controls show

#### Scenario: The choice persists

- **GIVEN** the user turned auto-hide on
- **WHEN** the GUI Client starts again
- **THEN** auto-hide is on

### Requirement: The player panel without an image shows the player alone

When the active item has no image, the expanded panel SHALL paint no artwork box. Under the header it SHALL show the track chips and the badge on one line, then the player controls, then the title. The player controls SHALL always show in that case, whatever the auto-hide setting, and the auto-hide control SHALL still show and keep its setting.

#### Scenario: The item has no image

- **GIVEN** the active item has no image
- **WHEN** the expanded panel paints
- **THEN** no artwork box and no placeholder paint, and the track chips and badge line, the player controls, and the title paint in that order under the header

#### Scenario: Auto-hide is on and the item has no image

- **GIVEN** auto-hide is on and the active item has no image
- **WHEN** an item plays and the pointer is away from the panel
- **THEN** the player controls show and the auto-hide control still shows that auto-hide is on

### Requirement: The stopped player shows its buttons

While a shown local queue holds slots and nothing plays, the expanded panel SHALL show the two control rows under the header, with an empty scrubber that resolves no seek. The panel SHALL show no artwork, no title, and no track chips. A press on play/pause SHALL start the shown queue.

#### Scenario: Play from the stopped state

- **GIVEN** a shown local queue holds slots and nothing plays
- **WHEN** the user presses play/pause in the player controls
- **THEN** the shown queue starts from its start slot

#### Scenario: A shown peer queue is stopped

- **GIVEN** the shown queue is a peer's queue and nothing plays
- **WHEN** the expanded panel paints
- **THEN** the panel paints no control rows

#### Scenario: An empty shown local queue is stopped

- **GIVEN** a shown local queue holds no slots and nothing plays
- **WHEN** the expanded panel paints
- **THEN** the panel paints no control rows

## REMOVED Requirements

### Requirement: Every action is reachable from the keyboard

**Reason**: Later changes superseded it. The GUI Client now has pointer-first controls, and this requirement no longer matches how the GUI is built.

**Migration**: None. The key chords that exist stay. A new GUI control does not need a key chord.
