# Design: home-rows-playback-palette

## Context

The merged `now-playing-media-type-titles` change (PR #730) put a typed two-part title model in
core (`QueueItem::playback_title_parts` → `PlaybackTitleParts`, closed `Title`/`Context` roles) and
gave the playback strip one colour-resolution site (`title_part_fg`): title = aqua
(`PLAYBACK_TITLE_FG`, `#35a77c`), context = gold (`PLAYBACK_CONTEXT_FG`, `#dbbc7f`).

Home rows still project through the older stringly `display_name_parts()`, which only splits
Emby episodes and ABS show episodes, and the canonical painter's two-tone branch paints
container/context in the ordinary title role (soft white) and the item title in the yellow
focus-accent role.

## Goals / Non-Goals

**Goals**

- One two-tone palette for "container + item title" everywhere it is painted: the now-playing
  roles.
- Home rows gain the artist and subscription context fields the strip already shows.
- Config-free components: feed subscription names resolve shell-side.

**Non-Goals**

- Restyling single-part (title-only) rows — they keep the ordinary title role per semantic state.
- Changing `PlaybackTitleParts`, its media-type mapping, or `title_part_fg` — all reused as-is.
- Per-list palette opt-ins, variants, or new row-model types.
- Touching grouped-list mechanisms (Heading rows, zebra fills) — unrelated to two-tone titles.

## Decisions

### D1: Painter-level palette change, not a typed row variant

The two-tone colours change in place in `media_list/row.rs` (two sites: the marquee `parts` vec and
the truncation branch). Rationale: a survey of every `MediaListRow::Item` constructor in production
found Home is the *only* destination that sets `secondary: Some` — every other component passes
`secondary: None`, and grouped lists use `Heading` rows, not split titles. A typed palette field on
the row model would therefore be paid at ~15 constructor sites to serve exactly one user, and a
paint-call parameter would thread through the carrier for the same single user. If a future
destination wants a *different* split treatment, that is when a typed variant earns its existence;
until then the row contract's own wording ("split rows paint the now-playing palette, no per-list
opt-in") is the guard.

*Alternatives considered:* typed `TitlePalette` field on `MediaListRow::Item` (per-list expansion
via field flips — rejected: speculative, constructor churn); palette parameter threaded through the
carrier/view chain (rejected: same speculative cost, one moving part instead of zero).

### D2: Reuse `playback_title_parts`; shell resolves feed names

`project_active_section` swaps `item.display_name_parts()` for the playback title-parts mapping.
The mapping needs the feed subscription display name, which only `Config` can resolve; components
never receive `Config`. So `assign_home_content` (shell-side, has config) resolves names for the
sections it assigns and stores a feed-id → display-name lookup on `HomeContent`; the component's
projection looks entries up by `feed_id`. This mirrors how the App layer already feeds
`feed_subscription_display_name` into `playback_title_parts` for the strip
(`chrome_player_context.rs`), without giving the component config access.

*Alternatives considered:* resolving names inside the component (rejected: violates the
component/`Config` boundary); rewriting feed entries' titles at assignment (rejected: mutates
domain items for presentation).

### D3: Played split rows mute the item title only

Today a played split row paints primary muted + secondary yellow — the mute follows the *position*
(primary), not the *role*. Under the role mapping, the mute follows the title role: played mutes
the item title (aqua → `TEXT_MUTED`), the context keeps gold. This preserves today's visible
asymmetry (the container name stays legible on watched rows) rather than inventing a new
mute-everything rule.

### D4: Title-only rows are explicitly out of the palette

"PR roles exactly" applies to *split* rows. A row with no context keeps the ordinary title role
(soft white, muted when played) — the aqua/gold pair marks a known container relationship, and
spending the now-playing aqua on every Home row would dilute both it and the soft-white emphasis.

## Risks / Trade-offs

- [Painter-level change restyles any future list that adopts `secondary`] → The spec clause pins
  the split-row palette to the now-playing roles with no per-list opt-in; a future destination that
  wants different colours must reopen the contract, which is the intended friction.
- [Feed subscription renamed/deleted between fetch and render] → The lookup resolves at assignment
  time; a stale name persists until the next Home refresh — same staleness window the strip has.
- [`PlaybackTitleParts` is a playback-queue type feeding a browse row] → Accepted: the *mapping*
  is reused, not the transport context; Home still renders provider-neutral `MediaListRow` strings,
  so the canonical row model stays colourless and playback-free.

## Migration Plan

Single binary, no persistence migration. Ship painter + projection + shell resolution together;
the pinning two-tone tests update in the same commit so no intermediate state paints the legacy
palette. Rollback = revert the commit.

## Open Questions

None.
