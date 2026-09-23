# home-latest-sections Specification

## Purpose

Defines Home's tab content as a Continue Watching destination: Home presents Continue Watching across libraries and retains no Latest selector, Latest rows, or Latest markers, which now live at each eligible destination under `destination-latest-modes`.

## Requirements

### Requirement: Home remains a Continue Watching destination
Home SHALL present Continue Watching without any Latest selector or Latest rows. A previously saved Home Latest selection SHALL fall back to Continue Watching on launch; the absence of Latest SHALL NOT hide the Home tab or prevent Emby Continue Watching from loading.

#### Scenario: Home after migration
- **WHEN** the user opens Home with eligible libraries and feed subscriptions
- **THEN** Home shows Continue Watching and no Latest pills or rows

#### Scenario: Legacy Home selection
- **WHEN** the saved launch location names a Home Latest section
- **THEN** Home opens on Continue Watching without failing or selecting another destination
