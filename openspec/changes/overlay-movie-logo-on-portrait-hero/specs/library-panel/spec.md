## ADDED Requirements

### Requirement: Portrait Wide Movie artwork carries its declared logo

When a Movie uses the Portrait Hero header at Wide geometry and declares a separate Logo image, the Library panel SHALL alpha-composite that logo into the top-left of the portrait artwork while preserving the logo's aspect ratio and transparency. The logo SHALL remain inset within and subordinate to the poster, and the panel SHALL present the result as one image through every supported image protocol.

The poster SHALL NOT wait for the optional logo. A missing, failed, invalid, or still-loading logo SHALL leave the poster unchanged, and a logo that becomes available after the poster SHALL decorate the shown poster without changing the Hero header's shape or geometry.

Logo decoration SHALL NOT appear on Landscape Wide artwork, Narrow inline artwork, non-Movie artwork, placeholders, or a Movie that does not declare a Logo image. Playback badges and progress decoration SHALL NOT be part of this behavior.

#### Scenario: Portrait Wide Movie declares a logo

- **WHEN** a Movie with portrait base artwork and a declared transparent Logo image is selected at Wide geometry
- **THEN** its Logo is alpha-composited near the top-left of the poster with its aspect ratio preserved
- **AND** one composited image is presented through the configured image protocol

#### Scenario: Poster arrives before its optional logo

- **WHEN** an eligible Movie poster is ready while its declared Logo is still loading
- **THEN** the unmodified poster is shown immediately
- **AND** the poster is decorated after the Logo becomes available without changing its artwork box

#### Scenario: Declared logo cannot be used

- **WHEN** an eligible Movie's Logo request fails or does not decode as an image
- **THEN** the unmodified poster remains shown
- **AND** no additional placeholder, error decoration, or reserved logo region appears

#### Scenario: Wide Movie uses landscape artwork

- **WHEN** a Movie with a declared Logo uses the Landscape Hero header at Wide geometry
- **THEN** its landscape artwork remains undecorated

#### Scenario: Movie uses inline artwork

- **WHEN** a Movie with a declared Logo is shown in the Narrow inline presentation
- **THEN** its artwork remains undecorated

#### Scenario: Ineligible hero content

- **WHEN** the shown hero is not a Movie, has no declared Logo, has no loaded base artwork, or uses the shared placeholder
- **THEN** no Logo request or decoration changes that hero's presentation
