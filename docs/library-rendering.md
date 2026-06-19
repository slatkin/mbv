# Library rendering, images, and music specifics

## Library rendering

`render_library()` dispatches to `render_album_view()` (inside a music album), `render_season_grid()` (level contains only Season items), or `render_library_table()` (standard row-per-item, everything else).

In `render_library_table`, album folder rows (`is_album_folder = at_album_folders && item.is_folder`) get 3-line height, always fetch/show album art, and a background year fetch via `fetch_album_year()`.

## Image handling

`fetch_card_image()` spawns a thread, downloads bytes, sends to `card_image_rx`. The `"AudioChild"` image type fetches the first Audio child of an album folder then grabs its Primary image — workaround for Emby's image API not serving art directly on MusicAlbum items.

`magick_resize` (`images.rs`) tries `magick convert` then falls back to `convert`, via a for-loop with `let Ok(...) else { continue }` rather than `?` — this is intentional, so failure of the first command falls through to the second instead of propagating.

## Music library specifics

`album_year_cache: HashMap<String, u32>` is lazily populated by `fetch_album_year()`, which fetches the first Audio child of an album to read its year — Emby doesn't always propagate year to the MusicAlbum container item itself.
