## Purpose

Keeps artwork legible and consistently dimmed whenever an overlay darkens the view behind it, by switching image rendering to a protocol the dimming pass can actually reach.

## ADDED Requirements

### Requirement: Dimmed backdrops render images in halfblocks

While a dimmed backdrop is displayed, all images on that backdrop SHALL render using the halfblock image protocol, so that the dimming applies to artwork as well as to text. This SHALL apply to every overlay that dims its backdrop.

When no dimmed backdrop is displayed, images SHALL render using the user's configured image protocol.

Switching between protocols SHALL NOT discard already-rendered images of the other protocol, and SHALL NOT require refetching image data over the network.

#### Scenario: Overlay opened over artwork

- **WHEN** an overlay that dims its backdrop is opened over a view containing images
- **THEN** those images SHALL render in halfblocks
- **AND** SHALL be dimmed to the same degree as the surrounding text

#### Scenario: Overlay dismissed

- **WHEN** the dimming overlay is dismissed
- **THEN** images SHALL return to the user's configured image protocol

#### Scenario: Repeated open and close

- **WHEN** a dimming overlay is opened and dismissed repeatedly
- **THEN** images already rendered in each protocol SHALL be reused rather than refetched from the server

#### Scenario: Configured protocol is already halfblocks

- **WHEN** the user's configured protocol is halfblocks and a dimming overlay is opened
- **THEN** image rendering SHALL be unchanged

#### Scenario: Images disabled

- **WHEN** image rendering is disabled entirely and a dimming overlay is opened
- **THEN** no images SHALL be rendered in either state
