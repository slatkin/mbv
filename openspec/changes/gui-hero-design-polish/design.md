# Design

## Context

See `proposal.md` for the why and the decisions, and `specs/gui-client/spec.md` for the behaviour. The artboards are in `canvas/` (`movie-proposed-wide.png`, `album-proposed-wide.png`, and `movie-current.png` for comparison). This design is for the GUI Client only. The TUI does not change.

`gui-music-artist-page` lands before this change. The facts below are the code on `main` plus that change's branch (`feat/gui-music-artist-page`).

- Every Hero except Home paints through `LibraryHeroFrame` (`crates/mbv-gui/src/window/components/library_hero_frame.rs`): a header band (kicker, title, one facts line, an optional header action), the detail block `LibraryHeroDetail` (art plus a text column with the name, the meta rows, the actions, and the overview), and the destination's children. Music, a Shows series, Audiobooks, and Feeds pass a list band as those children; Podcasts and plain Emby libraries pass none. Home paints through `LibraryHeroSummary`, which also uses `LibraryHeroDetail` and has no list band.
- The GUI Hero facts are the shared TUI producers' plain meta rows (`crates/mbv-ui-model/src/library_hero/producers.rs`: `hero_content_emby`, `hero_content_series_with_episode`, `hero_content_abs_book`, `hero_content_abs_episode`, `hero_content_feed`, `hero_content_queue`). A row is a list of typed segments (`MetaKind`: plain, date, time, count, label, ...). A row has no label word. The frames carry `meta_rows`, `duration_row`, and `progress_row`. The window splits them with `hero_layout` (`crates/mbv-gui/src/window/library/hero.rs`): the plain rows join on the header facts line, and the duration and progress rows go into the block.
- The Emby Hero carries `credits` (director first, then the rest of `EmbyItem::people`) and `imdb_url`. It paints the credits as a striped name and role table and the IMDb link as the header action.
- `EmbyPerson` (`crates/mbv-emby-model/src/lib.rs`) holds `name`, `role`, and `kind`. `parse_people` (`crates/mbv-emby/src/types_parsing.rs`) reads `Name`, `Role`, and `Type`. Emby's `BaseItemPerson` also carries `Id` and `PrimaryImageTag`, and a person's photo is `/Items/{Id}/Images/Primary`, the same endpoint shape as an item's Primary image. `EmbyPerson` is in the ctrl v10 schemas (`protocol/ctrl/v10/schema/`).
- Browse requests already ask for `People`, `Genres`, `Studios`, `ExternalUrls`, and `ProductionYear`. Home's Continue Watching and Recently Added requests do not: their field lists omit those values except for `Overview`, `PremiereDate`, and the runtime. Latest Episodes also omits `ProductionYear`.
- The GUI loads images through `mbv-art://localhost/<path>`. `ArtPath` (`crates/mbv-gui/src/art.rs`) has `Card(key)` and `Placeholder`. `CoreThread::answer_art` (`crates/mbv-gui/src/core_thread/tick_host.rs`) answers a card with the bytes that the image cache holds (`gui_art_source`). The GUI keeps no decoded pixels (`ImageDecode::SourceOnly`). `App::fetch_card_image(cache_key, item_id, series_id, types)` fetches an Emby image into the cache.
- The window renderer is Blitz with the `vello-hybrid` backend (`dioxus-native =0.8.0-alpha.3`, `blitz-paint =0.3.0-beta.4`). `mbv-gui` already depends on `image` 0.25 with `png`, `jpeg`, and `webp`.
- `LibraryWorkspaceTable` paints the playing row with the heading tone and a play glyph beside the title (`title_tone`, `title_weight`, `PlayCluster`). `LibraryTracklist`, the Shows episodes, the Audiobooks chapters, the Feeds entries, and the artist page's `LibraryArtistTracklist` all paint through it. Music and Audiobooks already pass real playhead identity to their rows; the Shows episodes and Feeds entries currently construct their Workspace rows with `playing: false`.
- The fonts are IBM Plex Sans and JetBrains Mono (`crates/mbv-gui/assets/app.css`).

## Goals / Non-Goals

**Goals:**

