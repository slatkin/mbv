# Design

## Context

See proposal.md for motivation. Current state (read from code):

- The queue card paints the cached cover with `Resize::Scale` (`render_card_painting`,
  `crates/mbv-render/src/components/card.rs`); its rendered box is aspect-fit and only known at
  paint time (`images.record_card_size`). The cache key is `{album-or-item}:P`, shared with MPRIS
  art and other readers.
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
  the delta folds that in. The "playback panel renders in the queue column" requirement
  (title band rows) is also behind those commits; it is left as is, not touched here.

## Goals / Non-Goals

**Goals:**
- Title text on the artwork, once per track/box/protocol, crisp (no downscale blur).
- Exactly one site carries the title at any time; failure modes land on the header.
- Reuse the hero's cover-fit + cache pattern; one pure, hermetically testable compositor.

**Non-Goals:**
- Time-varying overlay content; wrapping; marquee; user font config; system-font lookup;
  non-Latin coverage (fallback is the header).
- Overlay on Library hero, MPRIS art, or any non-queue image.
- Changing the main spec's stale title-band requirement.

## Decisions

**D1. Compose at final pixel size, keyed by box.** The overlay bitmap is the plain cover scaled
to the card's rendered box in pixels (cells x `image_font_size()`), then scrim + text drawn on it,
then encoded. Painting it at that same cell box means the protocol scales ~1:1, so text is not
softened by `Resize::Scale`. The box is obtained from the existing base protocol's `size_for`
(the same call the painter uses), so the compositor never re-implements ratatui-image's fit math.
*Alternative:* composite on the full-res source and let `Resize::Scale` shrink it - rejected,
text blurs and its size would track the source resolution.

**D2. Text size is cell-relative, not image-relative.** Nominal row height = one cell row
(`font.height`); glyph size ~70% of it; floor ~60% of nominal; below the floor, ellipsis.
Image size only changes how many characters fit. *Alternative:* percentage of image width (the
logo's model) - rejected, text would be huge on large art and illegible on small art.
Fallback `FontSize` (10x20, used when no picker is active) makes size approximate, but the
overlay is only live when a real protocol is configured.

**D3. Separate cache entry, not the `:P` entry.** The composed variant is its own `CachedImage`
under `{base-key}:t:{cols}x{rows}:{title-hash}` whose `img` is the composed bitmap and whose
`protocols` map is filled per suffix like any entry. The plain `:P` entry stays plain (MPRIS
and Library read it) and doubles as the measuring protocol and the fallback paint. Stale
variants age out through the existing image LRU. *Alternative:* extend `:P` with another applied-
key field - rejected, one entry cannot be both the plain and composed artwork, and sharing a key
across consumers is the exact flash bug `audiobookshelf_hero_cover_cache_key` documents.

**D4. Typed title site, decided once.** `mbv-ui-model` gets
`enum NowPlayingTitleSite { Header, Artwork }`, computed by the shell at
`sync_queue_playback_panel`/`refresh_queue_card_image` and consumed by both the panel (header text)
and the card projection (compose or not). `Artwork` iff: playing, images enabled, visualizer off,
slot shown, `!is_halfblock_configured()`, glyph-covered title, and a shell-recorded fact that the
overlay variant was painted for the current key+box+title. Everything else is `Header`. The
"painted" fact is recorded next to `record_card_size`; it lags one frame, which errs toward the
header (title shown twice for a frame at most, never zero times). The dim-backdrop halfblock
suffix is deliberately not an input.

**D5. Font and rasteriser.** Embed one static Lexend Deca weight (OFL; licence text shipped
beside it) via `include_bytes!` and rasterise with `ab_glyph` (pure Rust, no system deps).
Coverage is Latin / Latin-Ext / Vietnamese; the compositor exposes `covers(text) -> bool`
(`glyph_id != 0` for every char) and the site rule treats `false` as `Header`.
*Alternatives:* `fontdue` (comparable); system font via fontconfig (heavy dependency, still no
CJK guarantee); variable TTF (needs variation support - use a static instance instead).
Task 1.1 confirms the crate and weight before anything else builds on them.

**D6. Compositor home and shape.** A pure `mbv-images` module (`title_overlay.rs`):
`compose_title_overlay(base: &DynamicImage, cell: FontSize, parts: &TitleOverlayText, colours)
-> DynamicImage`, plus `covers`. It takes resolved RGB colours (from the theme roles
`PLAYBACK_TITLE_FG` / `PLAYBACK_CONTEXT_FG`; task 1.2 verifies they are `Color::Rgb`), no `App`.
Placement is top row for the context-or-lone title, bottom row for a two-part title; each row
gets a vertical alpha gradient scrim.

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
- **Theme colour not RGB** -> if a role is an indexed colour, resolve it through the palette
  table; otherwise pick the nearest RGB constant (task 1.2).
- **Memory** -> at most a few composed bitmaps live; the LRU bounds them. Must not regress the
  headless footprint (a headless owner never composes: it has no card).

## Open Questions

- Exact scrim opacity/height and Lexend weight (Medium vs SemiBold): tuned visually during
  task 3.3; no spec impact.
