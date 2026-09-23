# home-latest-sections Specification

## Purpose

Defines Home's tab content as a Continue Watching destination: Home presents Continue Watching across libraries and retains no Latest selector, Latest rows, or Latest markers, which now live at each eligible destination under `destination-latest-modes`.

## Requirements

### Requirement: Home is presented as Continue with no Selector row
The Home destination's tab is labelled `Continue`, and it presents Continue Watching without any Latest selector or Latest rows. Because the destination has no remaining selector choices, it presents no Selector row or pill bar; only the Continue Watching rows and hero. A previously saved Home Latest selection falls back to Continue Watching on launch; the absence of Latest does not hide the Continue tab or prevent Emby Continue Watching from loading.

#### Scenario: Home after migration
- **WHEN** the user opens the Continue tab with eligible libraries and feed subscriptions
- **THEN** the tab is labelled `Continue`, shows Continue Watching with no Latest pills or rows, and has no Selector row or pill bar

#### Scenario: Legacy Home selection
- **WHEN** the saved launch location names a Home Latest section
- **THEN** the Continue tab opens on Continue Watching without failing or selecting another destination
