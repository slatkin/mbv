# Invariant 16 — The title site is decided before the panel first displays

## The invariant

The queue card's title site (`NowPlayingTitleSite`) is `Artwork` exactly when
the sync-pass eligibility gates say the artwork will carry the now-playing
title: playback active, images on with a kitty/sixel-family protocol, the
visual slot shown, an active item with title parts, and a title whose glyphs
cover the overlay rows. The decision is made in the projection sync — before
the header ever paints for this playback — never from a paint-time fact. A
base-art fetch still in flight counts as eligible (the art will arrive); only
a fetch that resolved empty (no art will ever exist) keeps the site `Header`.

The painted-overlay key is not a site input. The card painter alone uses the
variant/plain cache keys to choose which bitmap paints; until the overlay
variant is encoded the slot paints plain art, so the artwork carries no title
for those frames. The header always paints, so it papers over that gap: from
the first frame of eligible playback it shows the playing brand row
(` [mbv] ... PLAYING:<host>`) while the artwork shows plain art, and the
overlay carries the title alone once encoded. That is the accepted trade
(2026-10-03 decision, revised 2026-10-08: the header row never collapses —
the user restored the always-visible header panel).

The site is *sticky against transient churn*: the projection carries it over
between syncs, and only mode gates overwrite it. Mode gates are stable
presentation facts — idle, visualizer, images off, halfblock configured, no
slot, no title, no art will ever exist, a font that cannot render the title
(`UncoveredGlyph`). Transient gates — a column-resize drag, an unmeasured
protocol size, an encode in flight — leave the carried site untouched, so the
header cannot resurrect for the duration of the churn. Hiding the visual slot
while playing is a mode change made outside the refresh path (`sync_queue`
resets the site to `Header`), because the artwork can no longer carry the
title. The site's covers rule is the strict, logo-free one, so a logo fetch
landing mid-playback cannot flip the site either.

## Why it matters

Deciding the site from whether the overlay *already painted* made the header's
content lag the panel's first display: playback start showed the title in the
header, then switched it to the brand row once the overlay encoded. Deciding
it from eligibility instead pins the site before the panel first displays,
while keeping the variant/plain key distinction where it belongs: in the
painter's bitmap choice, not in site ownership. The header row itself is
always visible (2026-10-08 user decision): it paints the title on the
`Header` site and the playing brand row on the `Artwork` site, so the site
only chooses the header row's content, never its visibility.

## How the code maintains it today

- `title_site_skip_reason` in `src/app/state/projection/card.rs` returns a
  `TitleSiteGate`: mode gates force `Header`, transient gates return early
  and leave the carried site untouched.
- `refresh_queue_card_image` starts each sync from the previous projection's
  site; `queue_title_overlay` sets `Artwork` right after the strict covers
  gate passes — before `ensure_title_overlay_protocol` may or may not
  produce the variant this sync.
- The header/geometry consumers (the panel's `view()` in
  `crates/mbv-components/src/queue_playback_panel.rs`) read only this
  projected fact, to choose the header row's content (title vs playing brand
  row) — never its visibility; the header band's two rows (the recess row
  above the text and the text row) are always reserved
  (`QUEUE_PLAYBACK_HEADER_ROWS`).
- Tests: `column_resize_drag_builds_no_overlay_variant_until_it_ends`
  (transient carry-over across a drag),
  `the_site_is_decided_while_the_art_fetch_is_pending` (pre-paint decision),
  `a_mode_gate_forces_the_header_back_from_the_artwork_site` (mode gates
  overwrite), `hiding_the_visual_slot_while_playing_returns_the_title_to_the_header`
  (the slot-hide revert), `header_rows_are_always_reserved_while_the_title_lives_on_the_artwork`
  (geometry) and `header_row_paints_the_brand_row_while_the_title_lives_on_the_artwork`
  (panel paint content); `building_title_overlay_leaves_shared_plain_card_bitmap_unchanged`
  in `src/app/infra/image_fetch/protocol/protocol_tests.rs` (the variant never
  mutates the plain entry, so the painter's plain fallback cannot alias the
  overlay variant).

## Where it currently fails

Known accepted gap: between playback start and the overlay variant's first
paint, the artwork carries no title (the art shows plain or is still loading)
while the header already shows the playing brand row. A fetch that resolves
empty keeps the header as the title's home, so the gap is bounded by the
fetch/encode window, never unbounded — unless a variant never becomes
encodable while eligibility holds (no known path).
