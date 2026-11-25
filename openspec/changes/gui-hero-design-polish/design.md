# Design

## Context

See `proposal.md` for the why and the decisions, and `specs/gui-client/spec.md` for the behaviour. The artboards are in `canvas/` (`movie-proposed-wide.png`, `album-proposed-wide.png`, and `movie-current.png` for comparison). This design is for the GUI Client only. The TUI does not change.

`gui-music-artist-page` lands before this change. The facts below are the code on `main` plus that change's branch (`feat/gui-music-artist-page`).

- Every Hero except Home paints through `LibraryHeroFrame` (`crates/mbv-gui/src/window/components/library_hero_frame.rs`): a header band (kicker, title, one facts line, an optional header action), the detail block `LibraryHeroDetail` (art plus a text column with the name, the meta rows, the actions, and the overview), and a list band (the children). Home paints through `LibraryHeroSummary`, which also uses `LibraryHeroDetail`.
- The GUI Hero facts are the shared TUI producers' plain meta rows (`crates/mbv-ui-model/src/library_hero/producers.rs`: `hero_content_emby`, `hero_content_series_with_episode`, `hero_content_abs_book`, `hero_content_abs_episode`, `hero_content_feed`, `hero_content_queue`). A row is a list of typed segments (`MetaKind`: plain, date, time, count, label, ...). A row has no label word. The frames carry `meta_rows`, `duration_row`, and `progress_row`. The window splits them with `hero_layout` (`crates/mbv-gui/src/window/library/hero.rs`): the plain rows join on the header facts line, and the duration and progress rows go into the block.
- The Emby Hero carries `credits` (director first, then the rest of `EmbyItem::people`) and `imdb_url`. It paints the credits as a striped name and role table and the IMDb link as the header action.
- `EmbyPerson` (`crates/mbv-emby-model/src/lib.rs`) holds `name`, `role`, and `kind`. `parse_people` (`crates/mbv-emby/src/types_parsing.rs`) reads `Name`, `Role`, and `Type`. Emby's `BaseItemPerson` also carries `Id` and `PrimaryImageTag`, and a person's photo is `/Items/{Id}/Images/Primary`, the same endpoint shape as an item's Primary image. `EmbyPerson` is in the ctrl v10 schemas (`protocol/ctrl/v10/schema/`).
- The GUI loads images through `mbv-art://localhost/<path>`. `ArtPath` (`crates/mbv-gui/src/art.rs`) has `Card(key)` and `Placeholder`. `CoreThread::answer_art` (`crates/mbv-gui/src/core_thread/tick_host.rs`) answers a card with the bytes that the image cache holds (`gui_art_source`). The GUI keeps no decoded pixels (`ImageDecode::SourceOnly`). `App::fetch_card_image(cache_key, item_id, series_id, types)` fetches an Emby image into the cache.
- The window renderer is Blitz with the `vello-hybrid` backend (`dioxus-native =0.8.0-alpha.3`, `blitz-paint =0.3.0-beta.4`). `mbv-gui` already depends on `image` 0.25 with `png`, `jpeg`, and `webp`.
- `LibraryWorkspaceTable` paints the playing row with the heading tone and a play glyph beside the title (`title_tone`, `title_weight`, `PlayCluster`). `LibraryTracklist`, the Shows episodes, the Audiobooks chapters, the Feeds entries, and the artist page's `LibraryArtistTracklist` all paint through it.
- The fonts are IBM Plex Sans and JetBrains Mono (`crates/mbv-gui/assets/app.css`).

## Goals / Non-Goals

**Goals:**

- One set of Hero parts in the window, used by every Hero, so that a fix lands in all destinations.
- One GUI-owned split of each Hero's facts into pills and labelled rows, built from the source items.
- The glow and the cast photos without decoded images on the core thread.

**Non-Goals:**

