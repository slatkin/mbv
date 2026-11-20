# Design

## Context

See `proposal.md` for the why and `specs/` for the behavior. The reference
mockup is `~/kkk.zip` (`Main.dc.html`); its values are the look source, mapped
to the GUI's theme tokens below.

Current state that shapes the approach:

- One-level Library list rows paint through `LibraryItemRow`
  (`crates/mbv-gui/src/window/components/library_item_row.rs`). Home paints
  its own copy (`library_home_list.rs::paint_home_item_row`). Tree lists
  (Music `library_list.rs`, Shows `library_shows_list.rs`) paint their own
  top-level rows. Every list paints over `library_virtual_list`, which knows
  the painted row range.
- Only Home rows carry art (`HomeItemRow::art`, from
  `App::gui_home_row_artwork`, which fetches the queue-card chain at once, for
  every Home row, with no idle gate).
- Every Emby image request is `maxWidth=1280&maxHeight=1280&quality=90`
  (`src/app/infra/image_fetch/protocol.rs`), with no `tag`. Cache keys hold no
  tag. Audiobookshelf covers are requested with no `width` (Service default
  400). The memory cache is one LRU of 50 entries (`image_cache_size`) for all
  surfaces. The disk cache keeps a file until 30 days of no use.
- The Hero's source per item comes from the artwork policy
  (`crates/mbv-components/src/library_panel/hero.rs`: `emby_artwork_policy`,
  `abs_book_artwork_policy`, `abs_episode_artwork_policy`,
  `feed_artwork_policy`). Feeds declare no source.
