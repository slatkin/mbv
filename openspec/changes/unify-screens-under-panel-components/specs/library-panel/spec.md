## Purpose

Defines the one Library panel skeleton that every library destination renders through, in its Wide and
Narrow forms, so that destinations can differ only in the content they supply and never in how the
panel is composed.

## ADDED Requirements

### Requirement: Every library destination renders through one Library panel skeleton

Home, Movies, Emby home videos, TV shows, grouped Music, Audiobookshelf Books, Audiobookshelf Podcasts
and Feeds SHALL each render through the same Library panel: the Wide library panel when the shared
Wide width and minimum-height conditions are met, and the Narrow library panel otherwise. The panel
SHALL own every row, pane, fill, border, gap, inset and placeholder position. A destination SHALL
supply only typed content for the panel's slots and SHALL NOT paint outside a slot, add a slot, or
select an alternative skeleton, pane order, fill, inset or border.

A slot a destination does not fill SHALL render as absent in the same way on every destination. Two
destinations supplying the same kind of content SHALL render it identically at the same geometry.

#### Scenario: Two destinations with the same slot content
- **WHEN** two library destinations supply the same kind of content for the same slots at the same
  terminal size
- **THEN** their panes, rows, fills, borders and insets occupy the same cells with the same surfaces

#### Scenario: A destination has no content for an optional slot
- **WHEN** a destination supplies no List controls row content
- **THEN** no List controls row renders and the rows below move up exactly as on every other
  destination without that content

#### Scenario: The breakpoint is crossed
- **WHEN** the terminal crosses the shared Wide conditions in either direction
- **THEN** every library destination switches between the Wide and Narrow library panel at the same
  width and height

### Requirement: Emby screens are the reference presentation

Where library destinations differ in presentation, the Emby destinations (Home, Movies, home videos,
TV shows, grouped Music) SHALL define the Library panel's presentation, and Audiobookshelf and Feeds
destinations SHALL conform to it. An Audiobookshelf or Feeds presentation SHALL NOT be a reason to
change a slot, and SHALL NOT remain as a destination-only difference.

#### Scenario: An Audiobookshelf screen differs from the Emby screens
- **WHEN** an Audiobookshelf destination renders a slot differently from the Emby destinations
- **THEN** the Audiobookshelf destination is non-conforming and renders the Emby presentation

### Requirement: The Browser pane has one Selector row and one List controls row

The Browser pane (the Wide panel's left pane, and the whole Narrow panel) SHALL present, top to bottom:
at most one Selector row (a single pill bar followed by the panel's spacer row), at most one List
controls row, and the list box. The Selector row carries the destination's primary browse selector
(letter ranges, music groups, surname buckets, Home sections, feed groups). The List controls row
carries secondary list controls: the Feeds Watched filter and the Emby home-video item count. No
destination SHALL render a second pill bar.

#### Scenario: Feeds renders its selectors
- **WHEN** the Feeds destination renders with subscriptions available
- **THEN** its feed-group pills render in the Selector row
- **AND** its All / Watched / Unwatched filter renders in the List controls row
- **AND** no second pill bar renders

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

- Music (albums and tracks) and podcasts (Audiobookshelf podcast shows and episodes, and podcast Feeds
  entries) SHALL always use Square artwork.
- Every other item SHALL use the most preferred artwork shape its provider declares available, in the
  order Landscape, then Square, then Portrait.
- An item with no available artwork SHALL use Landscape (Square for Music and podcasts) with the
  shared placeholder.

Availability SHALL be decided from provider metadata before any image is fetched, so the header type
does not change when the image arrives. The image the policy chooses SHALL be the same image for that
item in the Wide header and the Narrow inline hero.

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

#### Scenario: A podcast is selected at Wide geometry
- **WHEN** an Audiobookshelf podcast show, or a podcast Feeds entry, is the selected item
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

### Requirement: The overview is a Main content box below the Hero header

When the shown item has overview text, the Hero pane SHALL render it in one Main content box directly
below the Hero header, at the shared padding, for every header type. When the item has no overview
text, no overview box SHALL render and the Workspace (if any) SHALL move up.

#### Scenario: A Movie with an overview
- **WHEN** a Movie with overview text is selected at Wide geometry
- **THEN** the overview renders inside a Main content box below the header

#### Scenario: An album without an overview
- **WHEN** a Music album without overview text is selected at Wide geometry
- **THEN** no overview box renders and the track Workspace follows the header

### Requirement: Wide Workspaces are one Selector row plus one list box

A destination whose item has constituent items (TV seasons and episodes, Music tracks, Audiobookshelf
chapters, Audiobookshelf podcast episodes) SHALL present them in a Workspace below the overview: an
optional Selector row (season pills, episode played-state filter) followed by one Main content box
holding the canonical list. The Workspace box SHALL render the accent-soft surface while its list holds
focus and the backdrop surface otherwise. Its selected row SHALL use the owning-surface selected-row
treatment. A Hero pane with a Workspace is focusable; a Hero pane without one is read-only and always
renders the resting surface. No destination SHALL render a second Workspace box.

#### Scenario: Track list takes focus
- **WHEN** track selection becomes active in Wide grouped Music
- **THEN** the track Workspace box renders the accent-soft surface

#### Scenario: Episode list takes focus
- **WHEN** episode selection becomes active in Wide TV shows
- **THEN** the episode Workspace box renders the accent-soft surface, identically to Music

#### Scenario: Audiobookshelf chapters render in the Workspace
- **WHEN** an Audiobookshelf book is selected at Wide geometry
- **THEN** its chapters render in the one Workspace box with the owning-surface selected-row treatment
  used by TV and Music

#### Scenario: Read-only hero
- **WHEN** Home, Movies, home videos or Feeds renders at Wide geometry
- **THEN** the Hero pane has no Workspace and never renders the focused surface

### Requirement: Narrow inline hero has one form

In the Narrow library panel, the selected item's inline hero SHALL render one form for every
destination, derived by the panel from the same hero content the Wide panel uses: the image chosen by
the artwork policy right-aligned with a size derived from its aspect, and title, the metadata rows
(coloured as in Wide) and overview wrapping around it, continuing at full width below the image. The
Wide Hero header types SHALL NOT apply in Narrow, and no destination SHALL supply a separate Narrow
hero. The inline hero SHALL NOT contain Selector rows, List controls,
or constituent-item rows.

#### Scenario: Narrow Movie and Narrow podcast
- **WHEN** a Movie and an Audiobookshelf podcast are each selected in the Narrow panel
- **THEN** both inline heroes render their image right-aligned with text wrapping around it

#### Scenario: Narrow landscape artwork
- **WHEN** the selected item's image is a 16:9 thumbnail in the Narrow panel
- **THEN** it renders right-aligned with wrap-around text, not in a right-half meta column

### Requirement: Narrow workspace lists open only as the selection modal

In the Narrow library panel, a selected item's constituent items SHALL be reachable only through the
shared constituent-list selection modal, for every destination.

#### Scenario: Narrow Audiobookshelf book
- **WHEN** an Audiobookshelf book is selected in the Narrow panel
- **THEN** no chapter rows render inside its inline hero
- **AND** Enter opens the selection modal listing its chapters

#### Scenario: Narrow Audiobookshelf podcast
- **WHEN** an Audiobookshelf podcast show is selected in the Narrow panel
- **THEN** no filter pills or episode rows render inside its inline hero
- **AND** Enter opens the selection modal listing its episodes
