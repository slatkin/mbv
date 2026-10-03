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
variant is encoded the slot paints plain art, so the title may be absent from
both sites for those frames. That gap is accepted by decision (2026-10-03):
the header must not flash the title for the fetch/encode window and collapse
afterwards, so the header yields the title's home to the artwork up front.

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
fate lag the panel's first display: playback start showed the title in the
header, then dropped it once the overlay encoded — a one-time flash on every
start. Deciding it from eligibility instead pins the header's fate before its
first paint, while keeping the variant/plain key distinction where it belongs:
in the painter's bitmap choice, not in site ownership.

## How the code maintains it today

- `title_site_skip_reason` in `src/app/state/projection/card.rs` returns a
  `TitleSiteGate`: mode gates force `Header`, transient gates return early
  and leave the carried site untouched.
- `refresh_queue_card_image` starts each sync from the previous projection's
  site; `queue_title_overlay` sets `Artwork` right after the strict covers
  gate passes — before `ensure_title_overlay_protocol` may or may not
  produce the variant this sync.
- The header/geometry consumers (`queue_playback_header_visible` in
  `crates/mbv-render/src/arrangements/chrome.rs`, the panel's paint gate) read
  only this projected fact.
- Tests: `column_resize_drag_builds_no_overlay_variant_until_it_ends`
  (transient carry-over across a drag),
  `the_site_is_decided_while_the_art_fetch_is_pending` (pre-paint decision),
  `a_mode_gate_forces_the_header_back_from_the_artwork_site` (mode gates
  overwrite), `hiding_the_visual_slot_while_playing_returns_the_title_to_the_header`
  (the slot-hide revert), `header_rows_collapse_only_while_the_title_lives_on_the_artwork`
  (geometry) and `header_row_paints_the_title_only_while_it_is_the_title_site`
  (panel paint gate); `building_title_overlay_leaves_shared_plain_card_bitmap_unchanged`
  in `src/app/infra/image_fetch/protocol/protocol_tests.rs` (the variant never
  mutates the plain entry, so the painter's plain fallback cannot alias the
  overlay variant).

## Where it currently fails

Known accepted gap: between playback start and the overlay variant's first
paint, the title is on neither site (the art shows plain or is still loading).
A fetch that resolves empty keeps the header as the title's home, so the gap
is bounded by the fetch/encode window, never unbounded — unless a variant
never becomes encodable while eligibility holds (no known path).