- Queue rows (`queue_panel.rs`, `queue_panel/row_cells.rs`,
  `queue_panel/geometry.rs`) are 34 px with a 2 px gap, a progress bar inside
  the row bottom, and a start mark before the title. `GuiQueueRow::Item`
  already carries `duration` (the item's total runtime text).
- The window holds Library scroll (design D9 of add-gui-library-panel); the
  Client core does not know which rows are in view.

## Goals / Non-Goals

**Goals:**

- One top-level row part for every destination, Home included.
- Row images that cost about one small request per row, once, and stay correct
  when the server art changes.
- A no-source row kind that a later local-file kind can use with no layout
  change.

**Non-Goals:**

- The mockup's native browser scrollbars (its `overflow: auto` lists). Every
  list keeps its current scroll and scrollbar look.
- The mockup's top bar, Hero, player, footer, and section layout. Only the
  Library list rows (and the Home section headings) and the Queue rows change.
- Nested rows (seasons, episodes under a series, albums, tracks).
- The TUI. No TUI painter, projection output, or key changes.
- Feed image parsing (`itunes:image`, `media:thumbnail`).
- A local scale-down of the Hero image as a row source.

## Decisions

### D1. One top-level row part

`LibraryItemRow` becomes the top-level row part and gains the thumb. Home's
`paint_home_item_row`, the Music artist row, the Shows series row, and the
Shows New row paint through it. Nested rows keep their current painters.
Destinations keep their hook prefix (`data-library-<attr>-*`); the thumb adds
`data-library-<attr>-thumb` with the art state word (`image`, `loading`,
`missing`, `none`) and `data-library-<attr>-thumb-progress`.

Alternative: a second part beside `LibraryItemRow`. Rejected: the one-level lists
would then hold two row looks, which the shared-parts requirement forbids.

### D2. Library row look (mockup values, theme tokens)

| Part | Value |
| --- | --- |
| Row | 56 px high, padding `0 8px`, gap 12 px, radius 0 |
| Divider | 1 px line under each top-level row and over the list's first row, `--canvas-border` |
| Selected row | fill `--canvas-library-raise` (unchanged) |
| Thumb box | 64 × 36 px, radius 3 px, fill `--canvas-well`, overflow hidden |
| Landscape / portrait image | fills the box, `object-fit: cover`, centred |
| Square image | 36 × 36 px, centred in the box |
| Kind icon (missing, none) | `EighthNotes` (audio) or `MonitorPlay` (video), 16 px, `--canvas-muted` |
| Thumb progress track | 3 px along the box bottom, full width, new token `--canvas-thumb-track: rgba(0, 0, 0, 0.55)` |
| Thumb progress fill | 3 px, the row's percent, `--canvas-partial` |
| Title | 14 px, weight 400, `--canvas-text` (played: `--canvas-muted`, unchanged rule) |
| Subtitle | 12 px, `--canvas-muted` |
| Trailing text | `font-mono` 12 px, `--canvas-muted` |
| Home section heading | 12 px, weight 600, uppercase, letter-spacing 0.06em, `--canvas-muted` |

The mockup's mono face (IBM Plex Mono) is not bundled; it maps to the bundled
`font-mono` (JetBrains Mono, `--font-mono`) here and in D3. The kind icon, the
image's centred crop and centring, and the shared title start of every row
are look choices held here, not in the spec.

`FLAT_ROW_TITLE_WEIGHT` becomes 400 for top-level rows. The accent run under
the row is deleted. Nested rows keep their heights; the tree lists' fixed
row-height tables change only for their top-level kinds.

### D3. Queue row look

| Part | Value |
| --- | --- |
| Row | 40 px high, `ROW_GAP_PX` 0, columns `28px | 1fr | 96px`, gap 10 px, padding `0 10px` |
| Divider | 1 px line under each row, `--canvas-border` |
| Corners | radius 0; the now-playing row radius 4 px |
| Ordinal | `font-mono` 11 px, right-aligned, `Grey1` (unchanged colour) |
| Title | 14 px; now playing `Green` weight 600; played `Grey1` struck in `Bg5` (unchanged colours) |
| Ring | after the title, 6 px space; 14 px SVG, r 6, stroke 2.5; track `Bg2`, fill `Orange`, now playing `Green` |
| Right column | the frame's `duration`, `font-mono` 12 px, `Grey1`, right-aligned |
| Fills | now playing `BgGreen`; marked `Bg3`; hovered `Bg0` (unchanged colours, now square) |

The hover chrome stays: the grip covers the ordinal, the actions button
covers the right end of the row (so the hovered row hides its duration). The
drag ghost paints the title and the ring. The ghost and menu offsets already
derive from `ROW_HEIGHT_PX` and `ROW_GAP_PX`, so they follow the new values.

The ring copies the collapsed rail's ring (`collapsed_rail.rs`): two
circles, each with `stroke="currentColor"` and its token in the circle's own
`color` style, and the fill arc drawn with `stroke-dasharray` over the
circumference (37.7 at r 6), rotated −90°. A `var()` in a `stroke` attribute
is never used: usvg drops it and the ring does not paint. Its percent is
`row_progress` (unchanged): the frame's percent for a partial row and the
playhead's predicted percent for the now-playing row.

The start mark (`StartMark`, `start_mark`, the `Play` glyph before the title)
is deleted from the row. The row is the only reader of `GuiQueue::start`,
`GuiQueue::playing`, and `GuiQueue::start_slot_reported`, so those three
fields, `QueueStartProjection`, and `gui_queue_start_projection` are deleted
too, with the tests that cite the retired start-mark scenarios
(`src/app/tests/named_queues.rs`, `src/app/tests/named_queues/peer_link.rs`,
`crates/mbv-gui/tests/bridge/frame.rs`).

Alternative: keep the time left beside the ring. Rejected by the user
("precision is not necessary"); the right column is the duration, always.

### D4. Row art frame

A new frame enum, shared by every top-level row frame:

```rust
pub enum GuiRowArt {
    Image { art: GuiArtRef, square: bool }, // square: pixels decide the fit
    Loading,
    Missing,
    NoSource,
}
pub enum GuiRowMediaKind { Audio, Video }
```

`square` comes from the cached image's pixels through
`painted_shape_for_pixels` (`Square` → true; `Landscape` and `Portrait` →
false, both cover-fill). The frames that gain `art: GuiRowArt` and
`media: GuiRowMediaKind`: `HomeItemRow` (its `Option<GuiArtRef>` is replaced),
the Emby list row, `GuiMusicArtistFrame`, the Shows series and New rows,
`GuiEntryRowFrame` (Podcasts and Feeds), and the Audiobooks book row.

Media kind: an Emby item → `EmbyItem::is_audio`; a Music artist → Audio; a
Shows row → Video; an Audiobookshelf book or episode → Audio; a feed entry →
`FeedKind::Audio` → Audio, otherwise Video.

### D5. Row art source

The row's source is the item's Hero source from the artwork policy, so the row
and the Hero show the same image:

- Emby items (Home, Emby lists, Shows series and New): `emby_artwork_policy`.
- Shows Upcoming (display-only, no Hero; user decision): the series image —
  the `Thumb`, `Backdrop`, `Primary` chain addressed to the premiere's
  `series_id`, tagged from its `series_thumb` or `series_backdrops[0]` per D6.
  New and Upcoming rows share `paint_shows_flat` and `GuiShowsRowFrame`, so
  both take the thumb in one place.
- Music artist with `ArtistKey::Service(id)`: Emby `Primary` of the artist id
  (one request; the Music Hero shows albums, so the artist's own image is the
  artist row's image). `ArtistKey::Fallback` (a folder-based library: album
  folders carry no `ArtistItems`) → the Service artist item id its level fill's
  tracks named for that name (`artist_name_item_ids`), else `NoSource`. That id
  feeds the row's image source only and never becomes an `ArtistKey`.
- Audiobooks: `abs_book_artwork_policy`. Podcasts: `abs_episode_artwork_policy`
  (the show's cover).
- Home Recently added Audiobookshelf rows: the same ABS policy as their own
  destination (the show's cover for an episode), so the row is the same
  cached image as on Podcasts. The Home Hero's art does not change in this
  change.
- Feeds: `NoSource` (`feed_artwork_policy` declares no source).

A row whose policy gives no source is `NoSource`.

### D6. Row rendition request

`ImageFetchReq` gains `rendition: ImageRendition { Hero, Row }` (existing
callers pass `Hero`; their URLs do not change).

Emby `Row` request, one image type, one request:

- Resolve the chain to its first type whose tag the item declares, with the
  addressing `fetch_emby_image_type` uses: `Thumb`, `Backdrop`, and `Logo`
  with a `series_id` address the series and take `series_thumb` or
  `series_backdrops[0]`; without a `series_id` they address the item and
  take `thumb` or `backdrops[0]`. `Primary` addresses the item and takes
  `primary`. Thus a Continue watching or Shows New episode row gets the same
  series image as its Hero. Request that one type with `tag={tag}`. If the item declares no tag in the chain (for example a Music
  artist, whose frame carries no tags), request the chain untagged as today.
- Size: `maxWidth=128` for a declared Landscape or Portrait arm,
  `maxWidth=72&maxHeight=72` for a declared Square arm (2× the painted box).
  Never `width` with `height`: Emby then forces that exact size and distorts
  the image.
- `quality=80`. No `format` parameter (Emby returns the original jpg or png,
  which the window decodes).

Audiobookshelf `Row` request: `/api/items/{id}/cover?width=128&format=jpeg`
for a book, `width=72` for a podcast cover. Never `raw=1`.

Row cache keys, separate from every Hero and card key:

- Emby: `row:{item_id}:{type}:{tag}`; untagged: `row:{item_id}:untagged`.
- Audiobookshelf: `row:abs:{server_hash}:{library_item_id}:{updated_at}`;
  `{updated_at}` is `0` when the catalog lacks it.

Both live in the existing disk cache directory and its 30-day eviction. A new
tag makes a new key, so changed art loads again; the old file ages out.

Audiobookshelf change detection: the library item's `updatedAt` (ms, from the
Audiobookshelf API's library item object) is parsed into the catalog entry for
books and podcasts (`crates/mbv-audiobookshelf/src/catalog.rs`). A cover
upload updates the item, so the key changes with the cover.

Alternative: scale the cached 1280 px Hero image locally. Rejected: a row
whose item was never picked would first download the full Hero image, which is
slower and larger than a 128 px request.

### D7. Rows in view and fetch order

`library_virtual_list` reports the stable ids of the rows it paints plus an
overscan of 4 rows above and below, through a new command
`GuiLibraryCommand::RowsInView { scope: GuiRowArtScope, ids: Vec<String> }`,
each time that set changes. `GuiRowArtScope` names the destination list
whose rows the ids are (Home, Emby, Music, ShowsSeries, ShowsNew,
ShowsUpcoming, Podcasts, Audiobooks, Feeds); it lives in the app crate beside `GuiLibraryCommand`,
because the window's `PointerList` is not visible to the core and also names
the Tracklist and the Workspace, which carry no thumbs. The ids are the
frames' existing stable row ids, so the core resolves them with no
coordinates.

The core keeps the latest set per list and stamps the navigation-idle clock
(the same gate `fetch_list_card_image_when_idle` uses). Once navigation is
idle, the core resolves each in-view row's source and queues its `Row` fetch.

`ImageCache` gets a second pending queue for row fetches. Each `RowsInView`
replaces it, so a row that left the view before its fetch started is dropped.
Dropping an unstarted row request also clears its loading mark
(`card_image_loading`, set by `reserve_fetch` when it queues), so the row
fetches again when it comes back into view and a later reserve does not
return `Duplicate`.

Retries stay demand-driven (invariant 27): when an interrupted row key's
retry time passes and the key is in the stored in-view set, the core queues it
again. The retry deadline joins the existing art retry deadline
(`gui_art_retry_due`, `sections/deadlines.rs`).
`take_pending_fetch` drains the Hero/card queue first. Row fetches use at most
4 of the 6 fetch slots (`MAX_IMAGE_FETCHES`), so a Hero fetch always has a free
slot.

Every row frame resolves its `GuiRowArt` from the cache on each projection
(a cache read, no fetch), so a cached row out of view still shows its image.
A row not cached and not fetched is `Loading`. `gui_home_row_artwork` and its
at-once fetch are deleted; Home goes through this path.

### D8. Row memory budget

`ImageCache` gets a second LRU for keys that start with `row:`, with a fixed
capacity of 256 entries (about 2–3 MB at row size). Row inserts evict only
row entries. The existing `image_cache_size` LRU keeps every other key. The
budget is a constant, not a config key.

### D9. Continue watching time left

Home Continue rows get their trailing text from the Emby item's runtime minus
its saved position: whole minutes rounded up, `"{m}m left"` under one hour and
`"{h}h {m}m left"` from one hour. Recently added rows keep their date. The
format lives beside the Home row projection in `src/app/gui_library/home.rs`.

## Risks / Trade-offs

- [The ring's stroke colour] → A DOM render test cannot see a dropped
  stroke. The ring copies the collapsed rail's proven `currentColor` form
  (D3), so the stroke reaches usvg.
- [Untagged artist images do not refresh] → Same as today's behavior for
  every image; the 30-day eviction bounds it.
- [2× sizes look soft on a 3× display] → The GUI targets 1× and 2×; a larger
  scale only costs sharpness, not layout.
- [A row frame update per completed row fetch] → Row art changes ride the
  existing `libraryList` section update; fetches complete in batches of at
  most 4, so the update rate stays bounded.
- [Mixed top-level and nested heights in tree lists] → The virtual list
  already handles per-kind heights (Home uses three).

## Migration Plan

No data migration. Old cache files under Hero keys stay valid. Row keys are
new files. Rollback is a revert.
