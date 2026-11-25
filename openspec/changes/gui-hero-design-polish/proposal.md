# Proposal

## Why

The GUI Client's Hero looks plain next to the "mbv detail pane polish" artboards (`Movie — proposed (wide)`, `Album — proposed (wide)`). The artboards give the movie and album Heroes one set of design parts. The user wants every Hero to use these parts, so that all destinations look the same. The fonts do not change.

## What Changes

- Every Hero (Movie and other plain Emby items, Shows, the Music album, Home, Audiobooks, Podcasts, and Feeds) gets the design parts of the artboards:
  - an eyebrow line with a leading dot above the title;
  - the short facts as a row of pills under the title;
  - the artwork in an inset frame;
  - the labelled facts as key/value rows beside the artwork, under the action row;
  - a section header with a rule for each band below the detail block (for example, Overview, Cast, Tracks, Episodes);
  - rounded, striped list rows;
  - a blurred glow behind the header band, from the Hero's own artwork.
- **No font changes.** Each text keeps its current family, size, and weight. The one exception is the bold title of the row that plays now (below).
- **Pills and rows rule, for every Hero.** Pills hold the short facts: the year, the genres, the runtime or duration, the track count, and the IMDb link. The key/value rows hold the labelled facts: for example, Director, Released (the full date), Studio, Author, Network, Published. A fact shows in one of the two places, not in both. A Released row can show the full date while a pill shows the year. The movie Hero has no Runtime row, because the runtime pill shows it.
- **Images from the Service.** The glow is made from the Hero's own artwork: for a movie that is the Emby backdrop, and for the others it is the cover or art that the Hero already shows (album cover, artist image, book cover, podcast art, feed or entry art). A Hero with no artwork shows no glow. The cast entries of an Emby Hero show the person's photo from Emby. An entry with no photo shows the person's initials. To get the photos, the Emby item model keeps each person's `Id` and `PrimaryImageTag`.
- **Cast as a grid.** The Emby credits become a Cast band: a two-column grid of the photo or initials, the name, and the role. The grid wraps to one column when the Hero is narrow. The director moves out of the credits into the Director row.
- **No progress in any Hero.** The Hero shows no progress bar, no resume position, and no time left, in every destination. The queue handles playback. Play still resumes from the stored position.
- **Music album Hero.**
  - The artist facts line (album count, the artist's track count, and the artist's total time) is removed from the album Hero. The album count shows only on the artist page (`gui-music-artist-page`).
  - The album's track count and duration are pills, in the same style as the movie Hero's pills.
  - The layout does not change otherwise: Shuffle artist stays at the top right, and the vinyl disc behind the cover is for the album Hero only.
- **The row that plays now.** In a Hero list (tracks, episodes, chapters, entries), the row that plays now gets a fill, a bold title, and a bars mark in place of its number. **BREAKING (spec):** this replaces the rule "a highlight marks focus only, so the playing row SHALL NOT paint a green fill", and the current play glyph beside the title.
- **The artist page** (from `gui-music-artist-page`, which lands before this change) gets the same parts: its header facts (album count, year span, track count, total time) become pills, its album groups sit in the Tracks section, and it shows no progress.
- The Narrow arrangement keeps its current wrap: the text block drops below the artwork when the Hero is too narrow for two columns.

### Decisions made while planning

- The scope is every Hero, not only the movie and album Heroes.
- Both Emby images (the backdrop glow and the cast photos) are in scope, and the glow applies to every Hero.
- The seekbar of the movie artboard, its resume position, and its time left are not built. The progress leaves every Hero.
- One pills-and-rows rule applies to every Hero. A fact is not shown two times.
- The album count belongs to the artist page only.
- The row that plays now uses the artboard's look, and the prior spec rule is replaced.
- `gui-music-artist-page` lands before this change, and this change restyles the artist page. `gui-emby-row-played-marks` lands after this change.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `gui-client`: the requirements "The Hero shows the picked item with its actions" (the album Hero's header facts, the row that plays now) and "Every Hero is one frame with the largest unit as its title" (no progress in the detail block, the pills and rows rule, the Cast band, the glow, and the Movie and Home Heroes on the frame), and "Library lists and Workspace tables paint through shared parts" (the Hero's fact pills as a shared part, and the look of the row that plays now) change.

## Impact

- `crates/mbv-emby-model`: `EmbyPerson` gets the person `Id` and `PrimaryImageTag`.
- `crates/mbv-emby`: the parse of `People` keeps those two values.
- `protocol/ctrl/v10`: `EmbyPerson` is in the ctrl schemas. The two fields are additive (`#[serde(default)]`), so the schemas are regenerated and no version bump is needed.
- `crates/mbv-client-core`: none expected. The TUI keeps its Hero facts, its progress, and its credits table. The GUI splits the shared facts into pills and rows, and drops the progress, in its own projection.
- `crates/mbv-gui/src/bridge/frame/library`: the Hero frames carry the pills, the key/value rows, the glow image, and the cast photos.
- `crates/mbv-gui/src/gui_core`: the art requests for the glow image and the person photos.
- `crates/mbv-gui/src/window/components`: `LibraryHeroFrame`, `LibraryHeroDetail`, `LibraryHeroArt`, every destination Hero (`library_*_hero.rs`), `LibraryTracklist`, and the Workspace lists.
- `crates/mbv-gui/src/window/library/hero.rs`: the album Hero's header facts and the facts layout.
- The artist page that `gui-music-artist-page` adds (`LibraryHero`'s artist branch, `LibraryArtistTracklist`, `artist_page_facts`).
- `docs/architecture/gui-client.md`: the Hero frame section.
