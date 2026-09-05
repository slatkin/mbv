## ADDED Requirements

### Requirement: The hero-on-left left pane is a shared filled container

Every hero-on-left destination's left pane SHALL be painted by a single shared arrangement
primitive. That primitive SHALL derive the left pane's extent from the shared hero-on-left
presentation itself, fill that extent, and return the one shared content-inset rect that
destinations lay their hero content into. A destination SHALL NOT be able to supply a left-pane
extent of its own to the primitive.

The fill SHALL be unconditional: it does not depend on whether an item is selected, whether the
destination has hero data, which provider supplied the item, or how tall the painted content is.
A destination SHALL NOT resize, re-derive, clamp, or conditionally skip the fill, and SHALL NOT
apply a destination-specific content inset. The left pane's content inset SHALL be the single
shared pane inset used by every hero-on-left destination; no destination defines its own.

The status-row reserve remains owned solely by the shared hero-on-left presentation: the filled
left pane SHALL bottom out exactly one row above the status bar on every destination, in every
selection state, at every Wide geometry. Destinations SHALL NOT paint a separate strip below the
left pane to simulate that reserve.

#### Scenario: Every hero-on-left destination fills its left pane

- **WHEN** any hero-on-left destination renders at Wide geometry
- **THEN** every cell of its left pane carries the shared hero-pane surface
- **AND** no cell of the left pane shows the right column's backdrop surface

#### Scenario: Nothing is selected

- **WHEN** a hero-on-left destination renders at Wide geometry with no selected item, no hero
  data, or an empty library
- **THEN** its left pane is still filled to its full extent
- **AND** only the pane's content is absent, not the pane

#### Scenario: Hero content is shorter than the pane

- **WHEN** a hero-on-left destination's hero content occupies fewer rows than the left pane
- **THEN** the content is anchored to the top of the pane's content inset
- **AND** the pane's painted extent is unchanged by the content's height

#### Scenario: The left pane bottoms out one row above the status bar

- **WHEN** any hero-on-left destination renders at Wide geometry
- **THEN** the filled left pane's bottom edge is exactly one row above the status bar
- **AND** no destination paints an additional row of any surface below the left pane

#### Scenario: A destination attempts to supply its own pane extent

- **WHEN** a destination has computed or mutated a left-pane rect of its own
- **THEN** that rect cannot be used to paint the pane
- **AND** the painted extent is the one the shared hero-on-left presentation produced

#### Scenario: Every destination uses one content inset

- **WHEN** two hero-on-left destinations render at the same Wide geometry
- **THEN** their hero content begins at the same offset from their left pane's edges

### Requirement: Left-pane focus treatment follows one rule

A hero-on-left left pane SHALL render the focused surface treatment when, and only when, that
pane hosts a workspace that can hold focus and that workspace currently holds focus. This rule
SHALL be resolved by the shared pane primitive, not by each destination: a destination SHALL
declare only which of the two closed kinds its left pane is — a read-only hero, or a focusable
workspace together with that workspace's current focus state — and the primitive SHALL derive
the surface treatment from that declaration. A destination SHALL NOT be able to declare a
read-only pane as focused, and SHALL NOT select a surface treatment directly.

A left pane whose content is a read-only projection of the right rail's selection SHALL always
render the resting surface treatment, regardless of whether the right panel or the right rail is
focused.

#### Scenario: A focusable left workspace holds focus

- **WHEN** a hero-on-left destination whose left pane hosts a focusable workspace has focus in
  that workspace
- **THEN** its left pane renders the focused surface treatment

#### Scenario: A focusable left workspace does not hold focus

- **WHEN** the same destination's focus is in the right rail, or the right panel is unfocused
- **THEN** its left pane renders the resting surface treatment

#### Scenario: A read-only hero pane never renders as focused

- **WHEN** a destination whose left pane is a read-only hero renders in any focus state
- **THEN** its left pane renders the resting surface treatment

### Requirement: Hero overview and media-list boxes have distinct ownership

Every hero-on-left destination SHALL paint a recessed overview main-content box through the
shared primitive, even when its description is empty. The overview box carries only the Hero
text description and has one primitive-owned internal padding value.

A destination with structured episode, track, or chapter content SHALL additionally paint a
separate recessed media-list box. The shared arrangement owns both box and viewport rects; the
destination component owns its embedded `WideMediaList<Target>`, including rows, target identity,
cursor, scroll, selection, intent translation, and hit geometry. A Hero SHALL NOT carry a
structured listing or mutable list state. The destination SHALL NOT define its own box geometry
or surface.

#### Scenario: TV presents overview before episodes

- **WHEN** a selected Series renders at Wide geometry
- **THEN** title and ordered metadata render first
- **AND** one blank row separates the metadata from the overview main-content box
- **AND** a separate media-list box follows the overview box
- **AND** season pills are parent chrome above the episode `WideMediaList` viewport

#### Scenario: A structured workspace renders its media list

- **WHEN** TV, Music, or Audiobookshelf renders selected structured content
- **THEN** its parent-owned `WideMediaList` renders inside the separate media-list box
- **AND** canonical row, scroll, selection, and hit geometry are preserved

#### Scenario: The overview is empty

- **WHEN** a selected item supplies no description text
- **THEN** the overview main-content box is still painted
- **AND** its absence is never used to signal an empty payload

#### Scenario: Two overview payloads are compared

- **WHEN** two description payloads render in the overview main-content box at the same pane width
- **THEN** both begin at the same offset from the box's edges

## MODIFIED Requirements

### Requirement: Each screen is assigned one wide arrangement

