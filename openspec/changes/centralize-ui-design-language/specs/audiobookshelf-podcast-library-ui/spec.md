## RENAMED Requirements

- FROM: `### Requirement: Podcast libraries use the TV Shows tab composition`
  TO: `### Requirement: Podcast libraries use the hero-on-top arrangement`

## MODIFIED Requirements

### Requirement: Podcast libraries use the hero-on-top arrangement

An Audiobookshelf podcast library SHALL use the hero-on-top arrangement, the same arrangement the TV
Shows tab uses. The selected podcast hero SHALL occupy the dedicated full-width area pinned above the
podcast show list. The podcast show list SHALL occupy the remaining area below the hero and SHALL NOT
appear beside the hero or detail content at any terminal width. The podcast tab SHALL obtain this
composition from the shared arrangement rather than by reproducing the TV Shows tab's implementation.

The following substitutions SHALL be the only domain changes to that arrangement. They are DATA
the podcast tab supplies — the arrangement renders whatever hero content, list rows, and pills the
screen hands it — so they are not presentation declarations. The podcast tab's single declaration
of differences SHALL cover only the presentation fields (image shape, metadata lines and order,
colour variant, element presence, and the `image source` for the cover):

| Hero-on-top default | Audiobookshelf podcast tab |
|---|---|
| Series | Podcast show |
| Series Primary image | Audiobookshelf podcast cover |
| Season selector | `All` / `Played` / `Unplayed` filter selector |
| Episodes in the selected season | Downloaded episodes matching the selected filter |

All other observable layout behavior SHALL be that of the hero-on-top arrangement, including the hero
shell and placement, list-below-hero ordering, content padding, image slot, row budgeting, list
column count, selected-cell treatment, focus styling, scrolling, narrow-terminal fallback, and
loading placeholder stability.

#### Scenario: Podcast library is displayed

- **WHEN** an Audiobookshelf podcast library and a TV Shows library are displayed at the same
  terminal dimensions and image setting
- **THEN** both tabs SHALL divide the content area into the same top hero and lower list geometry
- **THEN** the podcast tab SHALL render podcast shows in the lower list positions occupied by Series
  rows in the TV Shows tab
- **THEN** the podcast tab SHALL NOT render a left catalog column beside a right detail column

#### Scenario: Podcast selection changes

- **WHEN** the user moves selection between podcast shows
- **THEN** the fixed hero SHALL update to the newly selected podcast without changing its screen
  position
- **THEN** the show list SHALL retain provider-native selection identity across loaded-page changes

#### Scenario: Selected show scrolls outside the visible list rows

- **WHEN** the selected podcast's row is outside the visible portion of the lower show list
- **THEN** the pinned hero SHALL continue to display that selected podcast in the same fixed position

#### Scenario: Terminal width crosses the TV list column breakpoint

- **WHEN** the podcast tab crosses the shared breakpoint
- **THEN** the podcast show list SHALL change column count at that breakpoint, as every hero-on-top
  screen does
- **THEN** the hero SHALL remain full-width above the list and SHALL NOT move to the side

#### Scenario: Terminal height cannot fit the hero

- **WHEN** the hero-on-top arrangement would suppress its hero because the available height cannot
  fit the minimum hero and a usable list
- **THEN** the podcast tab SHALL suppress its hero under the same condition and give the show list
  the corresponding area

#### Scenario: The hero-on-top arrangement changes

- **WHEN** the hero-on-top arrangement's presentation is changed
- **THEN** the podcast tab renders the change without an individual edit
