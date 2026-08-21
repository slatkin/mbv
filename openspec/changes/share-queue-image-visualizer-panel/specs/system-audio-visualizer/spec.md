## ADDED Requirements

### Requirement: Queue card selects artwork or visualization

mbv SHALL persist whether the queue card displays artwork or the visualizer. Pressing unmodified `v` SHALL switch between those two contents without changing the queue card's reserved rectangle.

#### Scenario: User selects the visualizer

- **WHEN** the queue card displays artwork and the user presses unmodified `v`
- **THEN** the same queue card rectangle displays the visualizer

#### Scenario: User selects artwork

- **WHEN** the queue card displays the visualizer and the user presses unmodified `v`
- **THEN** the same queue card rectangle displays the current queue artwork

#### Scenario: No playback can supply samples

- **WHEN** the visualizer is selected and no supported playback is active
- **THEN** the queue card rectangle remains present with an empty visualizer

#### Scenario: Selected item has no usable artwork

- **WHEN** the visualizer is selected and the current queue item has no usable artwork
- **THEN** the visualizer is displayed instead of the bundled queue-card placeholder

#### Scenario: Artwork is still loading

- **WHEN** artwork is selected and a usable image fetch is still pending
- **THEN** mbv preserves the queue card's loading reservation until the fetch resolves

#### Scenario: Terminal images are disabled

- **WHEN** terminal images are disabled and the user switches between artwork and the visualizer
- **THEN** the queue card keeps the same fallback rectangle, artwork selection renders no terminal image, the visualizer remains available, and mbv does not fetch artwork

### Requirement: Visualizer has one embedded placement

mbv SHALL render the embedded visualizer only in the queue card's artwork rectangle and SHALL NOT reserve a separate visualizer area below the queue list or within unused playback-panel rows.

#### Scenario: Visualizer is selected

- **WHEN** the visualizer is selected in any panel mode that displays the Queue panel
- **THEN** it occupies the queue artwork rectangle and the queue list retains all rows previously reserved for a separate visualizer

## MODIFIED Requirements

### Requirement: Local playback can show a system-audio vectorscope

When the visualizer is selected during supported local playback, mbv SHALL capture stereo PCM from the current default PipeWire system-output monitor and display it in the queue card's artwork rectangle.

#### Scenario: Visualizer starts with local playback

- **WHEN** supported local playback begins with the visualizer selected
- **THEN** mbv starts PipeWire capture and displays vectorscope points in the queue artwork rectangle without changing mpv audio output configuration

#### Scenario: Unrelated system audio is present

- **WHEN** another application produces audio on the current default system output
- **THEN** that application's stereo PCM MAY contribute to the vectorscope

### Requirement: Capture resources are bounded and cleaned up

mbv SHALL keep captured PCM in a bounded latest-sample buffer, supervise the PipeWire capture thread and stream, and release those resources after normal or failed shutdown.

#### Scenario: Capture outpaces rendering

- **WHEN** PipeWire supplies samples faster than the TUI consumes windows
- **THEN** mbv overwrites old samples and retains only the newest complete stereo samples

#### Scenario: Normal shutdown

- **WHEN** local playback ends or artwork is selected
- **THEN** mbv disconnects capture, stops its loop, joins its worker, and releases the sample buffer
