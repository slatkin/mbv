# Spec Delta

## MODIFIED Requirements

### Requirement: Queue card selects artwork or visualization

mbv SHALL keep the artwork/visualizer queue-card selection session-local: every launch SHALL display artwork when available, and pressing unmodified `v` SHALL switch between those two contents without changing the queue card's reserved rectangle. The same selection SHALL also be reachable from the queue card's context menu. When the Queue panel is visible and no playback is active, the queue card SHALL NOT be rendered and SHALL reserve zero rows instead; the selection persists and takes effect on the next playback, and pressing `v` while idle SHALL NOT create a card rectangle.

#### Scenario: Launch after selecting the visualizer

- **WHEN** a previous run ended with the visualizer selected
- **THEN** the next launch displays queue artwork instead of the visualizer

#### Scenario: User selects the visualizer

- **WHEN** the queue card is rendered, displays artwork, and the user presses unmodified `v`
- **THEN** the same queue card rectangle displays the visualizer

#### Scenario: User selects artwork

- **WHEN** the queue card is rendered, displays the visualizer, and the user presses unmodified `v`
- **THEN** the same queue card rectangle displays the current queue artwork

#### Scenario: User selects the visualizer from the queue card menu

- **WHEN** the queue card is rendered, displays artwork, and the user chooses the menu's visualizer entry
- **THEN** the same queue card rectangle displays the visualizer

#### Scenario: User selects artwork from the queue card menu

- **WHEN** the queue card is rendered, displays the visualizer, and the user chooses the menu's artwork entry
- **THEN** the same queue card rectangle displays the current queue artwork

#### Scenario: Idle queue-visible layout collapses the card

- **WHEN** the visualizer is selected, the Queue panel is visible, and no playback is active
- **THEN** no queue card rectangle SHALL be rendered and the Queue panel SHALL occupy those rows

#### Scenario: Idle visualizer selection captures no audio

- **WHEN** the user presses `v` with no playback active
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
