# Spec Delta

## Purpose

How the GUI Client presents toasts: as cards that float over the window, apart from every status line, with their order, maximum count, and place.

## ADDED Requirements

### Requirement: Toasts float over the window

The GUI Client SHALL show each presented toast as a card that floats at the bottom-right of the window. The cards SHALL paint over every panel, the Settings screen, and modal overlays, in every arrangement. A card SHALL show the toast's text and its severity. The cards SHALL NOT change the layout of what is under them.

#### Scenario: A toast arrives while the Library panel shows

- **GIVEN** the Library panel and the Queue column show
- **WHEN** a toast is presented
- **THEN** a card shows the toast at the bottom-right of the window, over the Queue column
- **AND** the Library Status bar and the footer line do not move or change

#### Scenario: A toast arrives while the Settings screen shows

- **GIVEN** the Settings screen shows
- **WHEN** a setting change is refused with a notice
- **THEN** the notice shows as a card at the bottom-right of the window, over the Settings screen

#### Scenario: A toast arrives while a confirmation modal shows

- **GIVEN** a confirmation modal shows
- **WHEN** a toast is presented
- **THEN** the card paints over the modal and its scrim

### Requirement: Several toast cards show in arrival order

While several toasts are presented, the GUI Client SHALL show one card for each, one above the other, with the newest card at the bottom. When a toast expires, its card SHALL go and the other cards SHALL stay.

#### Scenario: A second toast arrives

- **GIVEN** a Warning toast shows
- **WHEN** a Success toast is presented
- **THEN** two cards show: the Warning card above the Success card

#### Scenario: The older toast expires first

- **GIVEN** two cards show
- **WHEN** the older toast expires
- **THEN** only the newer card shows

### Requirement: Toast cards take no input

A toast card SHALL have no control and SHALL take no pointer input. A press or a hover on a card SHALL go to what is under the card. No key SHALL act on a card.

#### Scenario: A press lands on a card

- **GIVEN** a card shows over a queue row
- **WHEN** the user presses on the card
- **THEN** the press acts on the queue row under the card, as if no card showed
