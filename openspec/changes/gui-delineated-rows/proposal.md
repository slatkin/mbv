# Proposal

## Why

The GUI Library rows and Queue rows show progress as thin runs under the row,
and only Home rows carry art. The approved mockup ("Home delineated rows, no
seekbar underlines", `~/kkk.zip`, `Main.dc.html`) gives every top-level Library
row an art thumb that carries its progress, and gives the Queue a quiet,
delineated row with a fixed duration column. Art on every row also needs a
fetch path that is fast and correct: today every image is fetched at 1280 px
and cached under a key that never changes when the server art changes.

## What Changes

- **Library top-level rows** (flat-list items and parent rows, in every
  destination: Home, Emby lists, Music artists, Shows series and the Shows New
  list, Podcasts, Audiobooks, Feeds) get the mockup's delineated row: an art
  thumb in a fixed 16:9 box, the title over the subtitle, and the trailing text.
  Nested rows (seasons, episodes under a series, albums, tracks) do not change.
- The thumb shows the same image as the item's Hero, at row size. A landscape
  image fills the box, a portrait image is cropped from its centre, and a
  square image shows whole, centred in the box.
- Progress moves from the run under the row to a bar along the thumb's bottom
  edge. A row with no progress shows no bar.
- A row whose kind has no art source (Feeds now, local files later) is a normal
  row: its box shows the generic audio or video icon that the Queue playback
  panel title uses. A row whose source gave no image shows the same icon.
- A row with only a title (no subtitle, no trailing text, no progress) is a
  complete row.
- Continue watching rows show the time left as their trailing text. Other lists
  keep their trailing text.
- **Queue rows** get the mockup's delineated row: the ordinal, the title, and a
  right column that always shows the item's duration, in every row state. A row
  with progress (partly played, or now playing) shows a small progress ring
  after its title. **BREAKING** (spec): the queue row's progress bar inside the
  row goes, the "no right-column value" rule goes, and the start mark goes in
  both forms (filled for a playing queue, outlined for a stopped or resting
  queue). The now-playing row stays distinct by its fill and title.
- **Row art fetch**: each row thumb is its own small rendition from the
  Service, cached under its own key. Emby requests carry the item's image tag,
  and the cache key holds that tag, so changed art loads again. Only rows in
  view fetch, after navigation stops, behind the Hero fetch. Row thumbs have
  their own memory budget, so a scroll never evicts the Hero or the queue card.

## Capabilities

### New Capabilities

- `library-row-artwork`: the art a Library row thumb shows — its source (the
  item's Hero artwork source), its states (image, loading, missing, no source),
  the row-sized Service rendition, the tag-keyed cache, the in-view fetch
  order, and the row memory budget.

### Modified Capabilities

- `gui-client`: "The queue shows a row menu and drag reorder" — the queue row
  carries the duration column and the progress ring, and loses the in-row
  progress bar and the start mark. "Library lists and Workspace tables paint
  through shared parts" — every top-level row in every destination, Home
  included, paints through the one row part with its art thumb.

## Impact

- `crates/mbv-gui/src/window/components/`: `library_item_row.rs`,
  `library_home_list.rs`, the Emby, Music, Shows, Podcasts, Audiobooks, and
  Feeds list components (top-level rows only), `queue_panel.rs`,
  `queue_panel/row_cells.rs`, `queue_panel/geometry.rs`, `art_image.rs`.
- `crates/mbv-gui/src/bridge/frame/library/*` and `bridge/library.rs`: row art
  references and the media kind on the row frames; a rows-in-view command.
- `src/app/gui_library/`: per-row art resolution, row art fetch order.
- `src/app/infra/image_fetch/`: the row rendition request (Emby size and tag,
  Audiobookshelf width), the row cache keys.
- `crates/mbv-images/src/cache.rs`: the row memory budget.
- `crates/mbv-audiobookshelf/src/catalog.rs`: the library item's `updatedAt`
  for the cover cache key.
- The TUI does not change. No new dependency.
