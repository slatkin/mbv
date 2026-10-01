# Invariant 16 — The title site stays Header until the overlay variant has actually painted

## The invariant

The queue card's title site (`NowPlayingTitleSite`) is `Artwork` only when the
shell has recorded that the *composed overlay variant* for the current
identity, box and title was painted. Eligibility (the overlay could be built)
is not enough, and neither is a paint of the plain artwork: the recorded
painted key must equal the current variant key. Every other state resolves to
`Header`, so the title is never absent from both the header row and the
artwork.

## Why it matters

Nothing in the types stops `title_site` being set from eligibility, or the
painted key being recorded from whichever bitmap painted last. Either mistake
compiles and drops the header title for frames where the artwork still shows
plain art (variant not yet encoded, fallback to `plain_cache_key`, resize,
track change), leaving the title on neither site. The plain entry and the
overlay variant live under different keys (`{id}:P`/`{id}:QB` versus
`{identity}:t:{cols}x{rows}:{hash}`); confusing them is a race between the
fallback paint and the overlay paint.

## How the code maintains it today

- `resolve_title_site` in `src/app/state/projection/card.rs` compares the
  variant key with `painted_title_overlay_key()`; the projection starts from
  `Header`.
- `render_queue_playback_slot` records the painted key only when the variant
  itself painted (`painted_overlay_key` requires the derived-key separator), so
  a plain-fallback paint never records a key and the site stays `Header`.
- Tests: `chooses_site_from_painted_fact` and `overlay_painted_fact_requires_overlay_key`
  in `src/app/state/projection/card/title_site_tests.rs`;
  `building_title_overlay_leaves_shared_plain_card_bitmap_unchanged` in
  `src/app/infra/image_fetch/protocol/protocol_tests.rs` (the variant never
  mutates the plain entry, so the two keys cannot alias).
- The painted fact lags one frame by design: the header may show the title
  twice for a frame, never zero times.

## Where it currently fails

No known violation. The painted key is a single shell-held value, so a path
that paints a different variant (a new draw site for the card, a second
surface sharing the key) must record or clear it itself; eviction of the
painted variant clears it (`crates/mbv-images/src/cache.rs`). Nothing
enforces that a new paint path does so.
