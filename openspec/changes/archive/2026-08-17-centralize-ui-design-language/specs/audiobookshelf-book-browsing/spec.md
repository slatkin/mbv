## RENAMED Requirements

- FROM: `### Requirement: Book libraries use the Music tab composition`
  TO: `### Requirement: Book libraries use the hero-on-left arrangement`

## MODIFIED Requirements

### Requirement: Book libraries use the hero-on-left arrangement

An Audiobookshelf book library SHALL use the hero-on-left arrangement, the same arrangement grouped
Music uses: a persistent hero pane with chapters below, beside a persistent single-column book
browser with surname-bucket pills, at or above the shared breakpoint; falling back to hero-on-top
with a single list column below it. Both panes SHALL remain visible at all times. The book tab SHALL
obtain this arrangement from the shared definition rather than by reproducing the Music tab's
implementation, and SHALL NOT evaluate the breakpoint itself.

The following substitutions SHALL be the only domain changes to that arrangement. They are DATA
the book tab supplies — the arrangement renders whatever hero content, list rows, and pills the
screen hands it — so they are not presentation declarations. The book tab's single declaration of
differences SHALL cover only the presentation fields (image shape, metadata lines and order, colour
variant, element presence, and the `image source` for the cover):

| Hero-on-left default | Audiobookshelf book tab |
|---|---|
| Album | Book |
| Album cover | Audiobookshelf book cover |
| Track list (persistent hero pane) | Chapter list (persistent hero pane) |
| Artist grouping pills and filter drill | Author-surname bucket pills and filter drill |
| Album list within artist filter | Book list within surname-bucket filter |
| Left/right arrow toggles pane focus | Left/right arrow toggles pane focus |

All other observable layout behavior SHALL be that of the hero-on-left arrangement, including hero
placement, content padding, image slot, row budgeting, selected-cell treatment, focus styling,
scrolling, and narrow fallback.

#### Scenario: Terminal width crosses the two-column threshold

- **WHEN** the book tab crosses the shared breakpoint
- **THEN** the layout SHALL switch between hero-on-left and hero-on-top at the same width every other
  hero-on-left screen does

#### Scenario: Hero follows the browser cursor

- **WHEN** the book browser cursor moves to another book
- **THEN** the hero SHALL update to that book without an Enter/open action
- **THEN** the right-pane browser SHALL remain visible

#### Scenario: A surname pill filters the browser

- **WHEN** the user selects an author-surname bucket pill
- **THEN** the right-pane book list SHALL contain only books in that bucket until another bucket is
  selected

#### Scenario: Arrow focus leaves both panes visible

- **WHEN** the user presses left or right while the book tab is focused
- **THEN** focus SHALL toggle between the chapter list and right-pane browser
- **THEN** neither pane SHALL be hidden or replaced

#### Scenario: The hero-on-left arrangement changes

- **WHEN** the hero-on-left arrangement's presentation is changed
- **THEN** the book tab renders the change identically to grouped Music, without an individual edit
