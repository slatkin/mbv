# Compact Banner Image Pre-warming (Issue #287)

## Problem

The Power View compact movie-detail banner's poster image (`CompactBannerLayout`,
`src/app/render/power/detail.rs`, added by #263) only starts loading once the list
cursor actually lands on a movie. `compact_banner_layout()` calls `fetch_card_image()`
directly for the *currently selected* item's image alone — there is no lookahead for
rows the cursor is about to reach. Every cursor move into a new movie is a cold fetch:
an Emby HTTP request plus resize/encode for the terminal image protocol, with zero head
start, which is visible as a noticeable per-row load delay when browsing.

This is inconsistent with the rest of the app. Nearly every other list-image surface
already prefetches neighbors via `fetch_list_card_image_when_idle()` — episode lists
(`render/power/episode.rs`), the home carousel (`render/home.rs`), library album/season
grids (`render/library/album.rs`, `render/library/season_grid.rs`), the legacy library
table (`render/library/table/context.rs`), and, notably, the Power View home-card
carousel itself (`render/power/card.rs`), which already prefetches a window of
`PREFETCH_AHEAD = 3` items ahead and `PREFETCH_BEHIND = 1` behind the cursor. The
compact movie banner never got this treatment.

## Goal

When the list cursor sits on (or near) a leaf `Movie` item in a Power View movie
library, pre-fetch the compact banner's poster image for nearby movies in the same
list — so that by the time the cursor actually reaches one of them, its poster is
already fetched (and, ideally, already resized/encoded for the active terminal image
protocol) instead of visibly loading in.

## Scope

Movies library only, matching #263's scope — this only pre-warms
`compact_banner_layout`'s `"{item_id}:cmp_primary"` image cache entries. No other list
surface's prefetch behavior changes.

Within a movies library, only leaf `Movie` items participate (folders/box-sets never
show a banner or a poster, so they're never worth prefetching for this purpose).

## Design

### Window and mechanism

Reuse the exact pattern `render/power/card.rs` already establishes for the home-card
carousel: a fixed window of `PREFETCH_AHEAD` items ahead and `PREFETCH_BEHIND` items
behind the cursor, fetched via `fetch_list_card_image_when_idle()` (not the raw
`fetch_card_image()` the current cursor's own eager fetch uses). Using the same
already-idle-gated helper as every other prefetching surface means:

- The prefetch naturally respects the existing `NAV_IMAGE_FETCH_IDLE_DELAY` (150ms)
  debounce that gates all list-image fetch/render work during active navigation, with
  no new gating logic to write or reason about.
- The currently-selected item's own image fetch is unaffected — it keeps using the
  eager, non-idle-gated `fetch_card_image()` call `compact_banner_layout()` already
  makes today, since that one is needed immediately, not prefetched.

Reuse `card.rs`'s exact `PREFETCH_AHEAD = 3` / `PREFETCH_BEHIND = 1` values rather than
inventing new tuning constants — this is a homogeneous prefetch-window convention
already established elsewhere in Power View, and there's no evidence poster images
specifically need a different window.

### Integration point

`render_power_list()` (`src/app/render/power/list.rs`) already gathers `items` (the
current level's `Vec<MediaItem>`) and `cursor` before computing the compact banner's
row budget. This is the natural place to add the prefetch loop: once `items`/`cursor`
are known and the library is a movies library, walk a `[cursor - PREFETCH_BEHIND,
cursor + PREFETCH_AHEAD]` window over `items`, skip non-`Movie`/folder entries, and call
`fetch_list_card_image_when_idle()` for each with the same cache-key shape
`compact_banner_layout()` already uses (`format!("{}:cmp_primary", item.id)`) and the
same image type list (`&["Primary"]`).

### Cache-key and fetch-shape consistency

The prefetch loop must build its cache key and fetch call exactly the way
`compact_banner_layout()` does today, so a prefetched entry is actually a cache hit when
the cursor arrives (a mismatched key format would prefetch into a cache slot the real
render never looks up, silently wasting the work). This is a duplication risk worth
calling out explicitly in the implementation plan — consider whether the cache-key
construction should be a small shared helper instead of copy-pasted in two places.

## Unaffected / non-goals

- `NAV_IMAGE_FETCH_IDLE_DELAY` (the 150ms debounce) is not changed by this work. It was
  discussed during triage: today it stacks on top of a cold, unprefetched fetch, which
  is part of what makes navigation feel slow, but once this change lands the debounce
  will usually be masking an already-warm cache hit instead of gating a cold fetch. Its
  value should be revisited only after this change has shipped and its effect can be
  judged in isolation — not bundled into this work.
- No other list-image surface's prefetch window or behavior changes (episodes, home,
  album/season grids, legacy table, the home-card carousel itself).
- No new persisted state, config, or user-facing setting. This is a pure perceived
  -latency improvement to existing behavior.
- Does not change what image types are fetched (`&["Primary"]`, matching the compact
  banner's existing single-image render) or the compact banner's rendering/layout logic
  from #263.

## Open implementation questions (for the plan, not blocking design approval)

- Whether the prefetch loop should skip fetching for the item currently under the
  cursor (already covered by `compact_banner_layout()`'s own eager fetch) to avoid a
  redundant `fetch_list_card_image_when_idle()` call that will just hit the "already
  loading/loaded" early-return in `fetch_card_image()` — harmless either way, but worth
  a clean implementation choice. Proposed default: skip it, for clarity of intent even
  though it's a no-op either way.
- Whether cache-key construction (`"{}:cmp_primary"`) should be factored into a small
  shared helper used by both `compact_banner_layout()` and the new prefetch loop, so the
  two can never drift apart silently. Proposed default: yes, a private helper function
  in `detail.rs` (e.g. `fn compact_banner_image_cache_key(item_id: &str) -> String`),
  reused by both call sites — left to the plan to decide if it's proportionate.