- No TUI change. The shared producers, the TUI Hero, its progress, and its credits table stay as they are.
- No new fact that the GUI does not show now, except the movie's Studio row and Released row, which the artboard asks for. No Narrator or Series row: the data does not carry them.
- No change to a text string, a date format, or a duration format. A pill shows the text that the Hero shows now (for example `01:37:32`, not the artboard's `1h 37m`).
- No change to the list rows' own played, unplayed, and resume marks.
- No cast for Shows. Today only a movie carries credits.

## Decisions

### D1. The GUI builds its own Hero facts from the source items

A new module `crates/mbv-gui/src/gui_core/gui_library/hero_facts.rs` builds `GuiHeroFacts { pills: Vec<GuiHeroPill>, rows: Vec<GuiHeroFactRow> }`:

- `GuiHeroPill { segments: Vec<MetaSegment>, link: Option<String> }`: one short fact, with its typed segments (so the type colour stays as now) and an optional URL.
- `GuiHeroFactRow { label: GuiHeroFactLabel, value: Vec<MetaSegment> }`: `GuiHeroFactLabel` is an enum `Director | Released | Studio | Network | Author | Published`. The window paints the label word from the enum.

One builder for each source kind, each using the formatters of `mbv_ui_model::ui_util` (`fmt_duration_hms`, `format_release_date`, `fmt_publish_date`) so that the text is the same as today:

| Source | Pills, in order | Rows, in order |
|---|---|---|
| Emby `Movie` | year (`production_year`), each genre, runtime, IMDb (link) | Director (the directors, joined with `, `), Released (`premiere_date`), Studio (`studios`, joined with `, `) |
| Other Emby item (episode, video) | year, runtime | Released |
| Emby series (Shows) | year range (else nothing), each genre, season count | Network (first studio) |
| Audiobookshelf book | duration | Author |
| Audiobookshelf episode | duration | Published |
| Feed entry | duration | Published (when the entry has a date) |
| Twitch | none | none |

`hero_facts_queue(&QueueItem)` dispatches like `hero_content_queue`, so Home and Shows New use the same builders. A fact with no value is left out. The Music album and the artist page keep their own facts code in the window (D5), because their frames already carry typed totals.

The frames `GuiEmbyHeroFrame`, `GuiHomeHeroFrame`, `GuiShowsHeroFrame`, `GuiAudiobooksHeroFrame`, `GuiPodcastsHeroFrame`, and `GuiFeedsHeroFrame` lose `meta_rows`, `duration_row`, `progress_row`, and (Emby) `imdb_url`, and get `facts: GuiHeroFactsFrame` (the bridge mirror of `GuiHeroFacts`). The title, the overview, the art, the art shape, and the Workspace stay.

Alternatives considered:
- Classify the producers' plain rows by `MetaKind`. Rejected: a row has no label, and an author and a show name are both plain text.
- Add labels to the shared `HeroFacts`. Rejected: it changes the TUI's model for a GUI-only look.

### D2. One frame for every Hero, with the parts in one place

`LibraryHeroFrame` becomes the only Hero layout. Home moves onto it and `LibraryHeroSummary` is deleted. The frame paints, top to bottom:

1. The glow layer (D4), behind the header band only.
2. The header band: the kicker as the eyebrow with a leading dot, the title, and the fact pills row (`LibraryHeroPills`). The header action stays (Shuffle artist). The IMDb header action is deleted, because the IMDb pill is the link.
3. The detail block (`LibraryHeroDetail`): the art in an inset frame, and the text column with the block's eyebrow and name lines (when a parent took the title, and the album's year and label line), the album's pills (D5), the action row, and the fact rows (`LibraryHeroFactRows`). The overview leaves the block.
4. The bands, each under a section header with a rule (`LibraryHeroSection`, a new component): Overview (when the item has one), Cast (when the item has cast, D3), then the destination's list band (Tracks, Episodes, Chapters, Entries) as the children. The destination passes the list band's title.

`HeroBlock` loses `facts` and `meta` and gets `rows: Vec<GuiHeroFactRow>` and `pills: Vec<GuiHeroPill>` (the block pills, used by the album). `HeroHeader.facts` becomes `pills`. `hero_layout`, `HeroLayoutFacts`, `HeroMetaRow`, `HeroMetaRole`, `hero_meta_rows`, and `join_meta_rows` are deleted with their tests, because no progress or duration role remains. The overview moves to a frame prop.