- One set of Hero parts in the window, used by every Hero, so that a fix lands in all destinations.
- One GUI-owned split of each Hero's facts into pills and labelled rows, built from the source items.
- The backdrop and the cast photos without decoded images on the core thread.

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

Facts do not decide the Hero title. Queue-backed Heroes reuse the existing `QueueItem::playback_title_parts(feed_name)` split rather than inventing a second parent rule: its context is the parent (a series, show, artist, or feed), and its title is the item's own name. Home passes the same feed or subscription name that its row projection already passes; Shows New passes no feed name. An item whose split has no context has no parent and is its own largest unit. Home gains a `parent` field, and Shows New fills its existing `parent` field from this split. When a parent exists, the header title is the parent and the detail block's name is the item title; otherwise the header title is the item title.

The frames `GuiEmbyHeroFrame`, `GuiHomeHeroFrame`, `GuiShowsHeroFrame`, `GuiAudiobooksHeroFrame`, `GuiPodcastsHeroFrame`, and `GuiFeedsHeroFrame` lose `meta_rows`, `duration_row`, `progress_row`, and (Emby) `imdb_url`, and get `facts: GuiHeroFactsFrame` (the bridge mirror of `GuiHeroFacts`). The title, parent, overview, art, art shape, and Workspace stay, with Home gaining the parent.

Alternatives considered:
- Classify the producers' plain rows by `MetaKind`. Rejected: a row has no label, and an author and a show name are both plain text.
- Add labels to the shared `HeroFacts`. Rejected: it changes the TUI's model for a GUI-only look.

### D2. One frame for every Hero, with the parts in one place

`LibraryHeroFrame` becomes the only Hero layout. Home moves onto it and `LibraryHeroSummary` is deleted. The frame paints, top to bottom:

1. The backdrop layer (D4), behind the header and the detail block.
2. The header band: the kicker as the eyebrow with a leading dot, the title, and the fact pills row (`LibraryHeroPills`). The header action stays (Shuffle artist). The IMDb header action is deleted, because the IMDb pill is the link.
3. The detail block (`LibraryHeroDetail`): the art in an inset frame, and the text column with the block's eyebrow and name lines (when a parent took the title, and the album's year and label line), the album's pills (D5), the action row, and the fact rows (`LibraryHeroFactRows`). The overview leaves the block.
4. The bands, each under a section header with a rule (`LibraryHeroSection`, a new component): Overview (when the item has one), Cast (when the item has cast, D3), then the destination's Hero list band when one is defined. Music passes Tracks, a Shows series passes Episodes, Audiobooks passes Chapters, and Feeds passes Entries. Podcasts, Home, plain Emby libraries, and a flat Shows episode Hero pass no list title and no list children; Podcasts keeps its episodes only in the Library list and does not duplicate them in the detail pane. The list title prop is optional.

`HeroBlock` loses `facts` and `meta` and gets `rows: Vec<GuiHeroFactRow>` and `pills: Vec<GuiHeroPill>` (the block pills, used by the album). `HeroHeader.facts` becomes `pills`. `hero_layout`, `HeroLayoutFacts`, `HeroMetaRow`, `HeroMetaRole`, `hero_meta_rows`, and `join_meta_rows` are deleted with their tests, because no progress or duration role remains. The overview moves to a frame prop.

