## ADDED Requirements

### Requirement: Landscape Wide Movie artwork carries its declared logo

When a Movie uses the Landscape Hero header at Wide geometry and declares a separate Logo image, the Library panel SHALL alpha-composite that logo into the top-left of the landscape artwork while preserving the logo's aspect ratio and transparency. The logo SHALL remain inset within and subordinate to the fanart, and the panel SHALL present the result as one image through every supported image protocol.

The landscape art SHALL NOT wait for the optional logo. A missing, failed, invalid, or still-loading logo SHALL leave the landscape art unchanged, and a logo that becomes available after the art SHALL decorate the shown art without changing the Hero header's shape or geometry.

Logo decoration SHALL NOT appear on Portrait Wide artwork, Narrow inline artwork, non-Movie artwork, placeholders, or a Movie that does not declare a Logo image. Playback badges and progress decoration SHALL NOT be part of this behavior.

#### Scenario: Landscape Wide Movie declares a logo

- **WHEN** a Movie with landscape base artwork and a declared transparent Logo image is selected at Wide geometry
- **THEN** its Logo is alpha-composited near the top-left of the fanart with its aspect ratio preserved
- **AND** one composited image is presented through the configured image protocol

#### Scenario: Landscape art arrives before its optional logo

- **WHEN** an eligible Movie fanart is ready while its declared Logo is still loading
- **THEN** the unmodified fanart is shown immediately
- **AND** the fanart is decorated after the Logo becomes available without changing its artwork box

#### Scenario: Declared logo cannot be used

- **WHEN** an eligible Movie's Logo request fails or does not decode as an image
- **THEN** the unmodified fanart remains shown
- **AND** no additional placeholder, error decoration, or reserved logo region appears

#### Scenario: Wide Movie uses portrait artwork

- **WHEN** a Movie with a declared Logo uses the Portrait Hero header at Wide geometry
- **THEN** its portrait artwork remains undecorated

#### Scenario: Movie uses inline artwork

- **WHEN** a Movie with a declared Logo is shown in the Narrow inline presentation
- **THEN** its artwork remains undecorated

#### Scenario: Ineligible hero content

- **WHEN** the shown hero is not a Movie, has no declared Logo, has no loaded base artwork, or uses the shared placeholder
- **THEN** no Logo request or decoration changes that hero's presentation
