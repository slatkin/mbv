# Invariant 37 — A row cache key holds the Service's change value

**Scope:** the row image keys `mbv_images::emby_row_cache_key` and
`mbv_images::abs_row_cache_key` (`crates/mbv-images/src/lib.rs`), the Emby row
request that builds the first (`mbv_images::row::emby_row_request`,
`crates/mbv-images/src/row.rs`), the Audiobookshelf row cover path
(`row_cover_path`, `crates/mbv-audiobookshelf/src/catalog.rs`), the catalog
fields the key reads (`updated_at_ms` on `AudiobookshelfShow` in
`crates/mbv-audiobookshelf/src/catalog.rs` and on `AudiobookshelfBook` in
`crates/mbv-audiobookshelf/src/catalog_books.rs`), and the fetch that reads the
key's disk entry (`fetch_emby_image` and
`fetch_audiobookshelf_image`, the core's `app::infra::image_fetch` module).

## The invariant

A row image's cache key SHALL contain the Service's change value for that
image: the Emby image tag the item declares for the requested type
(`row:{item_id}:{type}:{tag}`), or the Audiobookshelf library item's last update
time in epoch milliseconds
(`row:abs:{server_hash}:{library_item_id}:{updated_at}`). A request whose item
declares no such value SHALL say so in the key rather than borrow another
image's key: Emby's untagged chain uses `row:{item_id}:untagged`, and a
catalog entry with no `updatedAt` keys `{updated_at}` as `0`.

## Why it matters

The disk cache keeps a row's bytes until 30 days of no use. Emby reissues an
item's image tag when its artwork changes, and an Audiobookshelf cover upload
updates the library item's `updatedAt`. Without the change value in the key, a
new cover would keep painting the old bytes: the fetch would read the existing
disk entry under the same key and never ask the Service. With it, a changed
cover is a new key, so the new art loads and the old file ages out.

The Hero image is a different key on purpose. A row costs its own 128 px or
72 px request (design D6) instead of downloading and scaling the 1280-pixel
Hero image.

## How the code upholds it today

- `emby_row_request` resolves the Thumb/Backdrop/Logo/Primary chain to the
  first kind whose tag the addressed source declares, and passes that kind and
  tag to `emby_row_cache_key`, so the kind and its tag are both in the key.
  The request carries the same kind and tag to the fetch, which adds
  `tag={tag}` to the URL.
- An item with no declared tag in the chain requests the chain untagged and
  keys it `untagged` (`ImageRendition::Row` with `emby: None`).
- `abs_row_cache_key` puts the library item's `updated_at_ms` in the key, and
  the catalog parse reads `updatedAt` from the library item DTO for both books
  and podcasts, defaulting it to `0` when the server sends none.
- The row remove/evict paths (`ImageCache::remove_image`,
  `touch_cache_budget`) key on the exact cache key, so a changed tag or update
  time leaves the previous key's entry alone until the disk eviction reclaims
  it.

## Where it still fails

- An untagged Emby row (a Music artist row, whose frame declares no tags) holds
  no change value. A changed artist image keeps the old bytes under
  `row:{item_id}:untagged` until the 30-day disk eviction.
- An Audiobookshelf row whose catalog entry lacks `updatedAt` keys `0`, so a
  later cover upload does not change its key. The same 30-day eviction is the
  only refresh.
- The disk cache generation is folded into the filename, not the key. A change
  to the row request's shape (width, quality) needs that generation bumped, or
  every existing row entry keeps satisfying the new request.