The detail block keeps its wrap rule. A landscape art box takes the larger share of the row beside the text column (the artboard's proportion) and drops above the text when the Hero is too narrow. Square and portrait art keep their box.

Fonts: each new part takes the font of the text it replaces. The eyebrow and the section headers use the kicker's label font. The pills use the font of today's facts line. A row's label uses the label font, and its value uses today's meta row font. A cast name and role use today's credit name and role fonts. Sizes, colours, radii, and spacing come from the artboards. New colour values become `--canvas-*` tokens in `crates/mbv-gui/assets/app.css`, and `make gui-css` regenerates `app.generated.css`.

### D3. Cast photos from Emby

- `EmbyPerson` gets `id: String` and `primary_image_tag: String`, each `#[serde(default)]`. `parse_people` reads `Id` and `PrimaryImageTag`. Browse requests already ask for `People`, and Emby returns both fields in each person by default. Home does not use those browse requests: extend Continue Watching and Recently Added with `ProductionYear,Genres,Studios,ExternalUrls,People`, and extend Latest Episodes with `ProductionYear`, so the Home facts and movie cast come from the same payload. Shows New needs no cast; `ProductionYear` is enough for its episode year pill.
- The ctrl v10 schemas list `EmbyPerson`. Per `docs/recipes.md` ("Add a ctrl protocol variant") and `crates/mbv-ctrl/src/protocol.rs`, a `#[serde(default)]` field is additive and advertises itself through a capability, with no version bump. Add the additive wire capability `emby-person-images` as `CTRL_CAP_EMBY_PERSON_IMAGES`, advertise it in `CtrlHello::current`, re-export it from `crates/mbv-ctrl/src/lib.rs`, and add `CtrlHello::supports_emby_person_images`. Because the Owner's `daemon_hello` starts from `CtrlHello::current`, it advertises the capability without a conditional `DaemonHelloCaps` flag. A peer without the capability reads `false` but still validates. The ctrl wire tests and the ctrl-protocol delta record the contract. The schemas are regenerated with the conformance bless, followed by the `mbvd` round-trip proof that the recipe names.
- The Emby projection (`emby_projection.rs`, and Home through the queue dispatch) builds `cast: Vec<GuiHeroCastMember { name, role, initials, photo: Option<GuiLibraryArtwork> }>` from the people that are not directors, in the order Emby gives. `role` is the person's role, else the person's kind, as now. `initials` is the first letter of the first two words of the name, upper case. `photo` is `None` when the person has no `id` or no tag. Otherwise the projection calls `fetch_card_image(emby_card_cache_key(id, ""), id, "", &["Primary"])` and wraps the cache read in `Some(resolve_gui_art(...))`, as `home_hero_artwork` does, so a started fetch can paint its loading state rather than looking like a person with no photo.
- The window paints the cast as a two-column grid (`LibraryHeroCast`, a new component) that wraps to one column when narrow. A member paints the photo in a round box, else the initials in that box.

Alternative considered: initials only. Rejected by the user.

### D4. The backdrop is a baked, blurred image from a new art path

- `ArtPath` gets `Backdrop(BackdropRequest)` for `backdrop/{style}/{width}/{percent-encoded key}`: the style letter (`v` video, `m` music), the baked width, and the image-cache key of the Hero's art.
- The source is a tiny Backdrop image. When an Emby Hero's art is projected (`home_hero_artwork`), `App::fetch_backdrop_image` also fetches `maxWidth=32&quality=90` of the item's Backdrop (the series' for an episode), falling back to Primary, under the key `{art key}:BD` (`ImageRendition::Backdrop`, `mbv_images::backdrop_cache_key`). A Music Hero fetches none: it blurs its own Primary art. `CoreThread::answer_art` answers a backdrop request with `GuiCore::gui_backdrop_source`: the tiny image when it landed, the Hero's own art when the item has no Backdrop image, and `Missing` while the tiny image is in flight (the window retries).
- The one `mbv-art-backdrop` worker thread (`art/backdrop/worker.rs`) bakes. A cached JPEG answers on the core thread. The worker keeps the last 32 JPEGs (first in, first out), and writes every bake to the image disk cache under `backdrop:{source key}:{width}:560:{strength}:v1`, so a re-selected Hero, or a Hero after a restart, is not baked twice.
- The bake (`art/backdrop.rs`): shrink the source to 32 px wide (the size a tiny image request gives); cover-fit to the width by 560 px with `CatmullRom` and a centre crop, anchored 30% from the top for `v` and 50% for `m`; blur at sigma 8 (run at a quarter size with sigma 2, then upscale); bake the scrims over the pane background `#272e33` with `a = S * (1 - y/(H-1))^1.4 * (0.55 + 0.45 * min(1, x/(0.6 W)))` (S 0.55 video, 0.45 music), so the bottom row is exactly the pane colour; encode JPEG at quality 88. A source whose title area falls under 4.5:1 against `#ece3cc` bakes again with S reduced until it passes, and the bake logs it.
- The Library panel provides the Hero pane's width (the Library column less the list column while Wide paints both). Each Hero asks for that width rounded up to the next 200 px, so a resize does not bake a new image on every frame.
- `art.rs` owns one helper that maps the Hero's existing `GuiArtRef::Image` card URL to the backdrop URL for the same cache key. `Placeholder` and `Loading` map to `None`. The frame takes `backdrop: Option<GuiArtRef>` and paints one `img` absolutely at the top left of the Hero's scrolling content, 100% wide and 560 px tall, behind the content. It paints nothing while the image bakes. There is no CSS opacity, gradient, filter, or blend: the scrims are in the bitmap.

Alternative considered: CSS `filter: blur()` on the art in the window. Blitz parses it and the `vello-hybrid` backend maps a Gaussian blur, but a large-radius blur over a large area runs on every repaint, and filter support in this backend is new. A baked image costs once and is renderer-independent.

### D5. Music: album Hero and artist page

- The album branch of `LibraryHero` passes no header pills (the artist facts line goes, `artist_facts` is deleted) and passes the album's totals as block pills: the track count pill and the total time pill, from `track_total_facts`, which becomes `track_total_pills` (two pills, the count and the time). The block's eyebrow line (year and label) stays.
- The artist branch passes header pills from `artist_page_facts`, which becomes `artist_page_pills`: the album count, the year span, and the track count and total time once loaded. The selected collections move to the eyebrow: the Artist arm of `hero_kicker` shows them, joined with ` + ` in the kicker's label style (today that arm returns an empty line because `artist_page_facts` carries the collections; its "kicker stays empty" comment changes with it). The `year_span_row` helper goes, because the year span is a header pill; `year_span_text` stays and builds the pill's text. (No symbol named `artist_year_span_facts` exists; the plan named it in error.) The block has no rows.
- `LibraryTracklist` and `LibraryArtistTracklist` sit under the Tracks section header. The artist page's album labels stay in the band.

### D6. The playing row and the striped rows

First give every Workspace row the playing identity it needs:

- Music and Audiobooks already carry their playing track or book identity. Shows gains `playing_episode_id` on its detail projection and frame. The value is the existing Emby item id at the playhead when that item is an episode; the existing `GuiPlayheadMarks.music_track` already samples that id and makes the Library section reproject, despite the field's music-specific name.
- Feeds gains `playing_entry_id` on its detail projection and frame. The value is the active feed item's `(feed_id, guid)` encoded in exactly the form the Feeds row uses for `GuiFeedEntryId`; an entry without a feed id uses the same empty-feed fallback as the row. `GuiPlayheadMarks` gains the encoded feed entry so the Library section reprojects when it changes. The same displayed-playhead rule as Music applies: a queued or pending target does not become the playing row merely because it is next; its identity comes only from the displayed playhead.
- `library_shows_hero.rs` and `library_feeds_hero.rs` compare those ids with each row's id and pass the result as `WorkspaceRow.playing` instead of hard-coding `false`.

Then change `LibraryWorkspaceTable`:

- The playing row gets a row fill (a new `--canvas-playing-row` token), the title weight 600, and the playing mark (a new bars icon in `window/icons`) in the number cell in place of the number. The play glyph beside the title is deleted. `title_tone` keeps the heading tone for the playing row; `title_weight` returns 600 for it.
- Rows get the rounded corners and the alternate stripe of the artboard. The hover veil, the focus highlight, and the row marks stay.
- Every current Workspace row has a number cell (`WorkspaceRow.number` is a `String`, and Feeds rows use the entry position). No numberless row case exists in this design.

### D7. Docs and the glossary

- `docs/architecture/gui-client.md`: the Hero frame section (the bands, the pills and rows, the backdrop path, the cast), the frame field lists for each destination (no `meta_rows`, `duration_row`, `progress_row`, or `imdb_url`; the `facts` and `cast` fields), and the sentence that says the GUI Hero carries "the same facts the TUI paints". It becomes: the same values, split into pills and rows by the GUI.
- `CONTEXT.md`: two terms. **Fact pill** (GUI): a short Hero fact in a pill; *Avoid* chip, tag, badge. **Hero backdrop** (GUI): the blurred copy of the Hero's art behind the header band; *Avoid* backdrop (the dimmed backdrop is an overlay scrim), background image.

### D8. Tests (contracts, by owner)

Per `docs/invariants/14-test-ownership.md` and `crates/mbv-gui/AGENTS.md`: hermetic, no sleeps, one contract per test, extend before adding.

- `mbv-emby` parse test: extend the existing people parse test so that it asserts `id` and `primary_image_tag`, and their defaults when absent. Extend the existing field-list test so that Resume and Latest assert the Home facts fields and Latest Episodes asserts `ProductionYear`.
- ctrl: extend the additive-capability handshake coverage so that the current hello advertises `emby-person-images`, a peer without it reads `false` and still validates, and the daemon advertises the same capability. Extend the pinned wire coverage so that the two optional person fields round-trip and absent fields decode empty.
- `hero_facts` (gui_core tests): one `#[case]` table over the source kinds, each asserting the pills and rows in order. The movie case is the contract of the spec scenario "A movie's Hero" (no runtime row; the year pill and the Released row both show).
- Emby projection: one `#[case]` table for cast members: a director is not in the cast; a person with a tag has a photo; a person with no tag has initials and no photo. Extend the Home/Shows-New projection tests for the queue-backed parent rule (series or show for an episode, feed for an entry, no parent for a movie or book).
- Shows/Feeds projections and destination rows: extend existing projection or component coverage so that the playing episode or entry id reaches `WorkspaceRow.playing`; the painter alone does not own this contract.
- `art.rs`: add a `parse_path` case table (none exists today) with a backdrop path case. One unit test for the backdrop encoder uses an in-memory image larger than 64 px and asserts that the output PNG's long side is at most 64 px with its aspect ratio preserved; bytes that do not decode give `Missing`.
- Window: extend the `LibraryHeroFrame` painted-structure test (pills row, fact rows, section headers, no progress element, backdrop present only with art). Extend the album branch test in `library_hero/tests.rs` (no artist facts, two block pills) and the artist branch test (header pills and the collections eyebrow). Change the existing playing-row test in `library_workspace_table` to assert the mark in the number cell and no glyph beside the title (spec scenario "The playing Workspace row keeps its own mark").
- Delete the tests of the deleted helpers (`hero_layout`, `hero_meta_rows`, `join_meta_rows`, `HeroMetaRole`, `LibraryHeroSummary`, the Emby credits table and IMDb header action).

## Risks / Trade-offs

- [The pill text keeps today's formats (`01:37:32`), not the artboard's `1h 37m`] → Accepted: the user asked for no text changes beyond the listed ones. A format change is a separate request.
- [A movie with a long cast fetches many photos] → Each photo is one bounded Hero-rendition Primary image (at most 1280 px on a side at the existing image quality) through the existing cache, not an unbounded original. The fetch starts only when the Hero shows the movie. The rendition is larger than a cast thumbnail needs, but changing the shared rendition policy is outside this design.
- [The backdrop thread decodes a large backdrop] → It runs off the core thread and the window thread. One worker thread runs the encodes in order, so a fast series of Hero changes queues work and never starts more threads; the cache answers a repeat key.
- [`gui-emby-row-played-marks` changes the same Workspace requirement and lands after this change] → That change must rebase its delta on this change's text of "Library lists and Workspace tables paint through shared parts" before it archives.
- [`EmbyPerson` grows on the ctrl wire] → `#[serde(default)]` makes the field additive at the serde and protocol-version level, and the `emby-person-images` capability advertises support. The conformance bless shows the schema diff for review. Separately, the existing local Owner wire-identity gate can refuse mixed local binaries because any field addition changes the wire digest; that gate is pre-existing local-binary policy, not a new protocol-version incompatibility.
- [Home moves from `LibraryHeroSummary` to the frame] → The Home tests that hook `data-library-home-hero-*` keep their attribute names through the frame's `attr`.

## Migration Plan

No data migration. The launch file does not change. The ctrl change is additive. The change ships as one GUI release. Rollback is a revert.
