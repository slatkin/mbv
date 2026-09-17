# library-panel Specification

## Purpose

Defines the one Library panel skeleton that every library destination renders through, in its Wide and
Narrow forms, so that destinations can differ only in the content they supply and never in how the
panel is composed.

## Requirements

### Requirement: Every library destination renders through one Library panel skeleton

Home, Movies, Emby home videos, TV shows, grouped Music, Audiobookshelf Books, Audiobookshelf Podcasts and Feeds SHALL each render through the same Library panel: the Wide library panel when the shared Wide width and minimum-height conditions are met, and the non-Wide library panel otherwise. The panel SHALL own every row, pane, fill, border, gap, inset, overlay and placeholder position. A destination SHALL supply only typed content for the panel's slots and SHALL NOT paint outside a slot, add a slot, or select an alternative skeleton, pane order, fill, inset, border, or overlay placement.

A slot a destination does not fill SHALL render as absent in the same way on every destination. Two destinations supplying the same kind of content SHALL render it identically at the same geometry. In non-Wide geometry the Browser pane SHALL show ordinary fixed-height canonical rows; selected-row replacement and Inline hero SHALL NOT render.

#### Scenario: Two destinations with the same slot content
- **WHEN** two library destinations supply the same kind of content for the same slots at the same terminal size
- **THEN** their panes, rows, fills, borders and insets occupy the same cells with the same surfaces

#### Scenario: A destination has no content for an optional slot
- **WHEN** a destination supplies no List controls row content
- **THEN** no List controls row renders and the rows below move up exactly as on every other destination without that content

#### Scenario: The breakpoint is crossed
- **WHEN** the terminal crosses the shared Wide conditions in either direction
- **THEN** every library destination switches between the Wide two-pane panel and the non-Wide standard-list panel at the same width and height
- **AND** no selected row changes height during that transition

### Requirement: Emby screens are the reference presentation

Where library destinations differ in presentation, the Emby destinations (Home, Movies, home videos,
TV shows, grouped Music) SHALL define the Library panel's presentation, and Audiobookshelf and Feeds
destinations SHALL conform to it. An Audiobookshelf or Feeds presentation SHALL NOT be a reason to
change a slot, and SHALL NOT remain as a destination-only difference.

#### Scenario: An Audiobookshelf screen differs from the Emby screens
- **WHEN** an Audiobookshelf destination renders a slot differently from the Emby destinations
- **THEN** the Audiobookshelf destination is non-conforming and renders the Emby presentation

### Requirement: The list pane has one Selector row and one optional List controls row

