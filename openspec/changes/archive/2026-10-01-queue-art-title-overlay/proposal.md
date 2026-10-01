# Proposal

## Why

The queue column's now-playing artwork is usually non-descriptive (a cover the user already
knows), while the title that matters sits on a header row that must marquee long titles and
long context/title pairs. Painting the title onto the artwork itself reclaims that header row
for a plain "Now Playing" label and lets long titles be fitted (shrink, then ellipsis) instead
of scrolled.

## What Changes

- When the queue card shows real artwork through an image protocol (Kitty, Sixel, iTerm2), the
  now-playing title is rasterised into the artwork bitmap: a one-part title in a scrim row at the
  top; a two-part title (artist / song, show / episode) with the context in a scrim row at the top
  and the title in a scrim row at the bottom.
- When the playing Emby item has a loaded logo (a movie's own, an episode's show logo), the logo is
  drawn in the upper-left corner in place of the top text row and its scrim; a two-part title keeps
  its bottom row and a one-part title becomes the logo alone.
- Text uses an embedded JetBrainsMono Nerd Font SemiBold (OFL) face, sized from the terminal's cell pixel size, fitted
  to the row by shrinking to a floor, then ellipsis.
- The queue header row says `Now Playing` (keeping its state icon: aqua play, yellow pause) while the overlay is
  live, and keeps carrying the title as it does today in every other state: halfblocks, visualizer,
  placeholder/loading slot, images disabled, slot hidden, or a title the font cannot fully render.
- The composed artwork is a separate cache entry per track/box, rebuilt on track change and on
  box or protocol change; nothing time-varying is baked into pixels.
- Adds one dependency (a glyph rasteriser) and one embedded font file.

## Capabilities

### New Capabilities
- `queue-artwork-title-overlay`: when the title is drawn onto the queue artwork, its layout
  (one-part vs two-part rows, scrims, fitting), and the single rule deciding whether the artwork
  or the header row carries the title.

### Modified Capabilities
- `queue-playback-panel`: the "Queue-visible layouts paint a now-playing header row" requirement
  gains the `Now Playing` state and folds in the header-carries-the-title behaviour that shipped
  in 55346db33 and 694acfb69 but never reached the main spec (no status word or host while a
  target plays, aqua play / yellow pause icon, two-part title right-aligned). The delta also
  brings the queue-column layout requirements up to the shipped band: a fixed four-row transport
  (controls, blank title row, seekbar with times, gap row), no separator row, and the queue panel
  adjoining the playback region (the "playback panel renders in the queue column", "queue panel
  opens directly", "Idle collapse", "pointer input", "hide visual slot", title-row and
  colour-delineation requirements).

## Impact

- `crates/mbv-images` (text/scrim compositor, overlay cache entries), `src/app/infra/image_fetch/`
  (overlay ensure/rebuild next to the hero cover-fit path), `src/app/state/projection/card.rs`
  (the card paints the overlay variant), `crates/mbv-ui-model` (typed title-site fact),
  `crates/mbv-components/src/queue_playback_panel.rs` and `crates/mbv-render` header painter.
- New dependency: a pure-Rust glyph rasteriser (`ab_glyph`, to be confirmed in task 1.1);
  new binary asset: JetBrainsMono Nerd Font SemiBold static weight plus its OFL text.
- No change to Player/queue authority, protocols, or persisted state.
