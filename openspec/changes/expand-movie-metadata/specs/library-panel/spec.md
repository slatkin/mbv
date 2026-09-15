## MODIFIED Requirements

### Requirement: The overview is a Main content box below the Hero header

When the shown item has overview text or Movie credits, the Hero pane SHALL render the overview Main
content box directly below the Hero header, at the shared padding, for every header type. When the
item has neither, no overview box SHALL render and the Workspace (if any) SHALL move up.

The box SHALL hold the item's overview text when it has one. A Movie hero with credits SHALL render
its cast and crew table in the same box, one blank row under the overview text, or at the box's first
content row when the item has no overview text.

#### Scenario: A Movie with an overview

- **WHEN** a Movie with overview text is selected at Wide geometry
- **THEN** the overview renders inside a Main content box below the header

#### Scenario: A Movie with credits but no overview text

- **WHEN** a Movie carrying cast and crew has no overview text
- **THEN** the overview Main content box renders with the cast and crew table and no overview paragraph

#### Scenario: An album without an overview

- **WHEN** a Music album without overview text is selected at Wide geometry
- **THEN** no overview box renders and the track Workspace follows the header

## ADDED Requirements

### Requirement: A Movie hero's metadata rows carry genres and provider links

A Movie hero's metadata rows SHALL carry, in order, the item's release date, its runtime, one row
joining every genre the item declares (delimited by a single `/` with no spacing), and one row
joining the item's provider link names. Genres come from
the provider's genre list, not from the first genre alone. A row SHALL NOT render when its
content is empty.

Only items the provider types as a Movie SHALL carry the genre and provider-link rows. Every other
item type SHALL keep the metadata rows it renders today, including a TV Series' combined year and
genre row.

Metadata rows remain an ordered list of plain-text rows coloured by position (see *Wide Hero header
has three types chosen by the artwork policy*), so adding rows does not change how any row is coloured
or wrapped.

#### Scenario: A Movie with genres and links

- **WHEN** a Movie declaring `Action` and `Drama` plus IMDb and TheMovieDb links is selected at Wide
  geometry
- **THEN** its metadata rows show the release date, the runtime, a row carrying both genres, and a row
  carrying both link names, in that order

#### Scenario: A Movie with no provider links

- **WHEN** a Movie declares no provider links
- **THEN** no provider-link row renders and the rows above it are unchanged

#### Scenario: A TV Series is unaffected

- **WHEN** a TV Series is selected at Wide geometry
- **THEN** its metadata rows are the combined year-and-genre row, release date and runtime it renders
  today, with no separate genre or provider-link row

#### Scenario: An album is unaffected

- **WHEN** a Music album is selected at Wide geometry
- **THEN** its metadata rows are unchanged and carry no genre or provider-link row

### Requirement: A Movie hero carries a cast and crew table

