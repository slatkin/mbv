# Invariant 30 — Shared UI is never bent to fit a consumer

**Scope:** every shared UI piece that more than one screen paints: the GUI
Library Hero frame (`LibraryHeroFrame`, `LibraryHeroArt`,
`LibraryHeroActions`), and the TUI's shared Panels, slots, and render
components (`docs/architecture/tui-frontend.md`).

## The invariant

1. A screen that adopts a shared piece paints the look the piece defines. A
   shared piece exists to give every screen one look, so the look is the
   contract.
2. The consumer maps its data onto the piece's shape. The piece is never given
   an escape hatch (an optional band, a flag that drops a part) so that a
   consumer with thin data can paint something else. A piece that can drop its
   header is two layouts under one name.
3. If the data cannot fit the shape, the author asks the user before the
   piece changes. The question names what the user will see, in visual terms.
   "An optional header band" is an implementation phrase and is not a valid
   question.
4. A look change is verified by looking: the screen next to its siblings.
   Passing type checks and unit tests prove attributes and commands, not
   appearance. Do not report a look-sharing change as done until someone has
   seen it.
5. The Hero header's big title is always the largest unit's name: the podcast,
   the feed, the series, the artist. The block beside the art holds the
   selected smaller unit (episode, entry, season, album). A movie or a book is
   its own largest unit.

## Why it matters

The Hero frame exists so Music, Shows, Podcasts, Audiobooks, Feeds, and Movies
look the same (add-gui-library-panel design D15). In `400d0fd28` the frame's
`header` became optional so that Podcasts, Audiobooks, and Feeds, whose facts
describe one row, could skip it. The three Heroes then looked nothing like
Music and TV. The checks passed, and the work was reported done. `ecfa33595`
reversed it: the header is required, and each consumer fills it. The TUI work
had the same failure earlier: a shared surface changed to suit a screen, then
a screen-specific copy.

## How the code maintains it today

- `LibraryHeroFrame` takes a required `header`. `heroLayout` in
  `$lib/library/hero.ts` maps a Hero's facts onto the header (the largest unit
  as the title) and the block (the selected unit), and Podcasts, Audiobooks,
  Feeds, and Shows Latest use it. The core carries the largest unit as
  `parent` on the Podcasts, Feeds, and Shows Hero frames.
- `LibraryHeroActions`, `LibraryWorkspaceTable`, `LibraryPillButton`,
  `LibraryItemRow`, `LibraryLetterHeading`, and `LibraryVirtualList` are the
  one painter of their part: the Hero buttons, the Workspace list band (Music's
  tracklist, Shows' episodes, Audiobooks' chapters, Feeds' entries), the pills
  (top row and season pills), the flat list row, the letter heading, and the
  scroller (Music's list too).
- One test per frame consumer asserts that the header kicker and title paint.

## Where it currently fails / how it could regress

Nothing mechanical enforces rules 2 to 4.

- **Optional props on a shared piece.** Each new optional prop that removes a
  band, or swaps a layout, is the same escape hatch. Review these as findings.
- **Looks that tests cannot see.** Spacing, size, and band order are not
  asserted. A consumer can match the attributes and still differ in look.
- **Destinations not yet on the shared piece.** Movies (`LibraryEmbyHero`) and
  Home still paint through `LibraryHeroSummary`, a second layout. Movies are
  held until the user lifts the hold, and Home is redone after the GUI
  migration. Home's list and its tree rows (Shows, Music) keep their own markup.
- **Music has no loading line.** Its tracklist has no loading signal from the
  core, so the Workspace table paints none for it.
