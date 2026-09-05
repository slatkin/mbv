## Context

See `proposal.md` for motivation. Current state:

- Paint-time key is built inline in `home_hero_emby.rs::paint_home_image` as
  `format!("{}:ser:{}", item.id, image_types.join(","))`. Two producers exist:
  TV Wide landscape (`hero_model.rs` Series+`Landscape` → `Thumb,Primary,Backdrop,Logo`,
  via `tv_wide.rs`) and narrow inline detail (`detail_series_view.rs` → `Primary`).
  Both chains are live: the `Thumb` URL arm (`images.rs`) fetches different bytes,
  so the keys must stay distinct.
- Four shell-side sites still format the pre-`0cbd51b7` key inline:
  `shell_tv_workspace.rs` prefetch + `image_loading`, `list_narrow.rs` `image_loading`,
  `shell_run.rs` completion gate, plus the `shell_tv_workspace_tests.rs` fixture.
- Precedent: `compact_banner_image_cache_key` in `detail.rs` already centralizes a
  shared key so the eager fetcher and the prefetch loop cannot drift. The Series key
  needs the same treatment. `card.rs` also imports key helpers from
  `crate::app::images`, so `images.rs` is an established home for cache-key
  constructors reachable from `App` methods, shell files (`shell_*` are `impl Model`
  / `impl App` blocks inside the `app` module tree), and the render components.

## Goals / Non-Goals

**Goals:**

- One fetch per Series per type-chain: the shell prefetch warms exactly the key the
  painter will request.
- Correct loading state on both TV Wide and narrow: placeholder shows until the
  painted entry lands, never a blank.
- Image completion triggers a targeted TV re-push for the `ser:` key family.

**Non-Goals:**

- No change to the TV Wide 16:9 landscape slot size or the larger kitty payload it
  entails (accepted design cost, proposal Out of scope).
- No change to fetch concurrency (`MAX_IMAGE_FETCHES`), server byte budget
  (`maxHeight=400&quality=80`), resize worker, disk cache, or render cadence.
- No new key scheme: the `{id}:ser:{types}` format introduced by `0cbd51b7` is kept.

## Decisions

**Series key helper lives in `images.rs` next to the other cache-key constructors**
(`compact_banner_image_cache_key`-style, imported from `crate::app::images` like
`card.rs` does). Alternatives: a method on `HeroArtwork` in `hero_model.rs`, or a
free function in `detail.rs`. `hero_model.rs` is provider-neutral content
(`item_id` + `image_types`) with no knowledge of the `App` image-cache key format;
`detail.rs` is the movie-banner module, the wrong home for a Series key. `images.rs`
already owns the `card_image_states`/`card_image_loading` key namespace, so the
constructor sits with its namespace owner.

Two shared items, not one. The key helper formats; it does not decide chains:
`pub(in crate::app) fn series_image_cache_key(item_id: &str, image_types: &[&str]) -> String`.
Chain ownership is centralized separately: one exported canonical constant for the
TV Wide Thumb-first chain (e.g. `SERIES_LANDSCAPE_IMAGE_TYPES` in `hero_model.rs`,
next to the `artwork_for` landscape mapping that must use it), so the painter's
chain and the shell prefetch's chain are the same item by construction, not two
literals that can drift. The narrow `&["Primary"]` chain stays inline at its two
sites (`detail_series_view.rs` paint, `list_narrow.rs` lookup) — a one-element
chain with no independent producer to drift from. No new `&["Thumb", ...]` literal
may be introduced anywhere; task 2.1 enforces this.

**TV Wide prefetch uses the full Thumb-first chain, not a prefix.** The paint site
passes `image_types` straight through to `fetch_card_image`, so the shell prefetch
must request the identical canonical constant to hit the identical key. Fetching
`Thumb,Primary` only would warm a different key and reintroduce the miss. The
double cost this creates (two entries per series, one per chain) is inherent to
`0cbd51b7`'s independent-caching design, now justified by the distinct Thumb bytes.

**Completion gate matches the `:ser:` infix, not an enumerated suffix list.**
`shell_run.rs` currently tests `ends_with(":ser_primary")`. Matching `:ser:` (the
family introduced by `0cbd51b7`, which no other key namespace uses) covers both
`ser:Primary` and `ser:Thumb,...` without a second drift site when chains change.
Verified: no other cache key in the tree contains `:ser:`.

**Narrow keeps its own `Primary`-chain entry; it does not share Wide's entry.**
Unifying would reintroduce the cross-surface clobbering `0cbd51b7` fixed (now real,
given distinct Thumb bytes). The narrow `image_loading` lookup moves to the helper
with `&["Primary"]`, matching what `detail_series_view.rs` paints.

**Disk-cache orphans under the old `{id}:ser_primary` key are left to expire.**
`evict_old_image_cache` (`src/config.rs`) removes entries older than 30 days by
mtime; no migration or bulk delete. Orphans may therefore persist up to 30 days.
The old entries are small (server-sized bytes) and unreachable after the fix, so
this is accepted.

## Risks / Trade-offs

- [Risk] A future third Series paint chain adds a site that formats the key inline
  instead of using the helper → Mitigation: tasks include a repo-wide search proving
  no `ser_primary` / inline `ser:` format remains outside the helper and its tests.
- [Risk] Prefetching the full Thumb-first chain issues up to 4 candidate fetches
  worth of server requests per new series vs 1 today → Mitigation: `find_map` stops
  at the first type that returns bytes; Thumb exists for the overwhelming majority
  of series, so the common case is still one request, now under the right key.
- [Risk] Widening the completion gate to `:ser:` re-pushes TV content on narrow-only
  completions too → Mitigation: `push_tv_workspace_content` is idempotent
  (fetch dedupes via `card_image_loading`/`card_image_states`); the re-push is
  already gated to Wide-active TV (`is_wide_tv_active` early return).

## Migration Plan

No deployment steps: in-memory keys plus best-effort disk bytes. Rollback is revert;
the old key's disk entries remain readable until pruned, so rollback degrades to
today's behavior without data loss.

## Open Questions

None. The Wide slot-size question from exploration is a separate design decision
(landscape artwork presentation), explicitly out of scope here.
