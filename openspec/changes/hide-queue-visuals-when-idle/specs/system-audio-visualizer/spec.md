## MODIFIED Requirements

### Requirement: Queue card selects artwork or visualization

mbv SHALL keep the artwork/visualizer queue-card selection session-local: every launch SHALL display artwork when available, and pressing unmodified `v` SHALL switch between those two contents without changing the queue card's reserved rectangle. When the layout is in queue-only state and no playback is active, the queue card SHALL NOT be rendered and SHALL reserve zero rows instead; the selection persists and takes effect on the next playback, and pressing `v` while idle SHALL NOT create a card rectangle.

#### Scenario: Launch after selecting the visualizer

- **WHEN** a previous run ended with the visualizer selected
- **THEN** the next launch displays queue artwork instead of the visualizer

#### Scenario: User selects the visualizer

- **WHEN** the queue card is rendered, displays artwork, and the user presses unmodified `v`
- **THEN** the same queue card rectangle displays the visualizer

#### Scenario: User selects artwork

- **WHEN** the queue card is rendered, displays the visualizer, and the user presses unmodified `v`
- **THEN** the same queue card rectangle displays the current queue artwork

#### Scenario: No playback can supply samples

- **WHEN** the visualizer is selected and no supported playback is active and the layout is not in queue-only state
- **THEN** the queue card rectangle remains present with an empty visualizer

#### Scenario: No playback in queue-only collapses the card

- **WHEN** the visualizer is selected and the layout is in queue-only state and no playback is active
- **THEN** no queue card rectangle SHALL be rendered and the queue list SHALL occupy those rows

#### Scenario: Idle visualizer selection captures no audio

- **WHEN** the user presses `v` while the layout is in queue-only state with no playback active
- **THEN** mbv SHALL NOT start system-audio capture and SHALL NOT repaint on the visualizer's frame cadence

#### Scenario: Selected item has no usable artwork

- **WHEN** the visualizer is selected and the current queue item has no usable artwork
- **THEN** the visualizer is displayed instead of the bundled queue-card placeholder

#### Scenario: Artwork is still loading

- **WHEN** artwork is selected and a usable image fetch is still pending and the queue card is rendered
- **THEN** mbv preserves the queue card's loading reservation until the fetch resolves

#### Scenario: Terminal images are disabled

- **WHEN** terminal images are disabled and the user switches between artwork and the visualizer while the queue card is rendered
- **THEN** the queue card keeps the same fallback rectangle, artwork selection renders no terminal image, the visualizer remains available, and mbv does not fetch artwork

## ADDED Requirements

### Requirement: Idle queue-only issues no card image fetches

When the layout is in queue-only state and no playback is active, mbv SHALL NOT fetch the card artwork for the cursor-selected item and SHALL NOT prefetch neighbour artwork for the card, because no card is rendered. Fetching SHALL resume when playback starts or the layout leaves queue-only state.

#### Scenario: Idle cursor movement fetches nothing

- **WHEN** the layout is in queue-only state with no playback active and the queue cursor moves
- **THEN** no card image fetch SHALL be issued for the newly selected item

#### Scenario: First playing frame after cold idle reserves the full slot

- **WHEN** playback starts in queue-only state before any card image has rendered this session
- **THEN** the queue card SHALL reserve its full uncached slot height while the first fetch is in flight and shrink to the image when it resolves
