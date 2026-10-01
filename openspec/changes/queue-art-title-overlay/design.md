# Design

## Context

See proposal.md for motivation. Current state (read from code):

- The queue card paints the cached cover with `Resize::Scale` (`render_card_painting`,
  `crates/mbv-render/src/components/card.rs`); its rendered box is aspect-fit and only known at
  paint time (`images.record_card_size`). Emby's cache key is `{album-or-item}:P`, shared with MPRIS
  art and other readers; Audiobookshelf's card keys carry the active protocol suffix
  (`audiobookshelf_cover_cache_key(server, id, current_protocol_suffix())` and the book variant,
  `project_audiobookshelf_cover`), so they change when a dimmed backdrop forces `halfblock`.
- The Library hero already composes onto a bitmap built for an exact cell box:
  `ensure_hero_cover_protocol` (`src/app/infra/image_fetch/protocol.rs`) does
  `cover_fill_hero_box` then `composite_landscape_logo`, tracked by `CachedImage.cover_box` /
  `applied_logo_key`. Box pixels come from `image_font_size()`.
- The header row paints the now-playing title via `render_header_title` (`HeaderTitle`,
  `crates/mbv-render/.../chrome_player/title.rs`); the two-part split is `PlaybackTitleParts`
  (`mbv-queue`), already carried to the panel as `transport.title_parts`.
- Dimmed backdrops force the halfblock suffix (`current_protocol_suffix`); the configured
  protocol is separate (`is_halfblock_configured`).
- `CachedImage` entries sit in an LRU (`cache_size_total`, `crates/mbv-images/src/cache.rs`).
- The main spec's header requirement predates 55346db33 / 694acfb69 (title moved into the header);
  the delta folds that in. The queue-column layout requirements (the four-row band, no separator
  row, the queue panel adjoining the playback region) are also behind the shipped code; the
  delta brings them up to date too, so the delta overlay matches what the code does.

## Goals / Non-Goals

**Goals:**
- Title text on the artwork, once per track/box/protocol, crisp (no downscale blur).
- Exactly one site carries the title at any time; failure modes land on the header.
- Reuse the hero's cover-fit + cache pattern; one pure, hermetically testable compositor.

**Non-Goals:**
- Time-varying overlay content; wrapping; marquee; user font config; system-font lookup;
  non-Latin coverage (fallback is the header).
- Overlay on Library hero, MPRIS art, or any non-queue image.
- Any further change to the queue column's band layout beyond syncing the delta to the shipped code.

## Decisions

**D1. Compose at final pixel size, keyed by box.** The overlay bitmap is the plain cover scaled
to the card's rendered box in pixels (cells x `image_font_size()`), then scrim + text drawn on it,
then encoded. Painting it at that same cell box means the protocol scales ~1:1, so text is not
softened by `Resize::Scale`. The box is obtained from the existing base protocol's `size_for`
(the same call the painter uses), so the compositor never re-implements ratatui-image's fit math.
`size_for` returns `None` while the base protocol is still encoding; the measurement then yields
nothing, no overlay is built that frame, and the header keeps the title. ratatui-image's
`Resize::Scale` fits proportionally in both directions, so a bitmap at the box's exact pixels
paints about 1:1.
*Alternative:* composite on the full-res source and let `Resize::Scale` shrink it - rejected,
text blurs and its size would track the source resolution.