The list pane (the Wide panel's left pane, and the whole Narrow panel) SHALL present, top to bottom:
at most one Selector row (a single pill bar followed by the panel's spacer row), at most one List
controls row, and the list box. The Selector row carries destination browse selectors, including the
Feeds All / Played / Unplayed watched filter followed by its feed-group pills. The List controls row
carries secondary controls such as the Emby home-video item count. No destination SHALL render a
second pill bar.

#### Scenario: Feeds renders its selectors
- **WHEN** the Feeds destination renders with subscriptions available
- **THEN** its All / Played / Unplayed filter and feed-group pills render in the one Selector row
- **AND** no List controls row or second pill bar renders

#### Scenario: An Emby home-video library renders its count
- **WHEN** an Emby home-video library renders
- **THEN** its item count renders in the List controls row, not above or inside the Selector row

#### Scenario: Selector pill hit geometry
- **WHEN** the user clicks a pill in any destination's Selector row
- **THEN** the pill painted under the pointer in the latest frame is selected

### Requirement: Inline Search replaces the Selector row and list box

While Inline Search is active in a searchable destination, the search box SHALL render in the Selector
row's place and the results SHALL render in the list box's place, with the rest of the panel unchanged.
The Selector row's place SHALL be reserved for the search box even when the destination supplies no
Selector row.
There SHALL be no other library search presentation inside the Library panel.

#### Scenario: Inline Search opens on a Wide destination
- **WHEN** Inline Search is active on a Wide searchable destination
- **THEN** the search box occupies the Selector row and the results occupy the list box
- **AND** the Hero pane continues to render the destination's selected item

### Requirement: Wide Hero header has three types chosen by the artwork policy

The Wide Hero pane SHALL begin with a Hero header of exactly one of three types:

- **Landscape**: the artwork spans the pane's content width at the top, followed by title and metadata.
- **Portrait**: title and metadata on the left, a 2:3 artwork box on the right.
- **Square**: title and metadata on the left, a 1:1 artwork box on the right.

The type SHALL be the shape of the artwork chosen by one artwork policy, applied in exactly one place
for every item wherever it appears (including Home rows), and never selected by a destination:

- Music (albums and tracks), Audiobookshelf podcasts (shows and episodes), and audio Feeds
  entries SHALL always use Square artwork.
- Every other item SHALL use the most preferred artwork shape its provider declares available, in the
  order Landscape, then Square, then Portrait.
- An item with no available artwork SHALL use Landscape (Square for Music, Audiobookshelf podcasts,
  and audio Feeds entries) with the
  shared placeholder.

Availability SHALL be decided from provider metadata before any image is fetched, so the header type
does not depend on fetched pixels for its initial arm. The image the policy chooses SHALL be the same
image for that item in the Wide header and the Library Hero overlay. When the chosen image decodes,
the header SHALL re-arm from the painted artwork's aspect: roughly landscape-decoding artwork keeps
the Landscape arm, squarish artwork re-arms to Square, and portrait artwork re-arms to Portrait —
so a landscape-declared thumbnail that resolves to a portrait poster paints the Portrait arm instead
of cover-cropping the poster into a landscape box. Until an image decodes, the declared policy shape
holds.

The header SHALL render title and metadata through one presentation for all three types. Metadata is
an ordered list of plain-text rows supplied by the destination; the panel SHALL colour row *n* with
the *n*-th of three metadata colours, repeating from the first after the third, and SHALL own
truncation and wrapping. A destination SHALL NOT style metadata.

The artwork SHALL fill its box completely: an image whose aspect differs from its box SHALL be scaled
to cover the box and cropped, centred, so no margin of the box shows. When images are disabled or the
artwork is not loaded yet, the artwork box SHALL render the shared placeholder at the same size. When
vertical space is constrained, the artwork SHALL shrink before a Workspace list viewport is removed.

#### Scenario: A TV series is selected at Wide geometry
- **WHEN** a TV series with a landscape thumbnail is the selected item in the Wide panel
- **THEN** a Landscape header renders its artwork above its title and metadata

#### Scenario: An album is selected at Wide geometry
- **WHEN** a Music album is the selected item in the Wide panel
- **THEN** a Square header renders title and metadata on the left and square artwork on the right

#### Scenario: An Audiobookshelf podcast is selected at Wide geometry
- **WHEN** an Audiobookshelf podcast show, or an audio Feeds entry, is the selected item
- **THEN** a Square header renders

#### Scenario: An Audiobookshelf book appears in a Home row
- **WHEN** an Audiobookshelf book with only its portrait cover is the selected Home row at Wide
  geometry
- **THEN** a Portrait header renders, the same header the Books tab renders for that book

#### Scenario: A Movie with both a backdrop and a poster
- **WHEN** a Movie declares both landscape and portrait artwork
- **THEN** a Landscape header renders with the landscape artwork, on Home and on the Movies tab alike

#### Scenario: An item without artwork
- **WHEN** a video Feeds entry with no artwork is selected at Wide geometry
- **THEN** a Landscape header renders with the shared placeholder in its artwork box

#### Scenario: A landscape-declared thumbnail decodes to a portrait poster
- **WHEN** an item's artwork policy declares Landscape artwork but the decoded image's aspect is
  portrait
- **THEN** the header re-arms to the Portrait arm once the image is ready, instead of cover-cropping
  the poster into the landscape box

#### Scenario: Metadata rows are coloured by position
- **WHEN** a header shows four metadata rows
- **THEN** rows one to three use the first, second and third metadata colours and row four uses the
  first again, on every destination

#### Scenario: A 4:3 image in a Landscape header
- **WHEN** the chosen landscape artwork is 4:3 rather than 16:9
- **THEN** it fills the 16:9 box, cropped at the top and bottom edges, with no empty margin inside the
  box

#### Scenario: Artwork has not loaded
- **WHEN** a header's artwork is still loading or absent
- **THEN** the artwork box keeps its size and shows the shared placeholder

### Requirement: Wide Hero artwork height is capped

The Wide Hero header's artwork box SHALL be at most 25 rows tall for the Landscape header type and at
most 20 rows tall for the non-landscape types, Portrait and Square. The cap applies to the artwork box,
not to the pane.

The cap SHALL be one rule: painting the artwork, sizing the shared placeholder, the shell's image
projection and its cover-fit encoding SHALL all use the same capped box, so a tall pane never fetches or
encodes an image larger than the box that paints it.

Smaller-space constraints SHALL still apply on top of the cap. A header in a pane that offers fewer
rows, or whose box is shrunk by a present Workspace or by the Landscape text block's room, SHALL use
that smaller height.

#### Scenario: A tall Wide pane with a Portrait header
- **WHEN** a Portrait header renders in a Wide Hero pane with more than 20 rows available for artwork
- **THEN** the artwork box is 20 rows tall and the encoded image matches that box

#### Scenario: A tall Wide pane with a Square header
- **WHEN** a Square header renders in a Wide Hero pane with more than 20 rows available for artwork
- **THEN** the artwork box is 20 rows tall and the encoded image matches that box

#### Scenario: A tall Wide pane with a Landscape header
- **WHEN** a Landscape header's 16:9 artwork box would be taller than 25 rows at the pane's width
- **THEN** the artwork box is capped at 25 rows

#### Scenario: A constrained pane
- **WHEN** a Wide Hero pane offers fewer rows for artwork than the header type's cap, or a present
  Workspace or starved text block shrinks the box below the cap
- **THEN** the artwork box uses the smaller height

#### Scenario: Artwork has not loaded in a capped box
- **WHEN** a header's artwork is still loading or absent in a box that the cap limits
- **THEN** the shared placeholder renders at the capped box's size

### Requirement: The overview is a Main content box below the Hero header

When the shown item has overview text or Movie credits, the Hero pane SHALL render the overview Main
content box directly below the Hero header, at the shared padding, for every header type. When the
item has neither, no overview box SHALL render and the Workspace (if any) SHALL move up.

The box SHALL hold the item's overview text when it has one. A Movie hero with credits SHALL render
its cast and crew table in the same box: the table follows the overview text through the separator
line and blank row the cast and crew table requirement defines, or starts at the box's first content
row when the item has no overview text.

#### Scenario: A Movie with an overview

- **WHEN** a Movie with overview text is selected at Wide geometry
- **THEN** the overview renders inside a Main content box below the header

#### Scenario: A Movie with credits but no overview text

- **WHEN** a Movie carrying cast and crew has no overview text
- **THEN** the overview Main content box renders with the cast and crew table and no overview paragraph

#### Scenario: An album without an overview

- **WHEN** a Music album without overview text is selected at Wide geometry
- **THEN** no overview box renders and the track Workspace follows the header

### Requirement: A Movie hero's metadata rows carry genres and provider links

A Movie hero's metadata rows SHALL carry, in order, the item's release date, its runtime, one row
joining every genre the item declares (delimited by a single `/` with no spacing), and one row
joining the item's IMDb provider link, when the item declares one (user direction 2026-09-15: only
IMDb — other providers' links are not shown). Genres come
from the provider's genre list, not from the first genre alone. A row SHALL NOT render when its
content is empty.

Only items the provider types as a Movie SHALL carry the genre and provider-link rows. Every other
item type SHALL keep the metadata rows it renders today, including a TV Series' combined year and
genre row.

Metadata rows remain an ordered list of plain-text rows coloured by position (see *Wide Hero header
has three types chosen by the artwork policy*), so adding rows does not change how any row is coloured
or wrapped.

#### Scenario: A Movie with genres and an IMDb link

- **WHEN** a Movie declaring `Action` and `Drama` plus IMDb and TheMovieDb links is selected at Wide
  geometry
- **THEN** its metadata rows show the release date, the runtime, a row carrying both genres, and a
  row carrying the IMDb link name only, in that order

#### Scenario: A Movie with no IMDb link

- **WHEN** a Movie declares other providers' links but no IMDb link
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

The overview text, the separator line, the blank row and the cast and crew table SHALL form one
scrollable flow inside the overview Main content box: when the flow offers more rows than the box's
height shows, the caller's scroll offset SHALL shift the whole flow — the overview text and the
separator scroll out of the box above the table instead of staying pinned — and a scrollbar
indicator SHALL paint at the box's right edge only while the flow overflows. The scroll offset SHALL
be component-local (owned by the Library
panel's destination owner), clamped at both ends against the whole flow's length, and reset when the
shown item changes. The table
SHALL render only in the Wide Hero pane.

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

#### Scenario: A long overview scrolls with its table

- **WHEN** a Movie's overview and cast and crew table need more rows than the overview box shows
- **THEN** scrolling the box shifts the overview text and the separator out of it above the table
- **AND** the scroll range covers the overview, the separator, the blank row and every table row

#### Scenario: A Movie selected in the Narrow panel

- **WHEN** a Movie with cast and crew is selected at Narrow geometry
- **THEN** the inline hero renders its title, metadata rows, overview and image with no cast and crew
  table

### Requirement: The Movie hero's IMDb link opens the provider URL when clicked

The provider-link name in a Movie hero's provider-link row SHALL open the provider's URL in the
user's browser when clicked: mbv owns the captured mouse, so the link click SHALL be resolved by the
Library panel against the label geometry it painted and handled by the same system-opener path the
feed link uses (`xdg-open`/`open`/`start` per platform), not by terminal hyperlink escapes.
(Reversed 2026-09-15 from the original OSC 8 design: terminal-side ctrl-click is unavailable while
the application captures the mouse, and an application-owned handler works on every terminal.)

A URL SHALL be opened only when it is an `http` or `https` URL containing no control bytes. A link
whose URL fails that check SHALL NOT open and SHALL render as ordinary body text. The Narrow inline
hero SHALL render the provider-link row as plain text with no click handling.

#### Scenario: Clicking the link name

- **WHEN** the user clicks a Movie hero's IMDb link name at Wide geometry
- **THEN** the provider URL opens in the user's browser

#### Scenario: A link with a non-web URL

- **WHEN** a provider link carries a URL that is not `http` or `https`
- **THEN** the name renders as ordinary body text and clicking it does nothing

#### Scenario: A link with control bytes in its URL

- **WHEN** a provider link's URL contains a control byte
- **THEN** the name renders as ordinary body text and clicking it does nothing

### Requirement: Wide Workspaces are one header plus one Selector row plus one list box

A destination whose item has constituent items (TV seasons and episodes, Music tracks, Audiobookshelf
chapters) SHALL present them in a Workspace below the overview: an
optional one-row header, an optional Selector row (season pills)
followed by one Main content box
holding the canonical list. The Workspace box SHALL render the accent-soft surface while its list holds
focus and the backdrop surface otherwise. Its selected row SHALL paint the selected-row bar like every
other canonical list row. A Hero pane with a Workspace is focusable; a Hero pane without one is read-only and always
renders the resting surface. No destination SHALL render a second Workspace box.

Grouped Music's Workspace SHALL carry the header `Tracks` (user direction 2026-09-15): the title in the
foam metadata role, then the same `▁` separator line and one blank row the Movie hero's overview box
puts under its overview text, then the track rows — all inside the Workspace box, whose surface and
bottom padding are unchanged. The header SHALL be omitted when the box has no room to keep a list row
under it.

Audiobookshelf podcast episodes are not constituent items: the podcast tab lists them as the panel's own
rows, and the selected episode's hero has no Workspace.

#### Scenario: Track list takes focus
- **WHEN** track selection becomes active in Wide grouped Music
- **THEN** the track Workspace box renders the accent-soft surface

#### Scenario: Grouped Music's track Workspace header
- **WHEN** a Music album's tracks render in the Wide Workspace
- **THEN** a `Tracks` header row paints above them in the foam metadata role
- **AND** the `▁` separator line and one blank row paint between the header and the first track row

#### Scenario: Episode list takes focus
- **WHEN** episode selection becomes active in Wide TV shows
- **THEN** the episode Workspace box renders the accent-soft surface, identically to Music

#### Scenario: Audiobookshelf chapters render in the Workspace
- **WHEN** an Audiobookshelf book is selected at Wide geometry
- **THEN** its chapters render in the one Workspace box with the selected-row bar
  used by TV and Music

#### Scenario: Read-only hero
- **WHEN** Home, Movies, home videos, Feeds or Audiobookshelf podcasts render at Wide geometry
- **THEN** the Hero pane has no Workspace and never renders the focused surface

### Requirement: Narrow inline hero has one form

The Narrow inline hero form is removed. In every non-Wide Library panel, the selected item SHALL remain an ordinary fixed-height canonical media row until the user opens the Library Hero overlay. No destination SHALL supply or paint a separate non-Wide Hero form.

#### Scenario: Narrow Movie and Narrow Audiobookshelf podcast

- **WHEN** a Movie and an Audiobookshelf podcast are each selected where the Wide Hero arrangement does not fit
- **THEN** both remain ordinary fixed-height media rows
- **AND** no Hero content replaces either row

#### Scenario: Narrow landscape artwork

- **WHEN** an item with landscape artwork is selected where the Wide Hero arrangement does not fit
- **THEN** its artwork does not render inside or beside the selected browser row
- **AND** opening its Library Hero overlay presents the artwork through the shared Hero content

### Requirement: Narrow workspace lists open only through the Library Hero overlay

The Narrow constituent-list selection modal is removed. In every non-Wide Library panel, constituent items SHALL be reachable through the selected parent's Library Hero overlay, whose Workspace uses the same canonical list and provider-owned content as Wide Hero. Audiobookshelf podcast episodes are not constituent items: the podcast tab lists them directly as its own rows, and Enter plays the selected episode.

#### Scenario: Narrow Audiobookshelf book

- **WHEN** an Audiobookshelf book is selected where the Wide Hero arrangement does not fit
- **THEN** no chapter rows render inside the browser list
- **AND** Enter opens the Library Hero overlay with its chapter Workspace focused

#### Scenario: Narrow Audiobookshelf podcast

- **WHEN** an Audiobookshelf podcast episode is selected where the Wide Hero arrangement does not fit
- **THEN** the episode remains an ordinary fixed-height media row in the browser list
- **AND** no filter pills or episode rows render inside a detail block or overlay
- **AND** Enter plays the episode

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

#### Scenario: Ineligible hero content

- **WHEN** the shown hero is not a Movie, has no declared Logo, has no loaded base artwork, or uses the shared placeholder
- **THEN** no Logo request or decoration changes that hero's presentation

### Requirement: The non-Wide library panel is the Wide browser pane without a Hero

In non-Wide geometry the Library panel SHALL compose the Wide browser pane's own rows — the Selector
row, the optional List controls row, and the list box — through the same browser-pane composition and
the same surface identities the Wide list pane uses, with the Hero pane absent. The non-Wide list box
SHALL be filled with the `LibraryPanel` pair and SHALL stripe with the `MainContentBox` pair, exactly
as the Wide Browser-pane list does; it SHALL NOT carry a body fill or a scrollbar-column fill of its
own. The non-Wide library column body — the panel placement, the Selector row's spacer row, the status
band's padding rows and the list's scrollbar column — SHALL resolve the fixed app backdrop
(`#2d353b`) in every focus state, as the Wide library column does. The status row inside the status
band SHALL remain the status bar's own surface. No non-Wide-specific library surface identity SHALL
exist. The non-Wide browser rail's focus input SHALL be the library panel's own focus bit alone: a
Workspace focused in Wide geometry SHALL NOT subtract the rail's focused surface after a transition
to non-Wide geometry, so a panel-focused non-Wide rail always paints one visible focus indicator. A
non-Wide list slot with no rows to spare SHALL keep its single row rather than collapsing to an empty
rect, so a very short panel still paints one row.

#### Scenario: A focused non-Wide library keeps the column backdrop

- **WHEN** the library panel renders in non-Wide geometry with the library panel focused
- **THEN** the panel placement, the Selector row's spacer row, the status band's padding rows and the
  list's scrollbar column all carry `#2d353b`
- **AND** the status row keeps the status bar's own surface

#### Scenario: The non-Wide list box matches the Wide Browser-pane list

- **WHEN** a non-Wide library list renders in either focus state
- **THEN** its list box carries the `LibraryPanel` fill for that state
- **AND** its alternating rows carry the `MainContentBox` fill for that state
- **AND** its scrollbar column carries the same fill the Wide Browser-pane list's column carries

#### Scenario: The non-Wide column body does not follow focus

- **WHEN** the library panel renders in non-Wide geometry with the library panel focused and without it
- **THEN** the column body's painted fill is identical in both frames

#### Scenario: A very short non-Wide panel keeps one row

- **WHEN** a non-Wide library panel is too short to afford the row-flow inset
- **THEN** its list slot keeps a single row instead of collapsing to an empty rect

#### Scenario: A Workspace focused in Wide does not darken the non-Wide rail

- **WHEN** the Hero Workspace holds focus in Wide geometry and the panel transitions to non-Wide
  geometry with the Library Hero overlay closed
- **THEN** a panel-focused non-Wide browser rail still paints its focused list-box fill
- **AND** the rail's focus input does not depend on the Workspace's focus bit

#### Scenario: Wide keeps its own column fill

- **WHEN** the library panel renders in Wide geometry, focused or not
- **THEN** the library column paints its fixed backdrop in both focus states
- **AND** no part of the Wide skeleton resolves a non-Wide body identity

#### Scenario: Mini follows the non-Wide presentation

- **WHEN** the terminal is narrow enough for the mini view
- **THEN** the library pane paints the same fixed backdrop and the same list-box pair as Narrow
- **AND** there is no Mini-specific paint bit or resting-only override