Every hero-bearing right-panel browse surface SHALL use hero-on-left for its wide presentation. This includes Home, Movies, TV shows, grouped Music, Emby podcasts, Emby home videos, Audiobookshelf podcasts, Audiobookshelf books, and Feeds. A read-only selected-item hero SHALL remain a projection of the right-hand browser selection. A surface whose left detail workspace contains episodes, tracks, or chapters MAY expose that existing interactive content without changing the shared placement rule. No hero-bearing browse surface SHALL declare a separate detail placement or a surface-specific responsive placement.

#### Scenario: Wide read-only hero surface
- **WHEN** Home, Movies, an Emby podcast library, an Emby home-video library, or Feeds is displayed with wide geometry
- **THEN** the selected-item hero renders in the left pane
- **AND** the right rail remains the only focusable browser pane

#### Scenario: Wide interactive detail surface
- **WHEN** TV shows, grouped Music, an Audiobookshelf podcast library, or an Audiobookshelf book library is displayed with wide geometry
- **THEN** the selected item's persistent detail workspace renders in the left pane
- **AND** the single-column catalog browser renders in the right rail
- **AND** existing episode, track, or chapter focus behavior remains available where that surface already provides it

#### Scenario: Movies is displayed at a wide width
- **WHEN** the dedicated Movies library meets the wide geometry conditions
- **THEN** the selected-media hero is on the left
- **AND** the letter-range pills and one-column Movies list are in the right rail

#### Scenario: TV shows is displayed at a wide width
- **WHEN** the TV shows library meets the wide geometry conditions
- **THEN** the selected Series detail, season pills, and persistent episode preview are on the left
- **AND** TV letter-range pills and the one-column Series list are in the right rail

#### Scenario: Feeds is displayed at a wide width
- **WHEN** Feeds meets the wide geometry conditions
- **THEN** the selected entry's hero is on the left
- **AND** group and watched selectors plus the one-column entry browser are in the right rail

#### Scenario: Audiobookshelf podcast library is displayed at a wide width
- **WHEN** an Audiobookshelf podcast library meets the wide geometry conditions
- **THEN** the selected show and its filtered episode workspace are on the left
- **AND** the one-column podcast-show browser is in the right rail

#### Scenario: Audiobookshelf book library is displayed at a wide width
- **WHEN** an Audiobookshelf book library meets the wide geometry conditions
- **THEN** it renders the hero-on-left arrangement matching grouped Music at the same dimensions

#### Scenario: Hero-bearing surface leaves wide geometry
- **WHEN** any hero-bearing browse surface no longer meets the shared wide geometry conditions
- **THEN** it renders its selected detail inline in a single-column browser
- **AND** no separate fallback is used

#### Scenario: Wide TV shows has an interactive left hero
- **WHEN** TV shows meets the wide geometry conditions
- **THEN** Series browsing remains on the right and the interactive episode workspace remains on the left

#### Scenario: Wide Movies has its selected-media hero
- **WHEN** Movies meets the wide geometry conditions
- **THEN** its selected-media hero renders on the left and its one-column browser on the right

#### Scenario: TV shows falls below the breakpoint
- **WHEN** TV shows does not meet the wide geometry conditions
- **THEN** selected Series detail replaces its ordinary row in its one-column browser

#### Scenario: Movies falls below the shared breakpoint
- **WHEN** Movies does not meet the wide geometry conditions
- **THEN** selected Movie detail replaces its ordinary row in its one-column browser

#### Scenario: Home videos is displayed at a wide width
- **WHEN** an Emby home-video library meets the wide geometry conditions
- **THEN** it renders hero-on-left with a one-column right-rail browser

#### Scenario: Audiobooks is displayed at a wide width
- **WHEN** an Audiobookshelf book library meets the wide geometry conditions
- **THEN** it renders hero-on-left matching grouped Music

### Requirement: Hero-on-left presents up to two focusable panes

The hero-on-left arrangement SHALL present up to two panes, of which at most one is focused, and
only while the right panel itself is focused. A screen with a read-only hero pane — Home, the
wide Movies library, and Feeds — SHALL expose only its right-hand list as focusable content. A
screen whose left pane hosts an interactive workspace — the wide TV shows library, grouped
Music, an Audiobookshelf book library, and an Audiobookshelf podcast library — SHALL expose both
the right-hand list and that left workspace as focusable content. While right-rail browsing is
active, the left pane SHALL remain a projection of the selected item; when the left workspace's
selection is active, the left pane SHALL receive focus.

#### Scenario: Wide Movies has Library focus

- **WHEN** the wide Movies library is displayed and the Library panel has focus
- **THEN** the right-hand Movies list is the focused pane
- **AND** the left selected-media hero remains read-only and does not become a second focus target

#### Scenario: Wide TV shows has Series-list focus

- **WHEN** the wide TV shows library is displayed and episode selection is inactive
- **THEN** the right-hand Series list is the focused pane
- **AND** the left Series and episode workspace renders as an unfocused preview

#### Scenario: Wide TV shows has episode focus

- **WHEN** episode selection is active in the wide TV shows library
- **THEN** the left-hand episode workspace is the focused pane
- **AND** the right-hand Series list renders its unfocused treatment

#### Scenario: An Audiobookshelf podcast workspace takes focus

- **WHEN** episode selection is active in a wide Audiobookshelf podcast library
- **THEN** the left-hand episode workspace is the focused pane and renders the focused surface
  treatment
- **AND** the right-hand show list renders its unfocused treatment

#### Scenario: Focus moves between panes

- **WHEN** the user moves focus within a hero-on-left screen that has focusable hero content
- **THEN** exactly one pane is focused and the other renders its unfocused appearance

#### Scenario: The right panel is unfocused

- **WHEN** the right panel is not focused
- **THEN** neither pane of a hero-on-left screen renders as focused