**D2. Text size is cell-relative, not image-relative.** Nominal row height = one cell row
(`font.height`); glyph size ~90% of it; floor ~60% of nominal; below the floor, ellipsis.
Image size only changes how many characters fit. *Alternative:* percentage of image width (the
logo's model) - rejected, text would be huge on large art and illegible on small art.
Fallback `FontSize` (10x20, used when no picker is active) makes size approximate, but the
overlay is only live when a real protocol is configured.

**D3. Separate cache entry, not the `:P` entry.** The composed variant is its own `CachedImage`
under `{item-identity}:t:{cols}x{rows}:{title-hash}`, where *item identity* is the card's cache
key with any protocol suffix stripped (Emby `{id}:P` as is; Audiobookshelf's cover/book key
without its suffix), so the variant key is stable when a dimmed backdrop flips the suffix. Its `img` is the composed bitmap and its
`protocols` map is filled per suffix like any entry. The plain `:P` entry stays plain (MPRIS
and Library read it) and doubles as the measuring protocol and the fallback paint. Under a dimmed backdrop the variant
re-encodes through `cached_image_protocol_mut` with no change (`cover_box` is `None`, so it encodes
the composed `img` directly); no re-compose path is added. Stale variants age out through the
existing image LRU. *Alternative:* extend `:P` with another applied-
key field - rejected, one entry cannot be both the plain and composed artwork, and sharing a key
across consumers is the exact flash bug `audiobookshelf_hero_cover_cache_key` documents.

**D4. Typed title site, decided once, in two steps.** `mbv-ui-model` gets
`enum NowPlayingTitleSite { Header, Artwork }`, computed by the shell and consumed by both the
panel (header text) and the card painter.

*Overlay eligible* (drives what the card paints and composes) iff all of: playback is active per
`displayed_playback_state().active` (paused counts as active and keeps the overlay; an idle card
that shows the cursor row's art, or a cast/session title with no active local item, is never
eligible); images enabled; visualizer off; `visual_slot_shown()` (not idle, not
`visual_slot_hidden`) with a non-zero card size; `!is_halfblock_configured()`; the title passes
`covers`; and a composed variant is ready for the current identity+box+title. The card paints the
overlay variant whenever eligible.

*Site* is `Artwork` iff eligible **and** the shell has recorded that this identity+box+title
variant was painted (the fact is recorded next to `record_card_size` after a successful paint);
otherwise `Header`. The painted fact lags one frame, which errs toward the header (title shown
twice for a frame at most, never zero times). The painted fact and the variant key are both
suffix-independent (item identity, D3), and the dim-backdrop `halfblock` suffix is not an input,
so opening a dialog cannot flip the site for Emby or Audiobookshelf items.

**D5. Font and rasteriser.** Embed one static Lexend Deca weight (OFL; licence text shipped
beside it) via `include_bytes!` and rasterise with `ab_glyph` (pure Rust, no system deps).
Coverage is Latin / Latin-Ext / Vietnamese; the compositor exposes `covers(text) -> bool`
(`glyph_id != 0` for every char) and the site rule treats `false` as `Header`.
*Alternatives:* `fontdue` (comparable); system font via fontconfig (heavy dependency, still no
CJK guarantee); variable TTF (needs variation support - use a static instance instead).
Task 1.1 confirms the crate and weight before anything else builds on them.

**D6. Compositor home and shape.** A pure `mbv-images` module (`title_overlay.rs`):
`compose_title_overlay(base: &DynamicImage, cell: FontSize, parts: &TitleOverlayText, colours)
-> DynamicImage`, plus `covers`. It takes resolved RGB colours (the theme roles are `Color::Rgb`), no `App`.
Colours: a context part paints `PLAYBACK_CONTEXT_FG` (yellow), a title part under a context
paints `PLAYBACK_TITLE_FG` (aqua), and a lone title paints `PLAYBACK_CONTEXT_FG` (yellow), the
same as the header's lone title. Placement is top row for the context-or-lone title, bottom row for a two-part title; each row
gets a flat translucent scrim at constant alpha across the whole row. Text in the top row is
right-aligned; text in the bottom row remains left-aligned.

## Risks / Trade-offs

- **Box not known until painted** -> measure from the base protocol's `size_for`; first frame
  falls back to header/plain art, the overlay swaps in on the next sync. Mitigation: D4's
  safe-direction rule.
- **Compose cost on the render thread** (`cover_fill` Lanczos of a large source) -> same cost the
  hero already pays on box change; measure in task 2.4, move to the resize worker only if it
  hitches. Not speculative-patched beforehand.
- **Missing glyphs for non-Latin titles** -> header fallback; the user sees no tofu.
- **Resize / split-drag churn** -> each new box recomposes; debounced by the existing box-keyed
  entry check, old variants LRU out.
- **Memory** -> at most a few composed bitmaps live; the LRU bounds them. Must not regress the
  headless footprint (a headless owner never composes: it has no card).

## Open Questions

- Exact scrim opacity/height and Lexend weight (Medium vs SemiBold): tuned visually during
  task 3.3; no spec impact.