A Movie hero whose item declares people SHALL render a cast and crew table. The table SHALL list
every person in the item's people list: every person the provider types as a director first, in
provider order, followed by every remaining person in provider order regardless of the provider's
type for them. The table SHALL NOT cap or reorder the provider's people beyond moving directors to
the front. (Amended 2026-09-15 from the user's visual sweep; the former 9-actor cap is removed.)

Each row SHALL show the person's name in a first column and their role in a second column. The second
column SHALL span the overview box's remaining width after the name column, which is sized from the
longest name the table shows, and every role SHALL be right-aligned at the box's right edge. A role
longer than the remaining width SHALL be truncated with an ellipsis at the box's edge. The table SHALL have
no header row, and SHALL use the body text treatment the overview paragraph
uses. A separator line of `▁` (U+2581) block characters spanning the box's content width in the
sage-green text role (#A7C080) SHALL sit directly under the overview text, with one blank row between
the separator and the table's first row. Table rows
SHALL be zebra-striped: alternate rows (by table position, first row unstriped) SHALL carry the
darker surface tone #333c43 behind the body text, applied through a semantic theme role. The table's
name column SHALL render in a yellow text role backed by the theme's `YELLOW` primitive (#dbbc7f).

A person's role SHALL be the provider's role text for that person. When the provider supplies no role
text, the person's provider type SHALL be shown instead.

The cast and crew table SHALL scroll within the overview Main content box when it offers more rows
than the box's height shows: the overview text and the separator line SHALL stay pinned at the box's
top, the table rows SHALL shift under them, and a scrollbar indicator SHALL paint at the box's right
edge only while the table overflows. The scroll offset SHALL be component-local (owned by the Library
panel's destination owner), clamped at both ends, and reset when the shown item changes. The table
SHALL render only in the
Wide Hero pane: the Narrow inline hero SHALL NOT render a cast and crew table.

#### Scenario: A Movie with one director and a full cast

- **WHEN** a Movie declares one director and 14 acting cast members
- **THEN** the table renders the director's row first followed by all 14 cast rows, 15 rows in total

#### Scenario: A Movie with two directors

- **WHEN** a Movie declares two directors and 12 acting cast members
- **THEN** both director rows render first, followed by all 12 cast rows

#### Scenario: A Movie with no director

- **WHEN** a Movie declares acting cast members but no director
- **THEN** the table renders cast rows only, with no director row

#### Scenario: People of other provider types

- **WHEN** a Movie's people include a writer and a composer alongside its director and cast
- **THEN** the writer and composer rows render after the cast rows, in provider order

#### Scenario: A Movie with a director and 2 acting cast members

- **WHEN** a Movie declares a director and 2 acting cast members
- **THEN** the table renders 3 rows and no empty row

#### Scenario: A person with no role text

- **WHEN** a person in the table has no role text
- **THEN** that row shows the person's provider type in the role column

#### Scenario: A Movie with no people

- **WHEN** a Movie declares no people
- **THEN** no cast and crew table renders, and the overview box renders as it does for any item with
  overview text

#### Scenario: A Movie selected in the Narrow panel

- **WHEN** a Movie with cast and crew is selected at Narrow geometry
- **THEN** the inline hero renders its title, metadata rows, overview and image with no cast and crew
  table

#### Scenario: A short Hero pane

- **WHEN** the Hero pane offers fewer rows than the overview text and the cast and crew table need
- **THEN** the table is clipped at the overview box's bottom edge rather than overflowing the pane or
  scrolling

### Requirement: A Movie hero's provider links are ctrl-clickable via OSC 8 hyperlinks

Each provider link name in a Movie hero's provider-link row SHALL be rendered as a clickable link
using OSC 8 escape sequences when the terminal declares hyperlink support, so that ctrl-clicking the
name opens that provider's URL in the user's browser.

A URL SHALL be embedded in an escape sequence only when it is an `http` or `https` URL containing no
control bytes. A link whose URL fails that check SHALL render as plain text with no escape sequence,
and the other links in the row SHALL still render as clickable.

When the terminal does not declare hyperlink support, the row SHALL render as plain text with no
escape sequences. The Narrow inline hero SHALL render the provider-link row as plain text.

#### Scenario: A supported terminal

- **WHEN** a Movie hero renders its provider-link row on a terminal declaring hyperlink support
- **THEN** each link name is wrapped in an OSC 8 escape sequence carrying that provider's URL

#### Scenario: An unsupported terminal

- **WHEN** a Movie hero renders its provider-link row on a terminal that does not declare hyperlink
  support
- **THEN** the row renders as plain text with no escape sequences

#### Scenario: A link with a non-web URL

- **WHEN** a provider link carries a URL that is not `http` or `https`
- **THEN** that link name renders as plain text with no escape sequence

#### Scenario: A link with control bytes in its URL

- **WHEN** a provider link's URL contains a control byte
- **THEN** that link name renders as plain text with no escape sequence
