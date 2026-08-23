## Why

`#596` audited the *inline* (narrow) hero and deferred the *hero-on-left* (wide)
audit as "separate and follows this one." Hero-on-left starts in a better state —
every wide surface already routes through the shared shell
(`shared_hero_presentation`, `hero_on_left_right_pane`,
`hero_on_left_list_panel_border`). What did not converge is small and specific:
the design language is one layout — a static hero on the left, and on the right a
pill bar over an inset box whose **contents** are the only thing that varies
(movies → overview, TV → seasons with a pill selector, albums / podcasts /
audiobooks → tracks). The deviations are drift, not variants.

## What Changes

- **Podcasts pill bar (ABS):** wide `render_audiobookshelf_podcasts` bypasses
  `hero_on_left_right_pane` and paints the show list straight onto `right_panel`,
  so wide has no pill row while narrow does. Route wide through the shared pane
  split and call the existing `render_audiobookshelf_podcast_bucket_pills` — the
  pill bar is identical across views (same as ABS Books already does). Delete the
  comment rationalizing the omission.
- **Emby podcast libraries:** remove the `is_podcast_library` special-casing
  (`feed_actions.rs`, `list.rs`, `detail.rs`). An Emby podcast library is a
  generic library and renders like any other — nothing bespoke.
- **Inset-box framing:** affirm in the arrangement spec that hero-on-left is one
  layout; only the inset box's contents vary by type. No per-surface geometry.
- Selection background is **not** owned here — it comes from
  `unify-selected-row-background`. This audit consumes that primitive.

## Capabilities

### Modified Capabilities
- `right-panel-arrangements`: hero-on-left is one layout with a type-varying inset
  box; wide surfaces reserve the pill row uniformly via `hero_on_left_right_pane`.
- `audiobookshelf-podcast-library-ui`: the wide podcast presentation shows the
  same bucket pill bar as narrow.

## Impact

- `src/app/render/components/audiobookshelf.rs` (wide podcast path),
  `audiobookshelf_book_browser.rs` (reference shape), `list.rs`, `detail.rs`,
  `feed_actions.rs` (`is_podcast_library` removal).
- **Depends on** `unify-selected-row-background` — do that first; this audit's
  selection rows call the shared primitive rather than re-deriving them.
- No protocol, provider, or daemon surface.
