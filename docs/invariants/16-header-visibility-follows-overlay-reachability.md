# Invariant 16 — Header visibility follows the classified reachability of the title overlay

**Scope:** the Queue playback panel's header row and the title-overlay
classification in `src/app/state/projection/card.rs`.

## The invariant

While the queue is playing, the playback panel's header row is painted exactly
when the title overlay is **unreachable** — images disabled, a disabled image
protocol, halfblock, the visualizer shown, the visual slot hidden by the user,
or a title the overlay font cannot cover. While the overlay is merely
**pending** — art not yet projected or decoded, card size unmeasured, no title
yet, the overlay composed but not yet painted, or a queue-column resize drag —
the header is hidden and the title is drawn on neither site until the overlay
lands. Header visibility is a property of the setup, the protocol and the
title, recomputed from live state every sync pass; it is never read from
whether a variant has actually painted. The painted-key bookkeeping is a
separate fact: it must record only the overlay variant, never the plain
fallback.

## Why it matters

Nothing in the types stops header visibility being read from painted reality
again, or the painted key being recorded from whichever bitmap painted last.
Either mistake compiles. Reading from painted reality reintroduces the flash
this design removes: the overlay composes a frame or more after a track
change, so the header would reappear and then vanish for every track. Recording
the plain fallback as the painted key aliases a plain entry with the variant:
the plain and derived entries live under different keys (`{id}:P`/`{id}:QB`
versus `{identity}:t:{cols}x{rows}:{hash}`), so the overlay paint and the
fallback paint become a race over one shell-held value.

## How the code maintains it today

- `title_site_skip_reason` in `src/app/state/projection/card.rs` decomposes the
  old conflated skip list into `unreachable()` (sets `header_visible = true`)
  and `pending()` (sets `header_visible = false`); the projection starts with
  `header_visible: false`, so a composable overlay leaves the header hidden by
  default.
- `refresh_queue_card_image` runs that classification every sync pass, ungated
  by slot visibility; the fetch and overlay-compose work stays gated on the
  shown slot. Idle is not part of this rule — the shell ORs it in via
  `queue_header_rows` (`src/app/shell/draw.rs`).
- `render_card_painting` records the painted key only through
  `painted_overlay_key`, which requires the `:t:` `DERIVED_SEP`; a
  plain-fallback paint never records a key, so the site is never misread as
  carrying an overlay.
- Eviction of an identity's derived variants clears the painted key
  (`crates/mbv-images/src/cache.rs`, `remove_derived_variants_for_identity` via
  `clear_painted_if`, and `clear_images_and_loading`).
- Tests: `unpainted_overlay_on_a_capable_setup_keeps_the_header_hidden`,
  `unreachable_fallback_flips_the_header_visible`,
  `uncovered_title_glyphs_flip_the_header_visible`, and
  `column_resize_drag_builds_no_overlay_variant_until_it_ends` in
  `src/app/state/projection/card/title_site_tests.rs`;
  `building_title_overlay_leaves_shared_plain_card_bitmap_unchanged` in
  `src/app/infra/image_fetch/protocol/protocol_tests.rs` (the variant never
  mutates the plain entry, so the two keys cannot alias).

## Where it currently fails

No known violation. A new unreachable condition must be added to
`title_site_skip_reason` and a new pending window must default to hidden;
nothing in the types forces either, and a misclassified pending state silently
shows no title. The painted key is a single shell-held value, so a new paint
path for a variant must record or clear it itself, and nothing enforces that a
new paint path does so.