The detail block keeps its wrap rule. A landscape art box takes the larger share of the row beside the text column (the artboard's proportion) and drops above the text when the Hero is too narrow. Square and portrait art keep their box. The album's square art gets the vinyl disc behind it through a `disc: bool` prop on `LibraryHeroArt`, which only the album branch sets.

Fonts: each new part takes the font of the text it replaces. The eyebrow and the section headers use the kicker's label font. The pills use the font of today's facts line. A row's label uses the label font, and its value uses today's meta row font. A cast name and role use today's credit name and role fonts. Sizes, colours, radii, and spacing come from the artboards. New colour values become `--canvas-*` tokens in `crates/mbv-gui/assets/app.css`, and `make gui-css` regenerates `app.generated.css`.

### D3. Cast photos from Emby

- `EmbyPerson` gets `id: String` and `primary_image_tag: String`, each `#[serde(default)]`. `parse_people` reads `Id` and `PrimaryImageTag`. The item requests already ask for `People`, and Emby returns both fields in each person by default, so no request changes.
- The ctrl v10 schemas list `EmbyPerson`. Per `docs/recipes.md` ("Add a ctrl protocol variant"), a `#[serde(default)]` field is additive: no version bump. No ctrl consumer reads the fields (the GUI reads people from its own Emby library store), so no capability is added and no `ctrl-protocol` requirement changes. The schemas are regenerated with the conformance bless.
- The Emby projection (`emby_projection.rs`, and Home through the queue dispatch) builds `cast: Vec<GuiHeroCastMember { name, role, initials, photo: Option<GuiLibraryArtwork> }>` from the people that are not directors, in the order Emby gives. `role` is the person's role, else the person's kind, as now. `initials` is the first letter of the first two words of the name, upper case. `photo` is `None` when the person has no `id` or no tag. Otherwise the projection calls `fetch_card_image(emby_card_cache_key(id, ""), id, "", &["Primary"])` and reads the cache with `resolve_gui_art`, as `home_hero_artwork` does.
- The window paints the cast as a two-column grid (`LibraryHeroCast`, a new component) that wraps to one column when narrow. A member paints the photo in a round box, else the initials in that box.

Alternative considered: initials only. Rejected by the user.

### D4. The glow is a pre-blurred image from a new art path

- `ArtPath` gets `Glow(key)` for `glow/<percent-encoded key>`, with `glow_path(cache_key)` beside `card_path`.
- `CoreThread::answer_art` answers `Glow(key)` by reading the cached source bytes (`gui_art_source`) on the core thread, and then moving the bytes and the `reply` to a new thread (`std::thread::Builder` named `mbv-art-glow`). That thread decodes with `image::load_from_memory`, shrinks the image with `imageops::thumbnail` to at most 64 px on its long side, blurs it with `imageops::blur` at sigma 4, encodes it as PNG, and calls `reply` with `image/png`. A decode or encode failure answers `Missing`. No memo: the window keeps the image while the Hero shows it, and a new Hero is a new request.
- The frame takes `glow: Option<GuiArtRef>`. Each Hero passes the art it already shows, mapped to its glow path (`GuiArtRef::Image` gives a glow source; `Placeholder` and `Loading` give `None`). The frame paints the image to cover the header band's area, under a fade to the pane's background, behind the text.

Alternative considered: CSS `filter: blur()` on the art in the window. Blitz parses it and the `vello-hybrid` backend maps a Gaussian blur, but a large-radius blur over a large area runs on every repaint, and filter support in this backend is new. A 64 px pre-blurred image costs once and is renderer-independent.

### D5. Music: album Hero and artist page

- The album branch of `LibraryHero` passes no header pills (the artist facts line goes, `artist_facts` is deleted) and passes the album's totals as block pills: the track count pill and the total time pill, from `track_total_facts`, which becomes `track_total_pills` (two pills, the count and the time). The block's eyebrow line (year and label) stays.
- The artist branch passes header pills from `artist_page_facts`, which becomes `artist_page_pills`: the album count, the year span, and the track count and total time once loaded. The selected collections stay in the kicker. `artist_year_span_facts` goes, because the year span is a header pill. The block has no rows.
- `LibraryTracklist` and `LibraryArtistTracklist` sit under the Tracks section header. The artist page's album labels stay in the band.

### D6. The playing row and the striped rows

In `LibraryWorkspaceTable`:
- The playing row gets a row fill (a new `--canvas-playing-row` token), the title weight 600, and the playing mark (a new bars icon in `window/icons`) in the number cell in place of the number. The play glyph beside the title is deleted. `title_tone` keeps the heading tone for the playing row; `title_weight` returns 600 for it.
- Rows get the rounded corners and the alternate stripe of the artboard. The hover veil, the focus highlight, and the row marks stay.
- A row with no number cell (a Feeds entry) paints the mark in the cell's place.

### D7. Docs and the glossary

- `docs/architecture/gui-client.md`: the Hero frame section (the bands, the pills and rows, the glow path, the cast), the frame field lists for each destination (no `meta_rows`, `duration_row`, `progress_row`, or `imdb_url`; the `facts` and `cast` fields), and the sentence that says the GUI Hero carries "the same facts the TUI paints". It becomes: the same values, split into pills and rows by the GUI.
- `CONTEXT.md`: two terms. **Fact pill** (GUI): a short Hero fact in a pill; *Avoid* chip, tag, badge. **Hero glow** (GUI): the blurred copy of the Hero's art behind the header band; *Avoid* backdrop (the dimmed backdrop is an overlay scrim), background image.

### D8. Tests (contracts, by owner)

Per `docs/invariants/14-test-ownership.md` and `crates/mbv-gui/AGENTS.md`: hermetic, no sleeps, one contract per test, extend before adding.

- `mbv-emby` parse test: extend the existing people parse test so that it asserts `id` and `primary_image_tag`, and their defaults when absent.
- `hero_facts` (gui_core tests): one `#[case]` table over the source kinds, each asserting the pills and rows in order. The movie case is the contract of the spec scenario "A movie's Hero" (no runtime row; the year pill and the Released row both show).
- Emby projection: one `#[case]` table for cast members: a director is not in the cast; a person with a tag has a photo; a person with no tag has initials and no photo.
- `art.rs`: extend the `parse_path` case table with a glow path. One unit test for the glow encoder: a small in-memory image gives a PNG no larger than 64 px on its long side; bytes that do not decode give `Missing`.
- Window: extend the `LibraryHeroFrame` painted-structure test (pills row, fact rows, section headers, no progress element, glow present only with art). Extend the album branch test in `library_hero/tests.rs` (no artist facts, two block pills) and the artist branch test (header pills). Change the existing playing-row test in `library_workspace_table` to assert the mark in the number cell and no glyph beside the title (spec scenario "The playing Workspace row keeps its own mark").
- Delete the tests of the deleted helpers (`hero_layout`, `hero_meta_rows`, `join_meta_rows`, `HeroMetaRole`, `LibraryHeroSummary`, the Emby credits table and IMDb header action).

## Risks / Trade-offs

- [The pill text keeps today's formats (`01:37:32`), not the artboard's `1h 37m`] → Accepted: the user asked for no text changes beyond the listed ones. A format change is a separate request.
- [A movie with a long cast fetches many photos] → Each photo is one small Primary image through the existing bounded card fetch, and it is cached. The fetch starts only when the Hero shows the movie.
- [The glow thread decodes a large backdrop] → It runs off the core thread and the window thread. A fast series of Hero changes can start a few threads that each end in tens of milliseconds.
- [`gui-emby-row-played-marks` changes the same Workspace requirement and lands after this change] → That change must rebase its delta on this change's text of "Library lists and Workspace tables paint through shared parts" before it archives.
- [`EmbyPerson` grows on the ctrl wire] → `#[serde(default)]` keeps old and new peers compatible. The conformance bless shows the schema diff for review.
- [Home moves from `LibraryHeroSummary` to the frame] → The Home tests that hook `data-library-home-hero-*` keep their attribute names through the frame's `attr`.

## Migration Plan

No data migration. The launch file does not change. The ctrl change is additive. The change ships as one GUI release. Rollback is a revert.
