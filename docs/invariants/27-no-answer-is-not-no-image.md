# Invariant 27 — A fetch with no answer is not a fetch with no image

**Scope:** every card-image fetch: `spawn_image_fetch`, `fetch_emby_image`,
`fetch_audiobookshelf_image`, `ImageCache::reserve_fetch`, and the receiver
`drain_card_image_completions`. Landed by the 2026-10-30 queue placeholder fix.

## The invariant

A cache entry with `img: None` SHALL mean that the server answered and that no usable image exists.
A fetch that got no answer (a timeout, a transport failure, no Emby client yet) SHALL write no cache
entry. Its key stays open to a retry.

## Why it matters

Surfaces treat `img: None` as final: the queue card paints the no-image placeholder, the hero
returns `HeroImageState::None`, and `reserve_fetch` refuses the key for the rest of the process.

On 2026-10-30 the Emby server stalled for one minute at 05:57. Image requests for three episodes
timed out at the 10 second limit. All three had a `Primary` tag. All three showed the placeholder
during playback that evening, while their neighbours showed art.

## How the code upholds it today

- `ImageFetchOutcome` separates `Image`, `Empty`, and `Interrupted`. In `protocol.rs`, a status code
  is an answer (`Missing`). Every other request failure is `Interrupted`.
- `first_image` stops the kind chain on `Interrupted`. Without this stop, a timed-out `Primary` falls
  through to the series' `Thumb`, and the disk cache keeps series art under the episode's own key.
- `ImageCache::fetch_interrupted` records a retry time and writes no entry. `reserve_fetch` refuses
  the key only until that time. Retries are demand-driven: a key runs again only while a surface asks
  for it, so a server outage costs one request per displayed key per delay.
- While a key is interrupted, the queue card paints the loading block and the title stays in the
  header (`is_fetch_interrupted`). The hero already reports `Loading` for a key with no entry.

## Where it still fails

- An artist artwork fetch (`artist_artwork_requests`) stays final after an interruption. Its only
  completion is the `ArtistArtworkFetched` event, so `drain_card_image_completions` writes the empty
  entry as before.
- A worker panic reports `Empty`.
- A 5xx status counts as an answer. Emby returned a repeated 500 for one item's `Primary` in the same
  log, so a retry would not help that item, but a short 5xx outage still writes empty entries.
